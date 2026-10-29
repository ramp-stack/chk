use std::{any::Any, collections::HashMap, marker::PhantomData, sync::{Arc, Mutex}};
use chk::{MessagesPage, Theme, ListItem, messages::{ChatRoom, ChatRoomAction, RoomReadState}};
use prism::{Context, air::{Contract, ContractType, Id, Metadata, Name, Request, Value}, traits};
use prism::drawable::{Drawable, Item};
use prism::events::WindowEvent;
use futures::FutureExt;
use chk::air::typing::{TypingState, TypingAction};

mod support;

fn names() -> [Name; 3] {
    [Name::orange_me(),
        "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".parse().unwrap(),
        "0379be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".parse().unwrap()]
}
fn metadata(signer: Name, timestamp: u64) -> Metadata {Metadata {signer, timestamp, confirmed: true}}

#[test]
fn read_flags_are_per_reader_and_do_not_clear_later_or_hidden_messages() {
    let [me, other, third] = names();
    let id = Id::from(1);
    let mut room = ChatRoom::init(id, metadata(me, 0));
    for member in [other, third] {room.apply(ChatRoomAction::Share(member), metadata(me, 1));}
    room.apply(ChatRoomAction::SendMessage("Mine".into()), metadata(me, 2));
    room.apply(ChatRoomAction::SendMessage("Incoming".into()), metadata(other, 3));
    let incoming = room.messages(me).last().unwrap().clone();
    let mut mine = RoomReadState::init(id, metadata(me, 0));
    let theirs = RoomReadState::init(id, metadata(third, 0));
    assert_eq!(mine.unread(&room).collect::<Vec<_>>(), vec![incoming.id]);
    mine.apply(vec![incoming.id], metadata(other, 4));
    assert!(!mine.is_read(&incoming), "other members cannot clear my badge");
    mine.apply(vec![incoming.id], metadata(me, 5));
    mine.apply(vec![incoming.id], metadata(me, 6));
    assert_eq!(mine.unread(&room).count(), 0);
    assert_eq!(theirs.unread(&room).count(), 2, "reading a group must not clear another reader");
    room.apply(ChatRoomAction::SendMessage("Arrived later".into()), metadata(other, 7));
    let later = room.messages(me).last().unwrap().clone();
    assert_eq!(mine.unread(&room).collect::<Vec<_>>(), vec![later.id]);
    room.apply(ChatRoomAction::DeleteMessage(later), metadata(me, 8));
    assert_eq!(mine.unread(&room).count(), 0, "hidden messages must not keep a badge");
}

#[test]
fn old_room_bytes_still_load_and_read_flags_survive_serialization() {
    #[derive(serde::Serialize)]
    struct LegacyMessage {
        body: chk::messages::MessageContent, author: Name, timestamp: u64,
        hidden_from: Vec<Name>, id: Id,
    }
    #[derive(serde::Serialize)]
    struct LegacyRoom {members: Vec<Name>, messages: Vec<LegacyMessage>, id: Id}
    let [me, other, _] = names();
    let mut room = ChatRoom::init(Id::from(1), metadata(me, 0));
    room.apply(ChatRoomAction::Share(other), metadata(me, 0));
    room.apply(ChatRoomAction::SendMessage("Existing history".into()), metadata(other, 1));
    let legacy = LegacyRoom {
        members: room.members.clone(), id: room.id,
        messages: room.messages(me).map(|message| LegacyMessage {
            body: message.body.clone(), author: message.author, timestamp: message.timestamp,
            hidden_from: message.hidden_from.clone(), id: message.id,
        }).collect(),
    };
    let bytes = postcard::to_allocvec(&legacy).unwrap();
    assert_eq!(bytes, postcard::to_allocvec(&room).unwrap(), "shared room encoding must remain unchanged");
    let restored: ChatRoom = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(restored, room);

    let mut reads = RoomReadState::init(room.id, metadata(me, 0));
    reads.apply(reads.unread(&room).collect(), metadata(me, 2));
    let restored: RoomReadState = postcard::from_bytes(&postcard::to_allocvec(&reads).unwrap()).unwrap();
    assert_eq!(restored.unread(&room).count(), 0);
    assert_eq!(restored.owner, me);
    assert_eq!(restored.room, room.id);
}

#[derive(Clone, Default, Debug)]
struct MemoryAir(Arc<Mutex<HashMap<(Id, Id), Box<dyn Any + Send + Sync>>>>);

impl traits::Air for MemoryAir {
    fn me(&self) -> Name {names()[0]}
    fn instances(&self, contract: ContractType) -> Box<dyn traits::DynInstances> {
        if contract.id == ChatRoom::id() {Box::new(Instances::<ChatRoom>(self.clone(), PhantomData))}
        else if contract.id == RoomReadState::id() {Box::new(Instances::<RoomReadState>(self.clone(), PhantomData))}
        else if contract.id == TypingState::id() {Box::new(Instances::<TypingState>(self.clone(), PhantomData))}
        else if contract.id == chk::profiles::Profile::id() {Box::new(Instances::<chk::profiles::Profile>(self.clone(), PhantomData))}
        else if contract.id == chk::blocking::Block::id() {Box::new(Instances::<chk::blocking::Block>(self.clone(), PhantomData))}
        else {panic!("unexpected contract")}
    }
    fn start(&self, _: prism::service::ServiceInit) {panic!("unexpected service")}
    fn shutdown(&self, _: prism::service::Key) {panic!("unexpected service")}
}

#[derive(Clone, Debug)]
struct Instances<C: Contract>(MemoryAir, PhantomData<C>);
impl<C: Contract> traits::DynInstances for Instances<C> {
    fn create(&self, init: Request) -> Box<dyn traits::DynInstance> {
        let init = *init.downcast::<C::Init>().unwrap();
        let id = Id::hash(&(names()[0], &init));
        let mut all = self.0.0.lock().unwrap();
        let entry = all.entry((C::id(), id)).or_insert_with(|| Box::new(Stored {
            id, state: Arc::new(Mutex::new(C::init(init, metadata(names()[0], 0)))),
        }));
        Box::new(entry.downcast_ref::<Stored<C>>().unwrap().clone())
    }
    fn list(&self) -> HashMap<Id, Box<dyn traits::DynInstance>> {
        self.0.0.lock().unwrap().iter().filter_map(|((kind, id), stored)| {
            (*kind == C::id()).then(|| (*id, Box::new(stored.downcast_ref::<Stored<C>>().unwrap().clone()) as Box<dyn traits::DynInstance>))
        }).collect()
    }
    fn try_next(&mut self) -> Option<Box<dyn traits::DynInstance>> {None}
    fn next(&mut self) -> traits::BoxFuture<'_, Box<dyn traits::DynInstance>> {Box::pin(std::future::pending())}
}

#[derive(Clone, Debug)]
struct Stored<C> {id: Id, state: Arc<Mutex<C>>}
#[derive(Debug)]
struct Snapshot<T>(T);
impl<T: std::fmt::Debug + Send + Sync + 'static> traits::DynRef for Snapshot<T> {
    fn value(&self) -> &(dyn Any + Send + Sync) {&self.0}
}
impl<C: Contract> traits::DynInstance for Stored<C> {
    fn id(&self) -> Id {self.id}
    fn notify(&self, enabled: bool) {assert!(!enabled, "read and typing updates must not enable notifications");}
    fn share(&self, _: Name) {
        assert!(C::id() == TypingState::id() || C::id() == ChatRoom::id(), "read state must remain private");
    }
    fn send(&mut self, message: Request) -> Box<dyn traits::DynPending> {
        Box::new(Pending(self.state.lock().unwrap().apply(*message.downcast::<C::Message>().unwrap(), metadata(names()[0], 100))))
    }
    fn pending(&self) -> Box<dyn traits::DynRef + '_> {Box::new(Snapshot(self.state.lock().unwrap().clone()))}
    fn confirmed(&self) -> Option<Box<dyn traits::DynRef + '_>> {Some(self.pending())}
    fn try_next(&mut self) -> Option<traits::ContractUpdate> {None}
    fn next(&mut self) -> traits::BoxFuture<'_, traits::ContractUpdate> {Box::pin(std::future::pending())}
}
#[derive(Clone, Debug)]
struct Pending<T>(T);
impl<T: Clone + std::fmt::Debug + Send + Sync + 'static> traits::DynPending for Pending<T> {
    fn pending(&self) -> Box<dyn traits::DynRef + '_> {Box::new(Snapshot(self.0.clone()))}
    fn try_confirmed(&self) -> Option<Value> {Some(Arc::new(self.0.clone()))}
    fn try_next(&mut self) -> Option<bool> {None}
    fn next(&mut self) -> traits::BoxFuture<'_, bool> {Box::pin(std::future::pending())}
}

fn settle(page: &mut impl Drawable, ctx: &mut Context) {
    for _ in 0..50 {
        page.request_width();
        page.request_height(400.0);
        page.layout((400.0, 800.0));
        let events = ctx.events();
        if !events.is_empty() {
            for event in events {
                page.request_width();
                page.request_height(400.0);
                let layout = page.layout((400.0, 800.0));
                page.event(ctx, &layout, event);
            }
        } else if let Some(callback) = page.changed().now_or_never() {callback(page, ctx);}
        else {return;}
    }
    panic!("page did not settle");
}

#[test]
fn opening_marks_read_but_construction_background_and_hidden_flows_do_not() {
    use ptsd::{display::Bin, layout::Stack, navigation::{Pages, Flow, FlowContainer, AppPage, NavigationEvent}};
    use prism::drawable::Component;
    #[derive(Debug, Clone, Component)]
    struct TestFlow(Stack, Flow);
    impl FlowContainer for TestFlow {fn flow(&mut self) -> &mut Flow {&mut self.1}}
    #[derive(Debug, Clone, Component)]
    struct Empty(Stack, Option<prism::drawable::Shape>);
    impl AppPage for Empty {}

    let air = MemoryAir::default();
    let mut ctx = support::context();
    ctx.air = prism::air::Air::new(air.clone());
    let mut room = ctx.air.instances::<ChatRoom>().create(Id::from(1));
    room.send(ChatRoomAction::Share(names()[1]));
    let stored = air.0.lock().unwrap()[&(ChatRoom::id(), room.id())].downcast_ref::<Stored<ChatRoom>>().unwrap().clone();
    let receive = |timestamp| {
        stored.state.lock().unwrap().apply(ChatRoomAction::SendMessage("Hello".into()), metadata(names()[1], timestamp));
    };
    receive(1);
    let theme = Theme::default();
    let mut page = MessagesPage::new(&mut ctx, &theme, room.clone(), 1);
    let reads = RoomReadState::open(&ctx, room.id());
    assert_eq!(reads.pending().unread(&room.pending()).count(), 1, "building a page must not read it");
    settle(&mut page, &mut ctx);
    assert_eq!(reads.pending().unread(&room.pending()).count(), 0);

    ctx.emit(WindowEvent::Focused(false));
    settle(&mut page, &mut ctx);
    receive(2);
    let mut refreshed = MessagesPage::new(&mut ctx, &theme, room.clone(), 1);
    refreshed.take_input_from(&mut page);
    settle(&mut refreshed, &mut ctx);
    assert_eq!(reads.pending().unread(&room.pending()).count(), 1, "background refresh must stay unread");
    ctx.emit(WindowEvent::Focused(true));
    settle(&mut refreshed, &mut ctx);
    assert_eq!(reads.pending().unread(&room.pending()).count(), 0);

    receive(3);
    let hidden = MessagesPage::new(&mut ctx, &theme, room.clone(), 1);
    let flow = TestFlow(Stack::default(), Flow::new(vec![Box::new(hidden), Box::new(Empty(Stack::default(), None))]));
    let mut pages = Bin(Stack::default(), Pages::new(vec![("root".into(), Box::new(Empty(Stack::default(), None)))]));
    ctx.emit(NavigationEvent::push(flow));
    ctx.emit(NavigationEvent::Next);
    settle(&mut pages, &mut ctx);
    assert!(pages.1.current().downcast_ref::<Empty>().is_some(), "test must navigate away from the message page");
    assert_eq!(reads.pending().unread(&room.pending()).count(), 1, "hidden page must not clear the badge");
    ctx.emit(NavigationEvent::Pop);
    settle(&mut pages, &mut ctx);
    assert_eq!(reads.pending().unread(&room.pending()).count(), 0);
}

#[test]
fn list_item_badge_is_drawn_and_changes_item_equality() {
    let theme = Theme::default();
    let plain = ListItem::plain("Friend", "Hello", None, None);
    let unread = plain.clone().with_badge(true);
    assert_ne!(plain, unread);
    for (item, expected) in [(plain, 0), (unread, 1)] {
        let mut item = item.build(&theme);
        item.request_width();
        let (_, height) = item.request_height(320.0);
        let layout = item.layout((320.0, height));
        let badge_count = item.draw(&layout, (0.0, 0.0), (0.0, 0.0, 320.0, height)).iter()
            .filter(|i| matches!(&i.item, Item::Shape(s) if s.width == 8.0 && s.height == 8.0)).count();
        assert_eq!(badge_count, expected);
    }
}

fn draw_page(page: &mut impl Drawable) -> Vec<prism::drawable::Instruction> {
    page.request_width();
    page.request_height(400.0);
    let layout = page.layout((400.0, 800.0));
    page.draw(&layout, (0.0, 0.0), (0.0, 0.0, 400.0, 800.0))
}

#[test]
fn typing_bubble_is_anonymous_in_dms_and_names_the_sender_below_in_groups() {
    let [me, other, third] = names();
    for group in [false, true] {
        let air = MemoryAir::default();
        let mut ctx = support::context();
        ctx.air = prism::air::Air::new(air.clone());
        let mut room = ctx.air.instances::<ChatRoom>().create(Id::from(1));
        room.send(ChatRoomAction::Share(other));
        if group {room.send(ChatRoomAction::Share(third));}
        let mut profile = ctx.air.instances::<chk::profiles::Profile>().create(other);
        profile.send(chk::profiles::ProfileAction::ChangeUsername("JadedFish".into()));
        let mut state = TypingState::init(room.id(), metadata(other, 0));
        state.apply(TypingAction::Activity {active: true, at: TypingState::now()}, metadata(other, 0));
        let stored = Stored {id: Id::from(99), state: Arc::new(Mutex::new(state))};
        air.0.lock().unwrap().insert((TypingState::id(), stored.id), Box::new(stored.clone()));
        let theme = Theme::default();
        let mut page = MessagesPage::new(&mut ctx, &theme, room.clone(), 1);
        settle(&mut page, &mut ctx);
        let drawing = draw_page(&mut page);
        let dots = drawing.iter().filter(|i| matches!(&i.item, Item::Shape(s) if s.width == 6.0 && s.height == 6.0)).collect::<Vec<_>>();
        assert_eq!(dots.len(), 3);
        let label = drawing.iter().find(|i| matches!(&i.item, Item::Text(t) if t.spans.iter().any(|s| s.text == "JadedFish is typing...")));
        assert_eq!(label.is_some(), group);
        if let Some(label) = label {assert!(label.offset.1 > dots[0].offset.1 + 6.0);}

        // The expiry timer refreshes an already-visible panel even without a new
        // message or a page rebuild (the memory AIR deliberately never notifies).
        stored.state.lock().unwrap().apply(TypingAction::Activity {active: false, at: TypingState::now()}, metadata(other, 0));
        futures::executor::block_on(async {
            let mut timeout = prism::events::Timer::new(std::time::Duration::from_secs(2));
            while draw_page(&mut page).iter().any(|i| matches!(&i.item, Item::Shape(s) if s.width == 6.0 && s.height == 6.0)) {
                let callback = futures::select! {
                    callback = page.changed().fuse() => callback,
                    _ = timeout.next().fuse() => panic!("typing indicator did not clear"),
                };
                callback(&mut page, &mut ctx);
            }
        });
        assert!(!draw_page(&mut page).iter().any(|i| matches!(&i.item, Item::Shape(s) if s.width == 6.0 && s.height == 6.0)));
        assert_eq!(room.pending().messages(me).count(), 0);
    }
}

#[test]
fn composer_publishes_activity_without_draft_text_and_stops_on_background_or_send() {
    use pelican_ui::components::TextInputEvent;
    let air = MemoryAir::default();
    let mut ctx = support::context();
    ctx.air = prism::air::Air::new(air.clone());
    let mut room = ctx.air.instances::<ChatRoom>().create(Id::from(1));
    room.send(ChatRoomAction::Share(names()[1]));
    let mut page = MessagesPage::new(&mut ctx, &Theme::default(), room.clone(), 1);
    ctx.emit(ptsd::events::TextInput::Focused(true));
    settle(&mut page, &mut ctx);
    let drawing = draw_page(&mut page);
    let input = drawing.iter().find_map(|i| match &i.item {Item::Text(t) if t.focused => Some(t.id), _ => None}).unwrap();
    ctx.emit(TextInputEvent::Set {id: input, value: "Private draft".into()});
    settle(&mut page, &mut ctx);
    let typing = ctx.air.instances::<TypingState>().list().into_values().next().unwrap();
    assert!(typing.pending().is_typing(TypingState::now(), None));
    assert_eq!(typing.pending().shared_with, vec![names()[1]]);
    assert_eq!(room.pending().messages(names()[0]).count(), 0);
    let bytes = postcard::to_allocvec(&*typing.pending()).unwrap();
    assert!(!bytes.windows(b"Private draft".len()).any(|w| w == b"Private draft"));
    ctx.emit(WindowEvent::Focused(false));
    settle(&mut page, &mut ctx);
    assert!(!typing.pending().is_typing(TypingState::now(), None));
    ctx.emit(WindowEvent::Focused(true));
    ctx.emit(TextInputEvent::Set {id: input, value: "Next edit".into()});
    settle(&mut page, &mut ctx);
    assert!(typing.pending().is_typing(TypingState::now(), None));
    ctx.emit(TextInputEvent::Submit {id: input});
    settle(&mut page, &mut ctx);
    assert!(!typing.pending().is_typing(TypingState::now(), None));
    assert_eq!(room.pending().messages(names()[0]).count(), 1);
}

#[test]
fn group_info_renders_controls_and_edits_do_not_overwrite_an_unchanged_remote_field() {
    use chk::{GroupMessageInfoPage, PageType, State, AvatarContent};
    let mut ctx = support::context();
    ctx.air = prism::air::Air::new(MemoryAir::default());
    let mut room = ctx.air.instances::<ChatRoom>().create(Id::from(31));
    for member in &names()[1..] {room.send(ChatRoomAction::Share(*member));}
    room.send(ChatRoomAction::RenameGroup("Friends".into()));
    let theme = Theme::default();
    let mut view = GroupMessageInfoPage::build(&mut ctx, room.clone()).build(&mut ctx, &theme);
    settle(&mut view, &mut ctx);
    let labels: Vec<_> = draw_page(&mut view).into_iter().filter_map(|i| {
        if let Item::Text(text) = i.item {Some(text.spans.iter().map(|s| s.text.clone()).collect::<String>())} else {None}
    }).collect();
    for label in ["Group info", "This group has 3 members.", "Add members", "Leave group"] {
        assert!(labels.iter().any(|text| text == label), "missing {label}: {labels:?}");
    }
    assert!(!labels.iter().any(|text| text == "Friends" || text == "Group message"));
    let PageType::EditAndDisplay {mut on_save, ..} = GroupMessageInfoPage::edit(room.clone()) else {panic!()};
    // Someone renames the group while this user changes only the photo.
    room.send(ChatRoomAction::RenameGroup("Remote name".into()));
    let photo = AvatarContent::image(media::Image::new((1, 1), vec![media::Pixel {
        red: 255, green: 32, blue: 32, alpha: 255,
    }]));
    on_save(&mut ctx, &vec![State::Avatar(photo.clone()), State::Text("Friends".into())]);
    assert_eq!(room.pending().group_details().name, "Remote name");
    assert_eq!(room.pending().group_details().photo, photo);
    ctx.events();
    let mut messages = MessagesPage::new(&mut ctx, &theme, room.clone(), 1);
    settle(&mut messages, &mut ctx);
    let drawing = draw_page(&mut messages);
    assert!(drawing.iter().any(|i| matches!(&i.item, Item::Text(t) if t.spans.iter().any(|s| s.text == "Remote name"))));
}

#[test]
fn leaving_requires_confirmation_and_stale_pages_cannot_send_or_edit() {
    use chk::{GroupMessageInfoPage, PageType, Bumper, State, AvatarContent};
    let mut ctx = support::context();
    ctx.air = prism::air::Air::new(MemoryAir::default());
    let mut room = ctx.air.instances::<ChatRoom>().create(Id::from(32));
    for member in &names()[1..] {room.send(ChatRoomAction::Share(*member));}
    let PageType::EditAndDisplay {mut on_save, ..} = GroupMessageInfoPage::edit(room.clone()) else {panic!()};
    let mut messages = MessagesPage::new(&mut ctx, &Theme::default(), room.clone(), 1);
    let PageType::Display {bumper: Bumper::Custom {action, ..}, ..} = GroupMessageInfoPage::leave(room.clone()) else {panic!()};
    assert!(room.pending().members.contains(&names()[0]), "opening the confirmation must not leave");
    (action.get())(&mut ctx, &Theme::default());
    assert!(!room.pending().members.contains(&names()[0]));
    assert!(ctx.events().iter().any(|event| matches!(
        event.downcast_ref::<pelican_ui::navigation::NavigationEvent>(),
        Some(pelican_ui::navigation::NavigationEvent::Root(None))
    )), "leaving must close all nested conversation screens");
    let original = room.pending().clone();
    on_save(&mut ctx, &vec![State::Avatar(AvatarContent::default()), State::Text("Stale edit".into())]);
    ctx.emit(pelican_ui::components::SendEvent::Photo(chk::messages::Photo::from_image(
        &media::Image::new((1, 1), vec![media::Pixel {red: 0, green: 0, blue: 0, alpha: 255}])
    ).unwrap()));
    settle(&mut messages, &mut ctx);
    assert_eq!(*room.pending(), original);
    let mut left_page = MessagesPage::new(&mut ctx, &Theme::default(), room, 1);
    settle(&mut left_page, &mut ctx);
    assert!(draw_page(&mut left_page).iter().any(|i| matches!(&i.item, Item::Text(t) if t.spans.iter().any(|s| s.text == "You left this group."))));
}

#[test]
fn member_picker_excludes_current_members_and_adds_selected_contacts() {
    use chk::{Form, FormItem, FormAction, GroupMessageInfoPage, State};
    let mut ctx = support::context();
    ctx.air = prism::air::Air::new(MemoryAir::default());
    let mut room = ctx.air.instances::<ChatRoom>().create(Id::from(33));
    room.send(ChatRoomAction::CreateGroup);
    room.send(ChatRoomAction::Share(names()[1]));
    for name in names() {chk::profiles::Profile::from_name(&mut ctx, name);}
    let Form::Flow {inputs, mut on_submit, ..} = GroupMessageInfoPage::add_members(&mut ctx, &Theme::default(), room.clone()) else {panic!()};
    let FormItem::Search(_, _, items, Some(actions)) = &inputs[0] else {panic!()};
    assert_eq!(items.iter().map(|(_, name)| *name).collect::<Vec<_>>(), vec![names()[2]]);
    assert!(actions.iter().any(|action| matches!(action, FormAction::Custom(label, ..) if label == "New contact")));
    on_submit(&mut ctx, &vec![State::Search(vec![names()[2]])]);
    assert_eq!(room.pending().members, names());
}

#[test]
fn membership_notices_are_centered_plain_text_and_previews_have_no_sender_prefix() {
    use chk::messages::{MessageContent, SystemMessage};
    use pelican_ui::components::MessageGroups;
    let mut ctx = support::context();
    ctx.air = prism::air::Air::new(MemoryAir::default());
    let [me, inviter, member] = names();
    let mut profile = chk::profiles::Profile::from_name(&mut ctx, member);
    profile.send(chk::profiles::ProfileAction::ChangeUsername("JadedFish".into()));
    let mut room = ChatRoom::init(Id::from(34), metadata(me, 1));
    room.apply(ChatRoomAction::CreateGroup, metadata(me, 2));
    room.apply(ChatRoomAction::Share(inviter), metadata(me, 3));
    room.apply(ChatRoomAction::AddMember(member), metadata(inviter, 4));
    room.apply(ChatRoomAction::LeaveGroup, metadata(member, 5));
    let notices = room.messages(me).cloned().collect::<Vec<_>>();
    assert_eq!(notices.len(), 2);
    assert!(matches!(notices[0].body, MessageContent::System(SystemMessage::MemberAdded(n)) if n == member));
    let mut added_me = notices[0].clone();
    added_me.body = MessageContent::System(SystemMessage::MemberAdded(me));
    let mut left_me = notices[1].clone();
    left_me.body = MessageContent::System(SystemMessage::MemberLeft(me));
    for (notice, expected) in [
        (&notices[0], "JadedFish was added to the group"),
        (&notices[1], "JadedFish left the group"),
        (&added_me, "You were added to the group"),
        (&left_me, "You left the group"),
    ] {
        assert_eq!(notice.preview(&mut ctx, true), expected);
        let pel = notice.to_pel(&mut ctx, |_, _| panic!("system messages must not be interactive"));
        let mut group = MessageGroups::new(&mut ctx, &Theme::default(), vec![pel], true, false);
        for width in [320.0, 768.0] {
            group.request_width();
            let (_, height) = group.request_height(width);
            let layout = group.layout((width, height));
            let drawing = group.draw(&layout, (0.0, 0.0), (0.0, 0.0, width, height));
            assert_eq!(drawing.len(), 1, "no avatar, bubble or timestamp on a system message");
            let Item::Text(text) = &drawing[0].item else {panic!("expected plain text")};
            assert_eq!(text.spans.iter().map(|span| span.text.as_str()).collect::<String>(), expected);
            assert_eq!(text.align, prism::drawable::text::Align::Center);
            assert!((text.width.unwrap() + drawing[0].offset.0 * 2.0 - width).abs() < 0.1);
        }
    }
    let plain = |text: &str| chk::messages::Message {
        body: MessageContent::Text(text.into()), author: me, timestamp: 6,
        hidden_from: vec![], id: Id::hash(text),
    };
    let timeline = [plain("Before"), notices[0].clone(), plain("After")].iter()
        .map(|message| message.to_pel(&mut ctx, |_, _| {})).collect();
    let mut timeline = MessageGroups::new(&mut ctx, &Theme::default(), timeline, true, false);
    let labels: Vec<_> = draw_page(&mut timeline).iter().filter_map(|i| {
        if let Item::Text(text) = &i.item {Some(text.spans.iter().map(|s| s.text.as_str()).collect::<String>())} else {None}
    }).collect();
    assert_eq!(labels.iter().filter(|label| *label == "You").count(), 2,
        "system messages must separate adjacent sender groups");
    let index = |label| labels.iter().position(|text| text == label).unwrap();
    assert!(index("Before") < index("JadedFish was added to the group"));
    assert!(index("JadedFish was added to the group") < index("After"));
}
