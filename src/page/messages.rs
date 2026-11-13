use crate::ContentSurface;
use prism::events::{DynEvent, OnEvent, on_event, OnChange, on_change, WindowEvent, ListenFuture, ListenCallback};
use prism::drawable::{Component, Drawable};
use prism::drawable::{LayoutTree, Instruction, Rect};
use crate::design_system::{Context, Callback};
use ptsd::layout::{Stack, Offset, Column};
use ptsd::display::Bin;
use crate::design_system::PageView;
use ptsd::navigation::AppPage;
use crate::design_system::{Theme, Icons};
use crate::design_system::{MessageGroups, SendEvent, TextInputEvent};
use ptsd::navigation::{NavigationEvent, NavigationChanged};
use crate::messages::{ChatRoom, Message, ChatRoomAction, MessageContent, RoomReadState};
use crate::profiles::Profile;
use crate::blocking::Block;
use crate::{Page, PageType, Bumper, ActionItem, ValueItem, PageBuilder};
use prism::hardware::Data;
use std::{cell::{Cell, RefCell}, rc::{Rc, Weak}, sync::Arc, collections::HashMap,
    future::{Future, poll_fn}, pin::Pin, task::{Poll, Waker}};
use crate::flow::Flow;
use crate::items::{Action, Display};


use prism::air::Instance;
use crate::typing_ui::{TypingPanel, TypingPublisher};

/// Drawing the whole page gates history work; content layout can draw children
/// while looking for focused inputs, before a real page frame has been built.
#[derive(Debug, Clone)]
pub struct MessagesPage {
    inner: MessagesPageInner,
}

impl MessagesPage {
    pub fn new(ctx: &mut Context, theme: &Theme, room: Instance<ChatRoom>, flow_len: usize) -> Self {
        Self {inner: MessagesPageInner::new(ctx, theme, room, flow_len)}
    }

    pub fn take_input_from(&mut self, old: &mut Self) {self.inner.take_input_from(&mut old.inner);}
}

impl AppPage for MessagesPage {}

impl Drawable for MessagesPage {
    fn request_width(&mut self) -> (f32, f32) {self.inner.request_width()}
    fn request_height(&mut self, width: f32) -> (f32, f32) {self.inner.request_height(width)}
    fn layout(&mut self, size: (f32, f32)) -> LayoutTree {
        let layout = Drawable::layout(&mut self.inner, size);
        if let Some((scroll, height)) = self.inner.scroll_anchor.take() {
            let growth = self.inner.history().map_or(0.0, |h| h.height.get() - height);
            if growth.is_finite() && growth > 0.0 {
                self.inner.page.content_mut().set_scroll(scroll + growth);
                return Drawable::layout(&mut self.inner, size);
            }
        }
        layout
    }
    fn draw(&self, layout: &LayoutTree, offset: (f32, f32), bound: Rect) -> Vec<Instruction> {
        if let Some(history) = self.inner.history() {history.near_top.set(false);}
        let instructions = self.inner.draw(layout, offset, bound);
        if bound.2 > 0.0 && bound.3 > 0.0 && (self.inner.loading.refresh
            || (self.inner.loading.more && self.inner.history().is_some_and(|h| h.near_top.get()))) {
            self.inner.loading.drawn();
        }
        instructions
    }
    fn event(&mut self, ctx: &mut Context, layout: &LayoutTree, event: Box<dyn DynEvent>) {
        self.inner.event(ctx, layout, event);
    }
    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            let callback = self.inner.changed().await;
            Box::new(move |target: &mut dyn std::any::Any, ctx: &mut Context| {
                callback(&mut target.downcast_mut::<Self>().unwrap().inner, ctx);
            }) as ListenCallback
        })
    }
}

#[derive(Debug)]
struct MessageLoading {
    drawn: Cell<bool>,
    waker: RefCell<Option<Waker>>,
    limit: usize,
    more: bool,
    refresh: bool,
}

impl MessageLoading {
    fn new() -> Self {
        Self {drawn: Cell::new(false), waker: RefCell::new(None), limit: 0, more: true, refresh: true}
    }

    fn drawn(&self) {
        self.drawn.set(true);
        let waker = self.waker.borrow_mut().take();
        if let Some(waker) = waker {waker.wake();}
    }

    async fn next(&mut self) {
        poll_fn(|cx| {
            if self.drawn.replace(false) {Poll::Ready(())}
            else {
                *self.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        }).await;
    }
}

impl Clone for MessageLoading {
    fn clone(&self) -> Self {
        // Hidden/cloned pages must wait for their own draw, not another page's.
        Self {drawn: Cell::new(false), waker: RefCell::new(None),
            limit: self.limit, more: self.more, refresh: true}
    }
}

#[derive(Debug, Component, Clone)]
struct MessagesPageInner {
    layout: Stack,
    page: PageView,
    #[skip] room: Instance<ChatRoom>,
    #[skip] is_blocked: bool,
    #[skip] reads: Instance<RoomReadState>,
    #[skip] displayed: Vec<prism::air::Id>,
    #[skip] acknowledge: bool,
    #[skip] focused: bool,
    #[skip] typing: TypingPublisher,
    #[skip] theme: Theme,
    #[skip] loading: MessageLoading,
    #[skip] scroll_anchor: Option<(f32, f32)>,
}

#[on_change(self.loading.next())]
impl OnChange<()> for MessagesPageInner {
    fn on_change(&mut self, ctx: &mut Context, _: ()) {self.load_messages(ctx);}
}

#[on_change(self.typing.next())]
impl OnChange<()> for MessagesPageInner {
    fn on_change(&mut self, _: &mut Context, _: ()) {self.typing.stop();}
}

#[on_event]
impl OnEvent<TextInputEvent> for MessagesPageInner {
    fn on(&mut self, ctx: &mut Context, event: TextInputEvent) -> Vec<Box<dyn DynEvent>> {
        if let Some((field, focused)) = self.page.composer_state() {
            match &event {
                TextInputEvent::Changed {id, value} if *id == field => {
                    if self.focused && !self.is_blocked && focused {
                        self.typing.edit(ctx, &self.room, !value.is_empty());
                    } else {self.typing.stop();}
                }
                TextInputEvent::Submit {id} if *id == field => self.typing.stop(),
                _ => {}
            }
        }
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<ptsd::events::TextInput> for MessagesPageInner {
    fn on(&mut self, _: &mut Context, event: ptsd::events::TextInput) -> Vec<Box<dyn DynEvent>> {
        if matches!(event, ptsd::events::TextInput::Focused(false)) {self.typing.stop();}
        prism::events![event]
    }
}

// Only the current page is polled. Constructing a flow for a list row must
// never mark its messages read; hidden and background pages do not acknowledge.
#[on_change(async {
    if !self.acknowledge {std::future::pending::<()>().await;}
})]
impl OnChange<()> for MessagesPageInner {
    fn on_change(&mut self, ctx: &mut Context, _: ()) {
        self.acknowledge = false;
        if !self.room.pending().members.contains(&ctx.air.me()) {return;}
        let unread = {
            let reads = self.reads.pending();
            self.displayed.iter().copied().filter(|id| !reads.read_flag(*id)).collect::<Vec<_>>()
        };
        if !unread.is_empty() {self.reads.send(unread);}
    }
}

#[on_event]
impl OnEvent<WindowEvent> for MessagesPageInner {
    fn on(&mut self, _: &mut Context, event: WindowEvent) -> Vec<Box<dyn DynEvent>> {
        if let WindowEvent::Focused(focused) = event {
            self.focused = focused;
            self.acknowledge = focused;
            if !focused {self.typing.stop();}
        }
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<NavigationChanged> for MessagesPageInner {
    fn on(&mut self, _: &mut Context, event: NavigationChanged) -> Vec<Box<dyn DynEvent>> {
        self.focused = true;
        self.acknowledge = true;
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<crate::design_system::MessageBarEvent> for MessagesPageInner {
    fn on(&mut self, ctx: &mut Context, event: crate::design_system::MessageBarEvent) -> Vec<Box<dyn DynEvent>> {
        if event == crate::design_system::MessageBarEvent::Capture {
            if self.is_blocked || !self.room.pending().members.contains(&ctx.air.me()) {return vec![];}
            self.typing.stop();
            let page = PageType::camera(|ctx: &mut Context, photo| {
                ctx.emit(crate::design_system::AttachmentEvent(SendEvent::Photo(photo)));
            });
            (Flow::new(vec![Page::Static(Box::new(page))]).build(ctx, &self.theme))(ctx, &self.theme);
            return vec![];
        }
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<SendEvent> for MessagesPageInner {
    fn on(&mut self, ctx: &mut Context, event: SendEvent) -> Vec<Box<dyn DynEvent>> {
        self.typing.stop();
        if self.is_blocked || !self.room.pending().members.contains(&ctx.air.me()) {return vec![];}
        match &event {
            SendEvent::Sticker(sticker) => {self.room.send(ChatRoomAction::SendSticker(sticker.clone()));},
            SendEvent::Gif(gif) => {self.room.send(ChatRoomAction::SendGif(gif.clone()));},
            SendEvent::Photo(photo) => {self.room.send(ChatRoomAction::SendPhoto(photo.clone()));},
            SendEvent::Video(video) => {
                let members = self.room.pending().members.clone();
                let video = video.share(ctx, &members);
                self.room.send(ChatRoomAction::SendVideo(video));
            }
        }
        prism::events![event]
    }
}


impl MessagesPageInner {
    fn history(&self) -> Option<&MessageHistory> {
        self.page.content().children().first()?.downcast_ref::<MessageHistory>()
    }

    fn load_messages(&mut self, ctx: &mut Context) {
        if !self.loading.refresh && (!self.loading.more || !self.history().is_some_and(|h| h.near_top.get())) {return;}
        // Show the latest message first; then fill the viewport and fetch older
        // history in small batches only as the reader approaches its top.
        let mut limit = if self.loading.limit == 0 {1}
            else if self.loading.refresh {self.loading.limit}
            else {self.loading.limit.saturating_add(16)};
        let me = ctx.air.me();
        let room = self.room.pending();
        let is_group = room.is_group();
        if self.loading.refresh
            && let Some(oldest) = self.history().and_then(|h| h.inner.1.first())
                .and_then(|g| g.messages.first()).map(|m| m.id)
            && let Some(index) = room.messages(me).rev().position(|m| m.id == oldest) {
            limit = limit.max(index.saturating_add(1));
        }
        let mut messages = room.messages(me).rev().take(limit.saturating_add(1)).cloned().collect::<Vec<_>>();
        self.loading.more = messages.len() > limit;
        messages.truncate(limit);
        messages.reverse();
        drop(room);
        let scroll = self.page.content().scroll();
        if self.loading.refresh && scroll > 0.0 && let Some(history) = self.history() {
            let previous = history.inner.1.iter().flat_map(|g| g.messages.iter()).map(|m| m.id).collect::<Vec<_>>();
            if !previous.is_empty() && messages.len() > previous.len()
                && messages.iter().take(previous.len()).map(|m| m.id).eq(previous) {
                self.scroll_anchor = Some((scroll, history.height.get()));
            }
        }
        self.displayed = messages.iter().filter(|m| m.author != me).map(|m| m.id).collect();
        let history = self.page.content_mut().children_mut().first_mut()
            .and_then(|child| child.downcast_mut::<MessageHistory>()).unwrap();
        history.replace(ctx, &self.theme, &self.room, messages, is_group);
        self.loading.limit = limit;
        self.loading.refresh = false;
        self.acknowledge = self.focused;
    }

    pub fn new(ctx: &mut Context, theme: &Theme, room: Instance<ChatRoom>, flow_len: usize ) -> Self {
        let room_data = room.pending();
        let my_name = ctx.air.me();
        let is_group = room_data.is_group();
        let has_left = !room_data.members.contains(&my_name);
        let reads = RoomReadState::open(ctx, room.id());

        let mut profiles = room_data.members.clone().into_iter()
            .map(|n| Profile::from_name(ctx, n))
            .filter(|p| p.pending().name != Some(my_name))
            .collect::<Vec<_>>();

        let did_i_block = if !is_group && profiles.len() == 1 {
            let profile = profiles[0].pending();
            Block::did_i_block(ctx, profile.name.unwrap())
                .then_some(profile.username.to_string())
        } else {None};

        let am_i_blocked = if !is_group && profiles.len() == 1 {
            let profile = profiles[0].pending();
            Block::am_i_blocked(ctx, profile.name.unwrap())
                .then_some(profile.username.to_string())
        } else {None};

        let is_blocked = has_left || did_i_block.is_some() || am_i_blocked.is_some();

        let deref_profiles = profiles.iter_mut()
            .map(|p| (*p.pending()).clone())
            .collect::<Vec<Profile>>();

        let display_profiles = deref_profiles.iter()
            .map(|p| p.to_view()).collect::<Vec<_>>();

        let info_room = room.clone();
        let info = Some(Box::new(move |ctx: &mut Context, theme: &Theme| {
            (Flow::new(vec![Page::chat_info(info_room.clone())]).build(ctx, theme))(ctx, theme);
        }) as Box<dyn Callback>);

        let title = room_data.name(ctx);
        let avatar = is_group.then(|| room_data.group_details().photo);
        let notice = if has_left {Some("You left this group.".to_string())}
            else if let Some(username) = am_i_blocked {Some(format!("{} blocked you.", username))}
            else {did_i_block.map(|username| format!("You blocked {}.", username))};
        let mut room_taken = room.clone();
        let on_send = move |_ctx: &mut Context, value: &mut String| {
            if !value.is_empty() {room_taken.send(ChatRoomAction::SendMessage(value.to_string()));}
        };

        let mut children: Vec<Box<dyn Drawable>> = vec![Box::new(MessageHistory::new())];
        if !is_blocked {children.push(Box::new(TypingPanel::new(ctx, theme, room.clone())));}
        let header = crate::design_system::Header::messaging(title, display_profiles, avatar, flow_len, info);
        let footer = match notice {
            Some(notice) => crate::design_system::Footer::alert(&notice),
            None => crate::design_system::Footer::input("Message...", on_send, |_: &mut Context, value: &mut String| value.is_empty()),
        };
        let page = theme.page(header, crate::design_system::Content::new(Offset::End, children, crate::PageType::always_valid()), Some(footer));

        drop(room_data);
        Self {
            layout: Stack::default(), page, room, is_blocked, reads, displayed: Vec::new(),
            acknowledge: true, focused: true, typing: TypingPublisher::default(), theme: theme.clone(),
            loading: MessageLoading::new(),
            scroll_anchor: None,
        }
    }

    pub fn take_input_from(&mut self, old: &mut Self) {
        if self.room.id() != old.room.id() {return;}
        self.focused = old.focused;
        self.acknowledge &= self.focused;
        if self.is_blocked {old.typing.stop();}
        else {self.typing = std::mem::take(&mut old.typing);}
        if !self.is_blocked && !old.is_blocked {self.page.take_footer_from(&mut old.page);}
        let scroll = old.page.content().scroll();
        // Keep the displayed history while refreshed room/profile data waits
        // for the next draw; incoming messages must not flash an empty room.
        std::mem::swap(&mut self.page.content_mut().children_mut()[0], &mut old.page.content_mut().children_mut()[0]);
        self.page.content_mut().set_scroll(scroll);
        self.loading.limit = old.loading.limit;
        self.loading.more = old.loading.more;
        self.displayed = std::mem::take(&mut old.displayed);
    }
}

#[derive(Debug, Clone)]
struct MessageHistory {
    inner: Bin<Column, Vec<MessageGroupView>>,
    near_top: Cell<bool>,
    height: Cell<f32>,
}

impl MessageHistory {
    fn new() -> Self {
        Self {inner: Bin(Column::center(24.0), Vec::new()), near_top: Cell::new(false), height: Cell::new(0.0)}
    }

    fn replace(&mut self, ctx: &mut Context, theme: &Theme, room: &Instance<ChatRoom>, messages: Vec<Message>, is_group: bool) {
        let mut previous = std::mem::take(&mut self.inner.1).into_iter()
            .map(|group| (group.messages.iter().map(|m| m.id).collect::<Vec<_>>(), group))
            .collect::<HashMap<_, _>>();
        let mut groups: Vec<Vec<Message>> = Vec::new();
        for message in messages {
            if groups.last().and_then(|g| g.last()).is_some_and(|last| MessageGroupView::adjacent(last, &message)) {
                groups.last_mut().unwrap().push(message);
            } else {groups.push(vec![message]);}
        }
        self.inner.1 = groups.into_iter().map(|messages| {
            let name = match &messages[0].body {
                MessageContent::System(notice) => notice.member(),
                _ => messages[0].author,
            };
            let profile = Profile::from_name(ctx, name).pending().to_view();
            let ids = messages.iter().map(|m| m.id).collect::<Vec<_>>();
            if let Some(old) = previous.remove(&ids)
                && old.messages == messages && old.profile == profile && old.is_group == is_group {return old;}
            MessageGroupView::new(ctx, theme, room, messages, profile, is_group)
        }).collect();
    }
}

impl Drawable for MessageHistory {
    fn request_width(&mut self) -> (f32, f32) {self.inner.request_width()}
    fn request_height(&mut self, width: f32) -> (f32, f32) {self.inner.request_height(width)}
    fn layout(&mut self, size: (f32, f32)) -> LayoutTree {
        self.height.set(size.1);
        Drawable::layout(&mut self.inner, size)
    }
    fn draw(&self, layout: &LayoutTree, offset: (f32, f32), bound: Rect) -> Vec<Instruction> {
        self.near_top.set(bound.1 - offset.1 < 256.0);
        self.inner.draw(layout, offset, bound)
    }
    fn event(&mut self, ctx: &mut Context, layout: &LayoutTree, event: Box<dyn DynEvent>) {
        self.inner.event(ctx, layout, event);
    }
    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            let callback = self.inner.changed().await;
            Box::new(move |target: &mut dyn std::any::Any, ctx: &mut Context| {
                callback(&mut target.downcast_mut::<Self>().unwrap().inner, ctx);
            }) as ListenCallback
        })
    }
}

#[derive(Debug, Component, Clone)]
struct MessageGroupView {
    layout: Stack,
    content: MessageGroups,
    #[skip] messages: Vec<Message>,
    #[skip] profile: crate::design_system::ProfileView,
    #[skip] is_group: bool,
}

impl MessageGroupView {
    fn adjacent(first: &Message, next: &Message) -> bool {
        !matches!(first.body, MessageContent::System(_)) && !matches!(next.body, MessageContent::System(_))
            && first.author == next.author && first.timestamp.abs_diff(next.timestamp) / 1_000_000_000 <= 60
    }

    fn new(ctx: &mut Context, theme: &Theme, room: &Instance<ChatRoom>, messages: Vec<Message>,
        profile: crate::design_system::ProfileView, is_group: bool) -> Self {
        let rendered = messages.iter().map(|message| {
            let room = room.clone();
            let tapped = message.clone();
            message.to_view(ctx, move |ctx: &mut Context, theme: &Theme| {
                let page = match &tapped.body {
                    MessageContent::Photo(photo) => Page::Static(Box::new(PageType::photo(photo.clone(), Some(MessageDetails::icon(room.clone(), tapped.clone()))))),
                    MessageContent::Video(video) => Page::Static(Box::new(PageType::video(video.clone(), Some(MessageDetails::icon(room.clone(), tapped.clone()))))),
                    _ => MessageDetails::new(room.clone(), tapped.clone()),
                };
                (Flow::new(vec![page]).build(ctx, theme))(ctx, theme);
            })
        }).collect();
        Self {layout: Stack::default(), content: theme.message_group(ctx, rendered, is_group),
            messages, profile, is_group}
    }
}

pub struct MessageDetails;
impl MessageDetails {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(room: Instance<ChatRoom>, message: Message) -> Page {
        Page::Refreshing(Box::new(MessageSave {
            room, message, pops: 1,
            state: Rc::new(RefCell::new(MessageSaveState {request: None, result: None, busy: false, wakers: Vec::new()})),
            profile: None,
        }))
    }

    pub fn icon(room: Instance<ChatRoom>, message: Message) -> (Icons, Action) {
        (Icons::Info, Action::deferred_flow(move |_: &mut Context, _: &Theme| {
            Flow::new(vec![Self::view(room.clone(), message.clone(), 2)])
        }))
    }

    pub fn view(room: Instance<ChatRoom>, message: Message, pops: usize) -> Page {
        Page::Refreshing(Box::new(MessageSave {
            room, message, pops,
            state: Rc::new(RefCell::new(MessageSaveState {request: None, result: None, busy: false, wakers: Vec::new()})),
            profile: None,
        }))
    }

    pub fn page(ctx: &mut Context, theme: &Theme, mut room: Instance<ChatRoom>, message: Message,
        pops: usize, save: Option<ActionItem>) -> PageType {
        let is_mine = message.author == ctx.air.me();
        let author = message.author;
        let sender_name = if is_mine {"You".to_string()}
            else {Profile::from_name(ctx, author).pending().username.clone()};
        let bubble = message.to_view(ctx, |_: &mut Context, _: &Theme| {});

        let m = message.clone();
        let delete = ActionItem::new(Action::custom(move |ctx: &mut Context, _: &Theme| {
            room.send(ChatRoomAction::DeleteMessage(m.clone()));
            (0..pops).for_each(|_| ctx.emit(NavigationEvent::Pop));
        }), "Delete", Icons::Delete);
        let timestamp = ptsd::utils::Timestamp::from_u64(message.timestamp);
        let mut actions = vec![delete];
        if let Some(copy) = Self::copy_action(ctx, &message.body) {actions.push(copy);}
        if let Some(save) = save {actions.push(save);}
        actions.push(ActionItem::new(Action::deferred_flow(move |ctx: &mut Context, _: &Theme| {
            let mut profile = Profile::from_name(ctx, author);
            Flow::new(vec![Page::profile(ctx, &mut profile)])
        }), "View profile", Icons::Profile));

        let title = match is_mine {
            true => "Sent message",
            false => "Received message",
        };

        PageType::display(title, vec![
            Display::messages(ctx, theme, vec![bubble]),
            Display::table("Info", vec![
                ValueItem::new("Sender", &sender_name),
                ValueItem::new("Time sent", &timestamp.time()),
                ValueItem::new("Date sent", &timestamp.date()),
            ], Some(actions)),
        ], None, Bumper::None, Offset::Start)
    }

    fn save_action(content: &MessageContent, state: Rc<RefCell<MessageSaveState>>) -> Option<ActionItem> {
        if matches!(content, MessageContent::Text(_) | MessageContent::System(_)) {return None;}
        let content = content.clone();
        let state = Rc::downgrade(&state);
        Some(ActionItem::new(Action::custom(move |ctx: &mut Context, theme: &Theme| {
            let Some(state) = state.upgrade() else {return;};
            if state.borrow().busy {return;}
            let request: SaveRequest = match &content {
                MessageContent::Photo(photo) => Box::pin(Self::export(Arc::from(photo.jpeg()), "jpg")),
                MessageContent::Sticker(sticker) => {
                    let image = sticker.image();
                    Box::pin(async move {Ok(Data::Image(image))})
                }
                MessageContent::Video(video) => {
                    let bytes = crate::messages::VideoClip::find(ctx, video.clip);
                    Box::pin(async move {
                        let bytes = bytes.ok_or("This video is still downloading. Try again when it finishes.")?;
                        Self::export(bytes, "mp4").await
                    })
                }
                MessageContent::Gif(gif) => {
                    let bytes = theme.gif_bytes(gif);
                    Box::pin(async move {
                        let bytes = bytes.await?;
                        let extension = Self::image_extension(&bytes)
                            .or_else(|| (bytes.get(4..8) == Some(b"ftyp")).then_some("mp4"))
                            .ok_or("This GIF's format cannot be saved to Photos.")?;
                        Self::export(bytes, extension).await
                    })
                }
                _ => return,
            };
            let wakers = {
                let mut state = state.borrow_mut();
                state.busy = true;
                state.request = Some((uuid::Uuid::new_v4(), crate::pending::Pending::new(request)));
                state.take_wakers()
            };
            for waker in wakers {waker.wake();}
        }), "Share", Icons::Send))
    }

    pub fn image_extension(bytes: &[u8]) -> Option<&'static str> {
        if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {Some("gif")}
        else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {Some("png")}
        else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {Some("jpg")}
        else {None}
    }

    async fn export(bytes: Arc<[u8]>, extension: &'static str) -> Result<Data, String> {
        crate::design_system::Task::spawn(move || {
            use std::io::Write;
            // Keep the original encoding, including GIF animation and video audio.
            // The system owns temporary-directory cleanup; the share sheet may
            // still be reading the file after the detail page has been closed.
            let path = std::env::temp_dir().join(format!("message-{}.{}", uuid::Uuid::new_v4(), extension));
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)
                .map_err(|_| "Could not prepare this media for saving.".to_string())?;
            file.write_all(&bytes).map_err(|_| "Could not prepare this media for saving.".to_string())?;
            Ok(Data::File(path))
        }).next().await.ok_or_else(|| "Could not prepare this media for saving.".to_string())?
    }

    pub fn copy_action(ctx: &mut Context, content: &MessageContent) -> Option<ActionItem> {
        let action = match content {
            MessageContent::Text(text) => Action::copy(text),
            MessageContent::Gif(gif) => Action::copy(&gif.url),
            MessageContent::Video(_) => return None,
            MessageContent::Photo(photo) => {
                let photo = photo.clone();
                Action::custom(move |ctx: &mut Context, _: &Theme| {
                    ctx.hardware.set_clipboard(&prism::hardware::Data::Image(photo.image()));
                })
            }
            MessageContent::Sticker(sticker) => {
                let sticker = sticker.clone();
                Action::custom(move |ctx: &mut Context, _: &Theme| {
                    sticker.copy(ctx);
                })
            }
            MessageContent::System(notice) => {
                let profile = Profile::from_name(ctx, notice.member());
                let text = notice.text_for(ctx.air.me(), &profile.pending().username);
                Action::copy(&text)
            }
        };
        Some(ActionItem::new_with_active(action, "Copy", "copied", Icons::Copy))
    }
}

type SaveRequest = Pin<Box<dyn Future<Output = Result<Data, String>>>>;
type SaveOperation = crate::pending::Pending<Result<Data, String>>;

struct MessageSaveState {
    request: Option<(uuid::Uuid, SaveOperation)>,
    result: Option<Result<Data, String>>,
    busy: bool,
    wakers: Vec<Weak<RefCell<Option<Waker>>>>,
}

impl MessageSaveState {
    fn take_wakers(&mut self) -> Vec<Waker> {
        let mut wakers = Vec::new();
        self.wakers.retain(|slot| {
            let Some(slot) = slot.upgrade() else {return false;};
            if let Some(waker) = slot.borrow_mut().take() {wakers.push(waker);}
            true
        });
        wakers
    }
}

#[derive(Clone)]
struct MessageSave {
    room: Instance<ChatRoom>,
    message: Message,
    pops: usize,
    state: Rc<RefCell<MessageSaveState>>,
    profile: Option<Instance<Profile>>,
}

impl std::fmt::Debug for MessageSave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageSave").field("busy", &self.state.borrow().busy).finish()
    }
}

impl PageBuilder for MessageSave {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<()>> + '_>> {
        Box::pin(async {
            let state = &self.state;
            let slot = Rc::new(RefCell::new(None));
            {
                let mut state = state.borrow_mut();
                state.wakers.retain(|slot| slot.strong_count() > 0);
                state.wakers.push(Rc::downgrade(&slot));
            }
            let mut request: Option<(uuid::Uuid, Pin<Box<SaveOperation>>)> = None;
            let mut profile = Box::pin(async {
                let Some(profile) = &mut self.profile else {return std::future::pending().await;};
                profile.next().await;
                Some(())
            });
            poll_fn(|cx| {
                *slot.borrow_mut() = Some(cx.waker().clone());
                let mut state = state.borrow_mut();
                if state.result.is_some() {return Poll::Ready(Some(()));}
                match &state.request {
                    Some((id, pending)) if request.as_ref().is_none_or(|(current, _)| current != id) => {
                        request = Some((*id, Box::pin(pending.clone())));
                    }
                    None => request = None,
                    _ => {}
                }
                if let Some((_, request)) = &mut request
                    && let Poll::Ready(result) = request.as_mut().poll(cx) {
                    state.request = None;
                    state.result = Some(result);
                    let wakers = state.take_wakers();
                    drop(state);
                    for waker in wakers {waker.wake();}
                    return Poll::Ready(Some(()));
                }
                drop(state);
                profile.as_mut().poll(cx)
            }).await
        })
    }
    fn build(&mut self, ctx: &mut Context, theme: &Theme) -> PageType {
        let profile = self.profile.get_or_insert_with(|| Profile::from_name(ctx, self.message.author));
        drop(profile.pending());
        let result = {
            let mut state = self.state.borrow_mut();
            let result = state.result.take();
            if result.is_some() {state.busy = false;}
            result
        };
        let error = match result {
            Some(Ok(data)) => {ctx.hardware.share(&data); None}
            Some(Err(error)) => Some(error),
            None => None,
        };
        let save = MessageDetails::save_action(&self.message.body, self.state.clone());
        let mut page = MessageDetails::page(ctx, theme, self.room.clone(), self.message.clone(), self.pops, save);
        if let (Some(error), PageType::Content {items, ..}) = (error, &mut page) {
            items.push(crate::Item::Display(Display::instructions(&error)));
        }
        page
    }
}
