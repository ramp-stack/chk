use prism::air::{Id, Instance, Name};
use prism::drawable::{Drawable, Instruction, LayoutTree, Rect, text::Align};
use prism::events::{ChildEvent, DynEvent, Event, ListenCallback, ListenFuture, WindowEvent};
use prism::hardware::Data;
use prism::Context;
use ptsd::layout::Offset;
use ptsd::navigation::{AppPage, NavigationChanged, NavigationEvent};
use ptsd::utils::ValidationFn;
use ptsd::Validate;
use uuid::Uuid;

use std::{
    any::Any, cell::{Cell, RefCell}, collections::HashMap, future::{Future, poll_fn},
    ops::Range, pin::Pin, rc::Rc, sync::Arc, task::Poll, time::Duration,
};

use crate::blocking::Block;
use crate::closure::{FormSubmit, NavFn, ReviewItemGetter, SuccessGetter};
use crate::design_system::{
    AvatarContent, AvatarIconStyle, Callback, Content, Footer, FormEvent, Header, Icons,
    MessageBarEvent, PageView, QrCodeFound, SendEvent, TextInputEvent, TextSize, TextStyle, Theme,
};
use crate::form::{FormItem, FormValues, Submission, SubmitFinished, SubmitRequest};
use crate::items::{Action, Display, Input, Item};
use crate::messages::{
    ChatRoom, ChatRoomAction, Message, MessageContent, Photo, RoomReadState, Video,
};
use crate::profiles::{Profile, ProfileAction};
use crate::typing_ui::TypingPublisher;
use crate::{
    ActionItem, AvatarPurpose, CameraCapture, ContactInvitations, ContentSurface, ContractWatch,
    Flow, Form, FormComplete, FormValidState, ListItem, ListWatch, Page, PhotoCaptured, StackPage,
    State, VideoPlayback, Watcher,
};

type MessageLists = (Vec<Instance<ChatRoom>>, Vec<Instance<Profile>>, Vec<Instance<RoomReadState>>);
type SaveRequest = Pin<Box<dyn Future<Output = Result<Data, String>>>>;

#[derive(Debug, Clone)]
pub struct DeferredSubmit(pub crate::PendingSubmit);
impl Event for DeferredSubmit {
    fn pass(&self, _: &mut Context, _: &mut [ChildEvent<'_, Self>]) {}
}

#[derive(Debug, Clone)]
pub enum Bumper {
    Default,
    Custom { label: String, action: Action, secondary: Option<(String, Action)> },
    Done, None,
}

impl Bumper {
    pub fn custom(label: &str, action: Action) -> Self {
        Bumper::Custom { label: label.to_string(), action, secondary: None }
    }

    pub fn double(l1: &str, a1: Action, l2: &str, a2: Action) -> Self {
        Bumper::Custom { label: l1.to_string(), action: a1, secondary: Some((l2.to_string(), a2)) }
    }
}

#[derive(Debug, Clone)]
pub enum SearchSource {
    Contacts(ListWatch<Profile>),
    Messages((ListWatch<ChatRoom>, ListWatch<Profile>, ListWatch<RoomReadState>)),
}
impl SearchSource {
    pub fn label(&self) -> &'static str {
        match self {SearchSource::Contacts(_) => "Contacts", SearchSource::Messages(_) => "Messages"}
    }
}
impl Watcher for SearchSource {
    type Output = MessageLists;

    async fn next(&mut self) -> Option<()> {
        match self {
            SearchSource::Contacts(watch) => watch.next().await,
            SearchSource::Messages(watch) => watch.next().await,
        }
    }
    fn get(&mut self, ctx: &mut Context) -> MessageLists {
        match self {
            SearchSource::Contacts(watch) => (vec![], watch.get(ctx), vec![]),
            SearchSource::Messages(watch) => watch.get(ctx),
        }
    }
}

#[derive(Clone, Debug)]
pub enum PageType {
    Root {
        title: String,
        input: Vec<Input>,
        display: Vec<Display>,
        header: Option<(Icons, Action)>,
        bumper_a: Option<(String, Action)>,
        bumper_b: Option<(String, Action)>,
    },
    Edit {
        title: String,
        header: Option<(Icons, Action)>,
        items: Vec<FormItem>,
        display: Vec<Display>,
        on_save: Box<dyn FormSubmit>,
        flow_len: usize,
    },
    Content {
        title: String,
        items: Vec<Item>,
        offset: Offset,
        header: Option<(Icons, Action)>,
        bumper: Bumper,
        next: Option<NavFn>,
        flow_len: usize,
    },
    Form {
        id: String,
        title: String,
        item: Input,
        flow_len: usize,
        next: Option<NavFn>,
        validate: Box<dyn ValidationFn>,
        on_submit: Option<Box<dyn FormSubmit>>,
    },
    Review {
        title: String,
        getter: Box<dyn ReviewItemGetter>,
        flow_len: usize,
        next: Option<NavFn>,
        on_submit: Box<dyn FormSubmit>,
    },
    Success {
        title: String,
        getter: Box<dyn SuccessGetter>,
        flow_len: usize,
        on_submit: Option<Box<dyn FormSubmit>>,
    },
    Search { source: SearchSource },
    EdgeToEdge { header: Header, content: Box<dyn Drawable> },
    Messaging { room: Instance<ChatRoom>, flow_len: usize, target: Option<Id> },
    MessageDetails { room: Instance<ChatRoom>, message: Message, pops: usize },
    Camera { on_capture: Box<dyn PhotoCaptured> },
    Photo { photo: Photo, header: Option<(Icons, Action)> },
    Video { video: Video, header: Option<(Icons, Action)> },
    Playback { title: String, player: prism::hardware::Player, size: (u32, u32) },
}

impl PageType {
    pub fn root(
        title: &str, input: Vec<Input>, display: Vec<Display>, header: Option<(Icons, Action)>,
        bumper_a: Option<(String, Action)>, bumper_b: Option<(String, Action)>,
    ) -> Self {
        PageType::Root { title: title.to_string(), input, display, header, bumper_a, bumper_b }
    }

    pub fn display_and_input(title: &str, inputs: Vec<Input>, display: Vec<Display>, header: Option<(Icons, Action)>, bumper: Bumper) -> Self {
        PageType::Content {
            title: title.into(),
            items: inputs.into_iter().map(Item::Input)
                .chain(display.into_iter().map(Item::Display)).collect(),
            header, bumper, offset: Offset::Start, flow_len: 1, next: None,
        }
    }

    pub fn content(title: &str, items: Vec<Item>, header: Option<(Icons, Action)>, bumper: Bumper, offset: Offset) -> Self {
        PageType::Content {
            title: title.into(), items, header, bumper, offset, flow_len: 1, next: None,
        }
    }

    pub fn display(title: &str, items: Vec<Display>, header: Option<(Icons, Action)>, bumper: Bumper, offset: Offset) -> Self {
        PageType::Content {
            title: title.into(), items: items.into_iter().map(Item::Display).collect(),
            header, bumper, offset, flow_len: 1, next: None,
        }
    }

    pub fn input(title: &str, item: Input, header: Option<(Icons, Action)>, bumper: Bumper) -> Self {
        let offset = item.offset();
        PageType::Content {
            title: title.into(), items: vec![Item::Input(item)], header, bumper,
            offset, flow_len: 1, next: None,
        }
    }

    pub fn form(id: &str, title: &str, item: Input, validate: Box<dyn ValidationFn>, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        PageType::Form {
            id: id.into(),
            title: title.to_string(),
            validate, item,
            flow_len: 1,
            next: None,
            on_submit,
        }
    }

    pub fn edit(title: &str, header: Option<(Icons, Action)>, items: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        PageType::Edit {
            title: title.to_string(),
            header, items, display, on_save,
            flow_len: 1,
        }
    }

    pub fn review(title: &str, getter: Box<dyn ReviewItemGetter>, on_submit: Box<dyn FormSubmit>) -> Self {
        PageType::Review { title: title.to_string(), getter, flow_len: 1, next: None, on_submit }
    }

    pub fn success(title: &str, getter: Box<dyn SuccessGetter>, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        PageType::Success { title: title.to_string(), getter, flow_len: 1, on_submit }
    }

    pub fn messaging(room: Instance<ChatRoom>) -> Self {
        PageType::Messaging { room, flow_len: 1, target: None }
    }

    pub fn edge_to_edge(title: &str, content: impl Drawable, action: Option<(Icons, Action)>) -> Self {
        PageType::EdgeToEdge {
            header: Header::stack(title, action.map(|(icon, action)| (icon, action.get()))),
            content: Box::new(content),
        }
    }

    pub fn camera(on_capture: impl PhotoCaptured + 'static) -> Self {
        PageType::Camera {on_capture: Box::new(on_capture)}
    }

    pub fn photo(photo: Photo, header: Option<(Icons, Action)>) -> Self {
        PageType::Photo { photo, header }
    }

    pub fn video(video: Video, header: Option<(Icons, Action)>) -> Self {
        PageType::Video { video, header }
    }

    /// Reuse CHK's video page with an application-prepared playback timeline.
    pub fn playback(title: &str, player: prism::hardware::Player, size: (u32, u32)) -> Self {
        PageType::Playback { title: title.into(), player, size }
    }

    pub fn scan_qr(instructions: &str, alt: Option<(String, Icons, Action)>, on_find: Box<dyn QrCodeFound>) -> Self {
        let item = Input::qr_code_scanner(instructions, alt, on_find);
        let offset = item.offset();
        PageType::Content {
            title: "Scan QR code".into(), items: vec![Item::Input(item)], header: None,
            bumper: Bumper::None, offset, flow_len: 1, next: None,
        }
    }

    pub fn display_qr_code(title: &str, data: &str, instructions: &str, share: bool) -> Self {
        PageType::Content {
            title: title.into(), items: vec![Item::Display(Display::qr_code(data, instructions))],
            header: None,
            bumper: Bumper::custom(
                if share { "Share" } else { "Copy" },
                if share { Action::share(data) } else { Action::copy(data) },
            ), offset: Offset::Center, flow_len: 1, next: None,
        }
    }

    pub fn qr_page(invitations: &ContactInvitations, name: Name) -> PageType {
        PageType::Content {
            title: "Share AirName".into(),
            items: vec![Item::Display(Display::qr_code(
                &name.to_string(), "Scan to share this AirName.",
            ))],
            header: None, bumper: Bumper::custom("Share", Action::share_contact(invitations, name)),
            offset: Offset::Center, flow_len: 1, next: None,
        }
    }

    pub fn chat_info(ctx: &mut Context, room: Instance<ChatRoom>) -> PageType {
        if room.pending().is_group() {
            let active = room.pending().members.contains(&ctx.air.me());
            let items = Display::group_info(ctx, room.clone()).into_iter()
                .map(Item::Display).collect();
            let header = active.then(|| (Icons::Edit, Action::deferred_flow(
                move |_: &mut Context, _: &Theme| {
                    Flow::new(vec![Page::Static(Box::new(PageType::edit_group(room.clone())))])
                },
            )));
            return PageType::Content {
                title: "Group info".into(), items, header,
                bumper: Bumper::None, offset: Offset::Start, flow_len: 1, next: None,
            };
        }
        let me = ctx.air.me();
        let name = room.pending().members.iter().copied().find(|name| *name != me).unwrap_or(me);
        let profile = Profile::from_name(ctx, name);
        if name == me {
            let profile = profile.pending();
            return PageType::Content {
                title: "Chat info".into(),
                items: vec![
                    Item::Display(Display::avatar(profile.avatar.clone(), AvatarPurpose::None)),
                    Item::Display(Display::label(&format!("{} (You)", profile.username))),
                    Item::Display(Display::actions(vec![ActionItem::mute(ctx, room)], true)),
                ],
                header: None, bumper: Bumper::None, offset: Offset::Start, flow_len: 1, next: None,
            };
        }
        let title = profile.pending().username.clone();
        let items = Display::profile(ctx, profile.clone())
            .into_iter().map(Item::Display).collect();
        let header = Some((Icons::Edit, Action::deferred_flow(move |ctx: &mut Context, _: &Theme| {
            Flow::new(vec![Page::Static(Box::new(PageType::editing(
                false, profile.clone(), ctx.window.is_mobile, None,
            )))])
        })));
        PageType::Content {
            title, items, header, bumper: Bumper::None,
            offset: Offset::Start, flow_len: 1, next: None,
        }
    }

    pub fn edit_group(mut room: Instance<ChatRoom>) -> PageType {
        let original = room.pending().group_details();
        let photo = original.photo.clone();
        let name = original.name.clone();
        let save = Box::new(move |ctx: &mut Context, values: &FormValues| {
            if !room.pending().members.contains(&ctx.air.me()) {
                return FormComplete::Error("You are no longer a member of this group.".into());
            }
            if let Some(photo) = values.avatar("photo") && *photo != original.photo {
                room.send(ChatRoomAction::SetGroupPhoto(photo.clone()));
            }
            if let Some(name) = values.text("name") && name.trim() != original.name {
                room.send(ChatRoomAction::RenameGroup(name.to_owned()));
            }
            FormComplete::Action(Action::custom(|ctx: &mut Context, _: &Theme| {
                ctx.emit(NavigationEvent::Pop);
            }))
        }) as Box<dyn FormSubmit>;
        PageType::Edit {
            title: "Edit group".into(), header: None,
            items: vec![
                FormItem::avatar_with_preset("Group photo", photo, Action::PickPhoto,
                    |_: &mut Context, _: String| FormValidState::Valid).named("photo"),
                FormItem::text_with_preset("Group name", &name, None, |_: &mut Context, name: String| {
                    if name.trim().is_empty() {
                        FormValidState::InvalidWithData("Enter a group name.".into())
                    } else if name.trim().chars().count() > 80 {
                        FormValidState::InvalidWithData("Use 80 characters or fewer.".into())
                    } else {FormValidState::Valid}
                }).named("name"),
            ],
            display: vec![], on_save: save, flow_len: 1,
        }
    }

    pub fn leave(mut room: Instance<ChatRoom>) -> PageType {
        let details = room.pending().group_details();
        PageType::Content {
            title: "Leave group".into(),
            items: vec![
                Item::Display(Display::avatar(details.photo, AvatarPurpose::Custom {
                    icon: Icons::Leave, style: AvatarIconStyle::Danger,
                })),
                Item::Display(Display::confirmation_message(&format!("Leave {}?", details.name))),
            ],
            header: None,
            bumper: Bumper::double("Leave", Action::custom(move |ctx: &mut Context, _: &Theme| {
                room.send(ChatRoomAction::LeaveGroup);
                room.notify(false);
                ctx.emit(NavigationEvent::Root(None));
            }), "Cancel", Action::None),
            offset: Offset::Center, flow_len: 1, next: None,
        }
    }

    pub fn profile_view(ctx: &mut Context, profile: Instance<Profile>) -> PageType {
        let name = profile.pending().name.unwrap();
        let title = profile.pending().username.clone();
        let is_me = name == ctx.air.me();
        let items = Display::profile(ctx, profile.clone())
            .into_iter().map(Item::Display).collect();
        let header = Some((Icons::Edit, Action::deferred_flow(move |ctx: &mut Context, _: &Theme| {
            Flow::new(vec![Page::Static(Box::new(PageType::editing(
                is_me, profile.clone(), ctx.window.is_mobile, None,
            )))])
        })));
        PageType::Content {
            title, items, header, bumper: Bumper::None,
            offset: Offset::Start, flow_len: 1, next: None,
        }
    }

    pub fn editing(is_me: bool, profile: Instance<Profile>, is_mobile: bool, contact_actions: Option<Vec<ActionItem>>) -> PageType {
        let mut p = profile.clone();
        let closure = Box::new(move |_ctx: &mut Context, objects: &FormValues| {
            if let Some(result) = objects.text("username") {
                p.send(ProfileAction::ChangeUsername(result.to_string()));
            }
            if let Some(result) = objects.text("about") {
                p.send(ProfileAction::ChangeNotes(result.to_string()));
            }
            if let Some(result) = objects.avatar("avatar") {
                p.send(ProfileAction::ChangeAvatar(result.clone()));
            }
            FormComplete::None
        }) as Box<dyn FormSubmit>;

        let p = (*profile.pending()).clone();
        let my_name = p.name.unwrap();
        let display = if is_me {
            vec![Display::cta(
                "AirName", None, &my_name.to_string(),
                contact_actions.unwrap_or_else(|| vec![
                    ActionItem::new_with_active(
                        Action::copy(&my_name.to_string()), "Copy", "Copied", Icons::Copy,
                    ),
                    ActionItem::new(
                        Action::flow(Flow::new(vec![Page::Static(Box::new(PageType::display_qr_code(
                            "Share AirName", &my_name.to_string(), "Scan to share AirName.",
                            is_mobile,
                        )))])), "Display QR Code", Icons::QrCode,
                    ),
                ]),
            )]
        } else {
            vec![]
        };

        PageType::Edit {
            title: if is_me {"My profile"} else {"Edit profile"}.into(),
            header: None,
            items: vec![
                FormItem::avatar_with_preset(
                    "Avatar", p.avatar.clone(), Action::PickPhoto,
                    |_ctx: &mut Context, _: String| FormValidState::Valid,
                ).named("avatar"),
                FormItem::text_with_preset(
                    "Username", &p.username, None,
                    |_ctx: &mut Context, value: String| {
                        if value.trim().is_empty() {
                            FormValidState::InvalidWithData("Username cannot be empty".to_string())
                        } else {
                            FormValidState::Valid
                        }
                    },
                ).named("username"),
                FormItem::text_with_preset(
                    "About me", &p.notes, None,
                    |_ctx: &mut Context, _: String| FormValidState::Valid,
                ).named("about"),
            ],
            display, on_save: closure, flow_len: 1,
        }
    }

    pub fn nav_fn(&mut self) -> Option<&mut Option<NavFn>> {
        match self {
            PageType::Root { .. }
            | PageType::Messaging { .. }
            | PageType::MessageDetails { .. }
            | PageType::Search { .. }
            | PageType::EdgeToEdge { .. }
            | PageType::Camera { .. }
            | PageType::Photo { .. }
            | PageType::Video { .. }
            | PageType::Playback { .. }
            | PageType::Edit { .. }
            | PageType::Success { .. } => None,
            PageType::Content { next, .. }
            | PageType::Form { next, .. }
            | PageType::Review { next, .. } => Some(next),
        }
    }

    pub fn length(&mut self) -> Option<&mut usize> {
        match self {
            PageType::Messaging { .. }
            | PageType::MessageDetails { .. }
            | PageType::Search { .. }
            | PageType::EdgeToEdge { .. }
            | PageType::Camera { .. }
            | PageType::Photo { .. }
            | PageType::Video { .. }
            | PageType::Playback { .. }
            | PageType::Root { .. } => None,
            PageType::Edit { flow_len, .. }
            | PageType::Content { flow_len, .. }
            | PageType::Form { flow_len, .. }
            | PageType::Success { flow_len, .. }
            | PageType::Review { flow_len, .. } => Some(flow_len),
        }
    }

    pub fn on_submit(&mut self) -> Option<&mut Box<dyn FormSubmit>> {
        match self {
            PageType::Edit { on_save, .. } => Some(on_save),
            PageType::Form { on_submit, .. } => on_submit.as_mut(),
            PageType::Review { on_submit, .. } => Some(on_submit),
            PageType::Success { on_submit, .. } => on_submit.as_mut(),
            _ => None,
        }
    }

    pub fn update(&mut self, new_len: usize, new_fn: Option<NavFn>) {
        if let Some(l) = self.length() { *l = new_len; }
        if let Some(nav) = self.nav_fn() { *nav = new_fn.clone(); }
    }

    pub fn build(&self, ctx: &mut Context, theme: &Theme) -> Box<dyn AppPage> {
        match self {
            PageType::Root { .. } => self.build_root(ctx, theme),
            PageType::Content { .. } => Box::new(self.build_content(ctx, theme)),
            PageType::Form { id, title, item, next, flow_len: _, validate, on_submit } => {
                let content = Content::new(Offset::Start,
                    item.build(theme, ctx.window.is_mobile).unwrap_or_default(), validate.clone());
                Box::new(PageType::build_form(
                    ctx, theme, id.clone(), title, content, next.clone(), on_submit.clone(),
                ))
            }
            PageType::Edit { .. } => Box::new(self.build_edit(ctx, theme, false)),
            PageType::Review { title, getter, next, on_submit, .. } => {
                Box::new(PageType::build_review(
                    ctx, theme, title, getter.clone(), next.clone(), on_submit.clone(),
                ))
            }
            PageType::Success { title, getter, flow_len, on_submit } => Box::new(PageType::build_success(
                ctx, theme, title, getter.clone(), *flow_len, on_submit.clone(),
            )),
            PageType::Messaging { room, flow_len, target } => {
                Box::new(PageType::build_messaging(ctx, theme, room.clone(), *flow_len, *target))
            }
            PageType::MessageDetails { room, message, pops } => Box::new(PageType::build_message_details(
                ctx, theme, room.clone(), message.clone(), *pops,
            )),
            PageType::Search { source } => Box::new(PageType::build_search(ctx, theme, source.clone())),
            PageType::EdgeToEdge { header, content } => {
                Box::new(StackPage::edge_to_edge(theme, header.clone(), content.clone()))
            }
            PageType::Camera { on_capture } => Box::new(StackPage::edge_to_edge(
                theme, Header::stack("Camera", None),
                Box::new(CameraCapture::new(theme, on_capture.clone())),
            )),
            PageType::Photo { photo, header } => Box::new(StackPage::edge_to_edge(
                theme, Header::stack("Photo", header.clone().map(|(icon, action)| (icon, action.get()))),
                theme.photo_view(photo),
            )),
            PageType::Video { video, header } => Box::new(StackPage::edge_to_edge(
                theme, Header::stack("Video", header.clone().map(|(icon, action)| (icon, action.get()))),
                Box::new(VideoPlayback::new(ctx, theme, video.clone())),
            )),
            PageType::Playback { title, player, size } => Box::new(StackPage::edge_to_edge(
                theme, Header::stack(title, None),
                Box::new(VideoPlayback::from_player(theme, player.clone(), *size)),
            )),
        }
    }

    pub fn build_content(&self, ctx: &mut Context, theme: &Theme) -> StackPage<PageView> {
        let PageType::Content {title, items, offset, header, bumper, next, flow_len} = self.clone()
            else {unreachable!()};
        let (header, footer) = {
            let title = title.as_str();
            let icon = header.map(|(icon, action)| (icon, action.get()));
            match bumper {
                Bumper::Custom { label, action, secondary } => {
                    let on_click = match next {
                        Some(n) => {
                            let next = n.clone();
                            Box::new(move |ctx: &mut Context, theme: &Theme| {
                                (next.borrow_mut())(ctx, theme);
                                (action.clone().get())(ctx, theme);
                            }) as Box<dyn Callback>
                        }
                        None => Box::new(move |ctx: &mut Context, theme: &Theme| {
                            (action.clone().get())(ctx, theme)
                        }) as Box<dyn Callback>,
                    };
                    let secondary = secondary.clone().map(|(l, a)| {
                        (
                            ActionItem::format_label(&l),
                            Box::new(move |ctx: &mut Context, theme: &Theme| {
                                (a.clone().get())(ctx, theme);
                                (0..1).for_each(|_| {
                                    ctx.emit(NavigationEvent::Pop)
                                })
                            }) as Box<dyn Callback>,
                        )
                    });
                    (Header::stack(title, icon), Some(Footer::stack(
                        Some(&ActionItem::format_label(&label)), on_click, secondary,
                    )))
                }
                Bumper::Default => match next {
                    Some(n) => {
                        let next = n.clone();
                        (Header::stack(title, icon), Some(Footer::stack(None,
                            Box::new(move |ctx: &mut Context, theme: &Theme| {
                                (next.borrow_mut())(ctx, theme);
                            }), None)))
                    }
                    None => (Header::stack_end(title), Some(Footer::stack_end(Some(flow_len)))),
                },
                Bumper::Done => (Header::stack_end(title), Some(Footer::stack_end(Some(flow_len)))),
                Bumper::None => (Header::stack(title, icon), None),
            }
        };
        StackPage::content(ctx, theme, header, items, offset, footer, None, None)
    }

    pub fn build_search(ctx: &mut Context, theme: &Theme, source: SearchSource) -> StackPage<PageView> {
        let input_id = Uuid::new_v4();
        let label = source.label().to_lowercase();
        let input = Item::Input(Input::Text {
            id: input_id, label: source.label().into(), actions: None, show_label: false,
            preset: None, placeholder: format!("Search {label}..."),
        });
        let contacts = matches!(&source, SearchSource::Contacts(_));
        let entries = Rc::new(RefCell::new((Vec::new(), Vec::new())));
        let current = entries.clone();
        let style = theme.clone();
        let on_create: ListenCallback<PageView> = Box::new(move |page, ctx| {
            page.watch(ctx, source, move |ctx, page, (rooms, profiles, reads)| {
                let mut entries = current.borrow_mut();
                *entries = if contacts {(SearchEntry::contacts(ctx, profiles), vec![])} else {
                    let history = SearchMessage::history(ctx, &rooms, &profiles);
                    (SearchEntry::messages(ctx, rooms, profiles, reads), history)
                };
                let query = page.content().children().first()
                    .and_then(|input| Input::state(&**input));
                let query = match &query {Some(State::Text(query)) => query.as_str(), _ => ""};
                let results = SearchEntry::filter(&entries.0, &entries.1, contacts, query);
                let children = page.content_mut().children_mut();
                children.truncate(1);
                children.extend(results.into_iter()
                    .flat_map(|mut item| item.build(ctx, &style).unwrap_or_default()));
            });
        });
        let event_theme = theme.clone();
        let mut focus_on_open = true;
        let mut query = String::new();
        let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
            if let Some(TextInputEvent::Changed {id, value} | TextInputEvent::Set {id, value}) =
                event.downcast_ref() && *id == input_id && *value != query {
                query.clone_from(value);
                let entries = entries.borrow();
                let results = SearchEntry::filter(&entries.0, &entries.1, contacts, &query);
                let children = page.content_mut().children_mut();
                children.truncate(1);
                children.extend(results.into_iter()
                    .flat_map(|mut item| item.build(ctx, &event_theme).unwrap_or_default()));
                page.content_mut().set_scroll(0.0);
            }
            if event.downcast_ref::<NavigationChanged>().is_some() && focus_on_open {
                focus_on_open = false;
                ctx.emit(TextInputEvent::Focus {id: input_id});
            }
            false
        });
        StackPage::content(
            ctx, theme, Header::stack(&format!("Search {label}"), None),
            vec![input], Offset::Start, None, Some(on_event), Some(on_create),
        )
    }

    pub fn build_root(&self, ctx: &mut Context, theme: &Theme) -> Box<dyn AppPage> {
        let (title, items, header, bumper_a, bumper_b) = match self.clone() {
            PageType::Root { title, input, display, header, bumper_a, bumper_b } => {
                (title, input.into_iter().map(Item::Input)
                    .chain(display.into_iter().map(Item::Display)).collect::<Vec<_>>(),
                    header, bumper_a, bumper_b)
            }
            PageType::Content { title, items, header, .. } => {
                (title, items, header, None, None)
            }
            PageType::Edit { .. } => return Box::new(self.build_edit(ctx, theme, true)),
            PageType::Form { .. } | PageType::Review { .. } | PageType::Success { .. }
            | PageType::Search { .. } | PageType::EdgeToEdge { .. } | PageType::Messaging { .. }
            | PageType::MessageDetails { .. } | PageType::Camera { .. }
            | PageType::Photo { .. } | PageType::Video { .. } | PageType::Playback { .. } => {
                panic!("Not an accepted root type");
            }
        };
        let length = items.len();
        let offset = match items.iter().find_map(|item| match item {
            Item::Display(display) => Some(display), _ => None,
        }) {
            Some(Display::List { .. }) => Offset::Start,
            _ if length <= 1 => Offset::Center, _ => Offset::Start,
        };

        let action = header.map(|(icon, action)| (icon, action.get()));
        let header = Header::home(&title, action);
        let action = |(label, action): (String, Action)| {
            (ActionItem::format_label(&label), action.get())
        };
        let footer = (bumper_a.is_some() || bumper_b.is_some())
            .then(|| Footer::home(bumper_a.map(action), bumper_b.map(action)));
        Box::new(StackPage::content(ctx, theme, header, items, offset, footer, None, None))
    }

    pub fn build_form(
        ctx: &mut Context, theme: &Theme, id: String, title: &str, content: Content,
        next: Option<NavFn>, mut on_submit: Option<Box<dyn FormSubmit>>,
    ) -> StackPage<PageView> {
        let fields = vec![(id, 0..content.children().len())];
        let id = content.id();
        let footer = Footer::stack(
            None, Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(SubmitRequest(id))), None,
        );
        let event_theme = theme.clone();
        let mut values = None;
        let mut submission = Submission::default();
        let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
            if submission.check_navigation(id, event) {return true;}
            if let Some(new) = event.downcast_ref::<FormValues>() {
                values = Some(new.clone());
                return true;
            }
            if let Some(SubmitRequest(target)) = event.downcast_ref::<SubmitRequest>() {
                if *target != id {return false;}
                if submission.pending() {return true;}
                let values = values.clone().unwrap_or_else(|| page.values());
                let result = on_submit.as_mut().map(|submit| submit(ctx, &values))
                    .unwrap_or(FormComplete::None);
                submission.start(id, result, ctx, &event_theme, next.clone());
                if submission.pending() {page.disable_actions(true);}
                return true;
            }
            if let Some(SubmitFinished(target, result)) = event.downcast_ref::<SubmitFinished>() {
                if *target != id {return false;}
                submission.finish();
                result.clone().then(next.clone()).run(ctx, &event_theme);
                ctx.emit(FormEvent::Refresh(id));
                return true;
            }
            if let Some(FormEvent::Validated {id: target, ..}) = event.downcast_ref::<FormEvent>()
                && *target == id && submission.pending() {
                page.disable_actions(true);
                return true;
            }
            false
        });
        let page = theme.page(
            ctx, Header::stack(title, None), content, Some(footer), fields,
            true, None, Some(on_event), None, None,
        );
        StackPage::new(page)
    }

    pub fn build_review(
        ctx: &mut Context, theme: &Theme, title: &str, mut getter: Box<dyn ReviewItemGetter>,
        next: Option<NavFn>, mut on_submit: Box<dyn FormSubmit>,
    ) -> StackPage<PageView> {
        let content = Content::new(
            Offset::Start, vec![], Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        );
        let id = content.id();
        let footer = Footer::stack(
            None, Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(SubmitRequest(id))), None,
        );
        let event_theme = theme.clone();
        let mut values = None;
        let mut submission = Submission::default();
        let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
            if submission.check_navigation(id, event) {return true;}
            if let Some(new) = event.downcast_ref::<FormValues>() {
                if values.as_ref() == Some(new) {return true;}
                values = Some(new.clone());
                let children = getter(new).into_iter()
                    .filter_map(|mut item| item.build(ctx, &event_theme)).flatten().collect();
                *page.content_mut().children_mut() = children;
                if submission.pending() {page.disable_actions(true);}
                return true;
            }
            if let Some(SubmitRequest(target)) = event.downcast_ref::<SubmitRequest>() {
                if *target != id {return false;}
                if submission.pending() {return true;}
                let Some(values) = values.as_ref() else {return true;};
                let result = on_submit(ctx, values);
                submission.start(id, result, ctx, &event_theme, next.clone());
                if submission.pending() {page.disable_actions(true);}
                return true;
            }
            if let Some(SubmitFinished(target, result)) = event.downcast_ref::<SubmitFinished>() {
                if *target != id {return false;}
                submission.finish();
                page.disable_actions(false);
                result.clone().then(next.clone()).run(ctx, &event_theme);
                return true;
            }
            if let Some(FormEvent::Validated {id: target, ..}) = event.downcast_ref::<FormEvent>()
                && *target == id && submission.pending() {
                page.disable_actions(true);
                return true;
            }
            false
        });
        let page = theme.page(
            ctx, Header::stack(title, None), content, Some(footer), vec![],
            false, None, Some(on_event), None, None,
        );
        StackPage::new(page)
    }

    pub fn build_success(
        ctx: &mut Context, theme: &Theme, title: &str, mut getter: Box<dyn SuccessGetter>,
        flow_len: usize, mut on_submit: Option<Box<dyn FormSubmit>>,
    ) -> StackPage<PageView> {
        let content = Content::new(
            Offset::Center, vec![], Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        );
        let event_theme = theme.clone();
        let mut values = None;
        let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
            let Some(new) = event.downcast_ref::<FormValues>() else {return false;};
            if values.as_ref() == Some(new) {return true;}
            values = Some(new.clone());
            if let Some(mut on_submit) = on_submit.take() {on_submit(ctx, new).run(ctx, &event_theme);}
            let (icon, description) = getter(new.clone());
            let items = vec![Display::icon(icon), Display::Text {
                text: description, size: TextSize::H4,
                style: TextStyle::Heading, align: Align::Center,
            }];
            *page.content_mut().children_mut() = items.into_iter()
                .filter_map(|mut item| item.build(ctx, &event_theme)).flatten().collect();
            true
        });
        let page = theme.page(
            ctx, Header::stack_end(title), content, Some(Footer::stack_end(Some(flow_len))),
            vec![], false, None, Some(on_event), None, None,
        );
        StackPage::new(page)
    }

    pub fn build_edit(&self, ctx: &mut Context, theme: &Theme, is_root: bool) -> StackPage<PageView> {
        let PageType::Edit {title, header, items, display, mut on_save, ..} = self.clone() else {
            panic!("Expected an edit page");
        };
        let ids = FormItem::ids(&items);
        let mut children = Vec::new();
        let mut validators = Vec::new();
        let mut ranges = Vec::new();
        for item in items {
            let validation = item.validation();
            let start = children.len();
            children.extend(item.build().build(theme, ctx.window.is_mobile).unwrap_or_default());
            let range = start..children.len();
            validators.push((range.clone(), validation));
            ranges.push(range);
        }
        for mut item in display {children.extend(item.build(ctx, theme).unwrap_or_default());}
        let content = Content::new(Offset::Start, children, Box::new(FormValidation(validators)));
        let id = content.id();
        let on_primary: Box<dyn Callback> =
            Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(EditRequest(id, None)));
        let action = header.map(|(icon, action)| (icon, action.get()));
        let (header, footer) = if is_root {
            (Header::home(&title, action), Footer::home(Some(("Save".into(), on_primary)), None))
        } else {
            (Header::stack(&title, action), Footer::stack(Some("Save"), on_primary, None))
        };
        let fields = ids.into_iter().zip(ranges).collect::<Vec<_>>();
        let event_theme = theme.clone();
        let mut values: Option<FormValues> = None;
        let mut saved_values = FormValues::from_content(&content, &fields);
        let mut saved_content = content.clone();
        let mut valid = false;
        let mut saving = false;
        let mut leave = None;
        let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
            if let Some(check) = event.downcast_ref::<CheckEdits>() {
                if saving || page.values() != saved_values {
                    *check.0.borrow_mut() = Some((id, saving));
                }
                return true;
            }
            if let Some(new) = event.downcast_ref::<FormValues>() {
                values = Some(new.clone());
                page.disable_actions(!valid || page.values() == saved_values || saving);
                return true;
            }
            if let Some(form) = event.downcast_ref::<FormEvent>() {
                match form {
                    FormEvent::Changed(target) if *target == id => {
                        if is_root {values = Some(page.values());}
                        page.disable_actions(!valid || page.values() == saved_values || saving);
                    }
                    FormEvent::Validated {id: target, valid: current} if *target == id => {
                        valid = *current;
                        page.disable_actions(!valid || page.values() == saved_values || saving);
                        return true;
                    }
                    _ => {}
                }
            }
            if let Some(EditDiscard(target)) = event.downcast_ref::<EditDiscard>() {
                if *target != id {return false;}
                *page.content_mut() = saved_content.clone();
                values = Some(page.values());
                page.disable_actions(!valid || page.values() == saved_values || saving);
                ctx.emit(FormEvent::Refresh(id));
                return true;
            }
            let finished = if let Some(EditRequest(target, navigation)) = event.downcast_ref() {
                if *target != id {return false;}
                if saving {return true;}
                if !valid {
                    FormComplete::Error("Please correct the highlighted fields before saving.".into())
                        .run(ctx, &event_theme);
                    return true;
                }
                let saved = page.values();
                if saved == saved_values {return true;}
                saving = true;
                leave = navigation.clone();
                page.disable_actions(true);
                let snapshot = page.content().clone();
                let result = on_save(ctx, values.as_ref().unwrap_or(&saved));
                if let FormComplete::Pending(pending) = result {
                    FormComplete::pending(async move {
                        let result = pending.await.resolved().await;
                        FormComplete::Action(Action::custom(move |ctx: &mut Context, _: &Theme| {
                            ctx.emit(EditFinished(
                                id, saved.clone(), snapshot.clone(), result.clone(),
                            ));
                        }))
                    }).run(ctx, &event_theme);
                    return true;
                }
                Some(EditFinished(id, saved, snapshot, result))
            } else {
                event.downcast_ref::<EditFinished>().filter(|finished| finished.0 == id).cloned()
            };
            if let Some(EditFinished(_, saved, snapshot, mut result)) = finished {
                saving = false;
                let navigation = leave.take();
                if !matches!(result, FormComplete::Error(_)) {
                    saved_values = saved;
                    saved_content = snapshot;
                }
                page.disable_actions(!valid || page.values() == saved_values);
                if let Some(navigation) = navigation {
                    if matches!(result, FormComplete::Error(_)) {result.run(ctx, &event_theme);}
                    else if page.values() == saved_values {ctx.emit(navigation);}
                    // Edits typed during an asynchronous save remain dirty and visible.
                } else {result.run(ctx, &event_theme);}
                return true;
            }
            false
        });
        let page = theme.page(
            ctx, header, content, Some(footer), fields, true, None, Some(on_event), None, None,
        );
        StackPage::new(page)
    }

    pub fn build_messaging(ctx: &mut Context, theme: &Theme, room: Instance<ChatRoom>, flow_len: usize, target: Option<Id>) -> StackPage {
        let room_data = room.pending();
        let my_name = ctx.air.me();
        let is_group = room_data.is_group();
        let has_left = !room_data.members.contains(&my_name);
        let mut reads = RoomReadState::open(ctx, room.id());
        let messages: Vec<Message> = room_data.messages(my_name).cloned().collect();
        let received: Vec<Id> = messages.iter().filter(|message| message.author != my_name)
            .map(|message| message.id).collect();

        let mut profiles = room_data.members.clone().into_iter()
            .map(|name| Profile::from_name(ctx, name))
            .filter(|profile| profile.pending().name != Some(my_name)).collect::<Vec<_>>();
        let did_i_block = if !is_group && profiles.len() == 1 {
            let profile = profiles[0].pending();
            Block::did_i_block(ctx, profile.name.unwrap()).then_some(profile.username.to_string())
        } else {None};
        let am_i_blocked = if !is_group && profiles.len() == 1 {
            let profile = profiles[0].pending();
            Block::am_i_blocked(ctx, profile.name.unwrap()).then_some(profile.username.to_string())
        } else {None};
        let is_blocked = has_left || did_i_block.is_some() || am_i_blocked.is_some();
        let display_profiles = profiles.iter_mut()
            .map(|profile| profile.pending().to_view()).collect();

        let info_room = room.clone();
        let info = Some(Box::new(move |ctx: &mut Context, theme: &Theme| {
            (Flow::new(vec![Page::chat_info(info_room.clone())]).build(ctx, theme))(ctx, theme);
        }) as Box<dyn Callback>);
        let title = room_data.name(ctx);
        let avatar = is_group.then(|| room_data.group_details().photo);
        let notice = if has_left {Some("You left this group.".to_string())}
            else if let Some(username) = am_i_blocked {Some(format!("{} blocked you.", username))}
            else {did_i_block.map(|username| format!("You blocked {}.", username))};
        drop(room_data);

        let mut groups: Vec<Vec<Message>> = Vec::new();
        for message in messages {
            if groups.last().and_then(|group| group.last()).is_some_and(|last| {
                Some(last.id) != target && Some(message.id) != target
                    && !matches!(last.body, MessageContent::System(_))
                    && !matches!(message.body, MessageContent::System(_))
                    && last.author == message.author
                    && last.timestamp.abs_diff(message.timestamp) / 1_000_000_000 <= 60
            }) {groups.last_mut().unwrap().push(message);}
            else {groups.push(vec![message]);}
        }
        let target_index = target.and_then(|target| groups.iter()
            .position(|group| group.iter().any(|message| message.id == target)));
        let mut items: Vec<Item> = groups.into_iter().map(|messages| {
            let rendered = messages.into_iter().map(|message| {
                let room = room.clone();
                let tapped = message.clone();
                message.to_view(ctx, move |ctx: &mut Context, theme: &Theme| {
                    let info_room = room.clone();
                    let info_message = tapped.clone();
                    let header = Some((Icons::Info, Action::deferred_flow(
                        move |_: &mut Context, _: &Theme| {
                            Flow::new(vec![Page::message_details(
                                info_room.clone(), info_message.clone(), 2,
                            )])
                        },
                    )));
                    let page = match &tapped.body {
                        MessageContent::Photo(photo) => {
                            Page::Static(Box::new(PageType::photo(photo.clone(), header)))
                        }
                        MessageContent::Video(video) => {
                            Page::Static(Box::new(PageType::video(video.clone(), header)))
                        }
                        _ => Page::message_details(room.clone(), tapped.clone(), 1),
                    };
                    (Flow::new(vec![page]).build(ctx, theme))(ctx, theme);
                })
            }).collect();
            Item::Display(Display::messages(rendered, is_group))
        }).collect();
        if !is_blocked {items.push(Item::Display(Display::Typing {room: room.clone()}));}
        let children = items.into_iter()
            .filter_map(|mut item| item.build(ctx, theme)).flatten().collect();
        let header = Header::messaging(title, display_profiles, avatar, flow_len, info);
        let mut send_room = room.clone();
        let footer = match notice {
            Some(notice) => Footer::alert(&notice),
            None => Footer::input("Message...", move |ctx: &mut Context, value: &mut String| {
                if !value.is_empty() && send_room.pending().can_message(ctx) {
                    send_room.send(ChatRoomAction::SendMessage(value.to_string()));
                }
            }, |_: &mut Context, value: &mut String| value.is_empty()),
        };
        let mut content = Content::new(Offset::End, children,
            Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true));
        if let Some(index) = target_index {content.reveal_child(index);}
        let identity = Some(room.id().to_string());
        let read_room = room.clone();
        let on_show = Box::new(move |ctx: &mut Context| {
            if !read_room.pending().members.contains(&ctx.air.me()) {return;}
            let unread = {
                let reads = reads.pending();
                received.iter().copied().filter(|id| !reads.read_flag(*id)).collect::<Vec<_>>()
            };
            if !unread.is_empty() {reads.send(unread);}
        });

        let mut room = room;
        let event_theme = theme.clone();
        let typing = Rc::new(RefCell::new(TypingPublisher::default()));
        let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
            if matches!(event.downcast_ref::<WindowEvent>(), Some(WindowEvent::Focused(false)))
                || matches!(event.downcast_ref::<ptsd::events::TextInput>(),
                    Some(ptsd::events::TextInput::Focused(false))) {
                typing.borrow_mut().stop();
            }
            let text = event.downcast_ref::<TextInputEvent>();
            let send = event.downcast_ref::<SendEvent>();
            let capture = event.downcast_ref::<MessageBarEvent>() == Some(&MessageBarEvent::Capture);
            if text.is_none() && send.is_none() && !capture {return false;}
            let allowed = room.pending().can_message(ctx);
            if let Some(text) = text && let Some((field, focused)) = page.composer_state() {
                match text {
                    TextInputEvent::Changed {id, value} if *id == field => {
                        if page.focused() && allowed && focused {
                            typing.borrow_mut().edit(ctx, &room, !value.is_empty());
                            let typing = typing.clone();
                            page.after(Duration::from_secs(3), move |_: &mut Context| {
                                typing.borrow_mut().stop();
                            });
                        } else {typing.borrow_mut().stop();}
                    }
                    TextInputEvent::Submit {id} if *id == field => typing.borrow_mut().stop(),
                    _ => {}
                }
            }
            if capture {
                typing.borrow_mut().stop();
                if !allowed {return true;}
                let camera = PageType::camera(|ctx: &mut Context, photo| {
                    ctx.emit(crate::design_system::AttachmentEvent(SendEvent::Photo(photo)));
                });
                (Flow::new(vec![Page::Static(Box::new(camera))]).build(ctx, &event_theme))(
                    ctx, &event_theme,
                );
                return true;
            }
            if let Some(send) = send {
                typing.borrow_mut().stop();
                if !allowed {return true;}
                match send {
                    SendEvent::Sticker(sticker) => {
                        room.send(ChatRoomAction::SendSticker(sticker.clone()));
                    }
                    SendEvent::Gif(gif) => {room.send(ChatRoomAction::SendGif(gif.clone()));}
                    SendEvent::Photo(photo) => {room.send(ChatRoomAction::SendPhoto(photo.clone()));}
                    SendEvent::Video(video) => {
                        let members = room.pending().members.clone();
                        let video = video.share(ctx, &members);
                        room.send(ChatRoomAction::SendVideo(video));
                    }
                }
            }
            false
        });
        let page = theme.page(
            ctx, header, content, Some(footer), vec![], false,
            identity, Some(on_event), Some(on_show), None,
        );
        StackPage::new(page)
    }

    pub fn build_message_details(ctx: &mut Context, theme: &Theme, room: Instance<ChatRoom>, message: Message, pops: usize) -> StackPage<PageView> {
        let title = if message.author == ctx.air.me() {"Sent message"} else {"Received message"};
        let page_content = Content::new(
            Offset::Start, vec![], Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        );
        let id = page_content.id();
        let content = message.body.clone();
        let share = (!matches!(content, MessageContent::Text(_) | MessageContent::System(_))).then(|| {
            ActionItem::new(Action::custom(move |ctx: &mut Context, _: &Theme| {
                ctx.emit(SubmitRequest(id));
            }), "Share", Icons::Send)
        });
        let profile = Profile::from_name(ctx, message.author);
        let error = Rc::new(RefCell::new(None::<String>));
        let visible_error = error.clone();
        let render_theme = theme.clone();
        let update = move |ctx: &mut Context, page: &mut PageView, _: Instance<Profile>| {
            let mut items = message.details(ctx, room.clone(), pops, share.clone());
            if let Some(error) = visible_error.borrow().as_deref() {
                items.push(Item::Display(Display::instructions(error)));
            }
            *page.content_mut().children_mut() = items.into_iter()
                .filter_map(|mut item| item.build(ctx, &render_theme)).flatten().collect();
        };
        let watcher = ContractWatch::new(profile.clone());
        let render = update.clone();
        let on_create: ListenCallback<PageView> = Box::new(move |page, ctx| {
            page.watch(ctx, watcher, render);
        });
        let event_theme = theme.clone();
        let mut request = None;
        let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
            if !matches!(event.downcast_ref::<SubmitRequest>(),
                Some(SubmitRequest(target)) if *target == id) {
                return false;
            }
            if request.is_some_and(|request| page.pending(request)) {return true;}
            let future: SaveRequest = match &content {
                MessageContent::Photo(photo) => {
                    Box::pin(Message::export(Arc::from(photo.jpeg()), "jpg"))
                }
                MessageContent::Sticker(sticker) => {
                    let image = sticker.image();
                    Box::pin(async move {Ok(Data::Image(image))})
                }
                MessageContent::Video(video) => {
                    let bytes = crate::messages::VideoClip::find(ctx, video.clip);
                    Box::pin(async move {
                        let bytes = bytes.ok_or(
                            "This video is still downloading. Try again when it finishes.",
                        )?;
                        Message::export(bytes, "mp4").await
                    })
                }
                MessageContent::Gif(gif) => {
                    let bytes = event_theme.gif_bytes(gif);
                    Box::pin(async move {
                        let bytes = bytes.await?;
                        let extension = Message::image_extension(&bytes)
                            .or_else(|| (bytes.get(4..8) == Some(b"ftyp")).then_some("mp4"))
                            .ok_or("This GIF's format cannot be saved to Photos.")?;
                        Message::export(bytes, extension).await
                    })
                }
                _ => return true,
            };
            let (error, update, profile) = (error.clone(), update.clone(), profile.clone());
            let shared = Cell::new(false);
            request = Some(page.spawn(future, move |ctx, page, result| {
                *error.borrow_mut() = match result {
                    Ok(data) => {
                        if !shared.replace(true) {ctx.hardware.share(&data);}
                        None
                    }
                    Err(error) => Some(error),
                };
                update(ctx, page, profile.clone());
            }));
            true
        });
        let page = theme.page(
            ctx, Header::stack(title, None), page_content, None, vec![], false,
            None, Some(on_event), None, Some(on_create),
        );
        StackPage::new(page)
    }
}

pub struct Root(pub Page);
impl Root {
    pub fn new(
        title: &str, items: Vec<Display>, header: Option<(Icons, Action)>,
        bumper_a: Option<(String, Action)>, bumper_b: Option<(String, Action)>,
    ) -> Self {
        Root(Page::Static(Box::new(PageType::root(
            title, vec![], items, header, bumper_a, bumper_b,
        ))))
    }

    pub fn custom(page: Page) -> Self { Root(page) }

    pub fn contacts(invitations: &ContactInvitations) -> Root {
        let invitations = invitations.clone();
        Root(Page::watching(ListWatch::<Profile>::default(),
            move |ctx: &mut Context, _theme: &Theme, profiles: Vec<Instance<Profile>>| {
                let items = SearchEntry::contacts(ctx, profiles).into_iter()
                    .map(|entry| entry.item).collect();
                let search = Action::flow(Flow::new(vec![Page::Static(Box::new(PageType::Search {
                    source: SearchSource::Contacts(ListWatch::<Profile>::default()),
                }))]));
                let invitations = invitations.clone();
                let new_contact = Action::deferred_flow(move |_ctx: &mut Context, theme: &Theme| {
                    Flow::from_form(Form::new_contact(&invitations, theme))
                });
                PageType::root("Contacts", vec![], vec![Display::list(
                    None, items, Some("No contacts yet.\nGet started by adding a friend."),
                )], Some((Icons::Search, search)), Some(("New Contact".into(), new_contact)), None)
            }))
    }

    pub fn profile(ctx: &mut Context, invitations: &ContactInvitations) -> Root {
        let name = ctx.air.me();
        Root(Page::Static(Box::new(PageType::editing(
            true, Profile::me(ctx), ctx.window.is_mobile, Some(vec![
                ActionItem::new_with_active(
                    Action::copy(&name.to_string()), "Copy", "Copied", Icons::Copy,
                ),
                ActionItem::new(Action::flow(Flow::new(vec![Page::Static(Box::new(
                    PageType::qr_page(invitations, name),
                ))])), "Display QR Code", Icons::QrCode),
                ActionItem::new(Action::share_contact(invitations, name), "Share", Icons::Send),
            ]),
        ))))
    }

    pub fn messages(ctx: &mut Context, invitations: &ContactInvitations) -> Root {
        ChatRoom::start_notifications(ctx);
        let invitations = invitations.clone();
        Root(Page::watching((
            ListWatch::<ChatRoom>::default(), ListWatch::<Profile>::default(),
            ListWatch::<RoomReadState>::default(),
        ),
            move |ctx: &mut Context, _theme: &Theme, (rooms, profiles, reads): MessageLists| {
                let items = SearchEntry::messages(ctx, rooms, profiles, reads).into_iter()
                    .map(|entry| entry.item).collect();
                let search = Action::flow(Flow::new(vec![Page::Static(Box::new(PageType::Search {
                    source: SearchSource::Messages((
                        ListWatch::<ChatRoom>::default(), ListWatch::<Profile>::default(),
                        ListWatch::<RoomReadState>::default(),
                    )),
                }))]));
                let invitations = invitations.clone();
                let new_message = Action::deferred_flow(move |ctx: &mut Context, theme: &Theme| {
                    Flow::from_form(Form::new_message(&invitations, ctx, theme))
                });
                PageType::root("Messages", vec![], vec![Display::list(
                    None, items, Some("No messages yet.\nGet started by messaging a friend."),
                )], Some((Icons::Search, search)), Some(("New Message".into(), new_message)), None)
            }))
    }

}

impl crate::form::Submission {
    pub fn check_navigation(&self, id: Uuid, event: &dyn DynEvent) -> bool {
        let Some(check) = event.downcast_ref::<CheckEdits>() else {return false;};
        if self.pending() {*check.0.borrow_mut() = Some((id, true));}
        true
    }
}

/// Owns deferred submissions and checks the active editor before destructive navigation.
/// App::build installs one host for the whole interface, including incoming invitations.
#[derive(Debug, Clone)]
pub struct FormHost {
    inner: Box<dyn Drawable>,
    theme: Theme,
    pending: Vec<crate::PendingSubmit>,
}
impl FormHost {
    pub fn new(inner: impl Drawable, theme: Theme) -> Self {
        Self {inner: Box::new(inner), theme, pending: vec![]}
    }

    pub fn confirm(&self, ctx: &mut Context, id: Uuid, navigation: NavigationEvent) {
        let save_navigation = navigation.clone();
        let save = Action::custom(move |ctx: &mut Context, _: &Theme| {
            let navigation = save_navigation.clone();
            ctx.emit(NavigationEvent::reset_with_fn(move |ctx: &mut Context| {
                ctx.emit(EditRequest(id, Some(navigation.clone())));
            }));
        });
        let discard = Action::custom(move |ctx: &mut Context, _: &Theme| {
            let navigation = navigation.clone();
            ctx.emit(NavigationEvent::reset_with_fn(move |ctx: &mut Context| {
                ctx.emit(EditDiscard(id));
                ctx.emit(navigation.clone());
            }));
        });
        let keep = Action::custom(|ctx: &mut Context, _: &Theme| ctx.emit(NavigationEvent::Reset));
        let page = PageType::display("Unsaved changes", vec![
            Display::instructions("Save your changes before leaving?"),
            Display::actions(vec![
                ActionItem::new(save, "Save and leave", Icons::Forward),
                ActionItem::new(discard, "Discard changes", Icons::Block),
                ActionItem::new(keep, "Keep editing", Icons::Edit),
            ], false),
        ], None, Bumper::None, Offset::Center);
        (Flow::new(vec![Page::Static(Box::new(page))]).build(ctx, &self.theme))(ctx, &self.theme);
    }
}
impl Drawable for FormHost {
    fn request_width(&mut self) -> (f32, f32) {self.inner.request_width()}
    fn request_height(&mut self, width: f32) -> (f32, f32) {self.inner.request_height(width)}
    fn layout(&mut self, size: (f32, f32)) -> LayoutTree {self.inner.layout(size)}
    fn draw(&self, layout: &LayoutTree, offset: (f32, f32), bound: Rect) -> Vec<Instruction> {
        self.inner.draw(layout, offset, bound)
    }
    fn event(&mut self, ctx: &mut Context, layout: &LayoutTree, event: Box<dyn DynEvent>) {
        if let Some(deferred) = event.downcast_ref::<DeferredSubmit>() {
            self.pending.push(deferred.0.clone());
            return;
        }
        if let Some(navigation) = event.downcast_ref::<NavigationEvent>() {
            // Push preserves the current page (pickers and dialogs must retain drafts).
            if !matches!(navigation, NavigationEvent::Error(_)) {
                let check = CheckEdits::default();
                self.inner.event(ctx, layout, Box::new(check.clone()));
                if let Some((id, saving)) = *check.0.borrow() {
                    if saving {return;}
                    if !matches!(navigation, NavigationEvent::Push(..)) {
                        self.confirm(ctx, id, navigation.clone());
                        return;
                    }
                }
            }
        }
        self.inner.event(ctx, layout, event);
    }
    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            let mut pending = self.pending.iter()
                .map(|pending| Box::pin(pending.clone())).collect::<Vec<_>>();
            let mut inner = self.inner.as_mut().changed();
            poll_fn(|cx| {
                for (index, pending) in pending.iter_mut().enumerate() {
                    if let Poll::Ready(mut result) = pending.as_mut().poll(cx) {
                        return Poll::Ready(Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                            let target = target.downcast_mut::<FormHost>().unwrap();
                            target.pending.remove(index);
                            result.run(ctx, &target.theme);
                        }) as ListenCallback);
                    }
                }
                if let Poll::Ready(callback) = inner.as_mut().poll(cx) {
                    return Poll::Ready(Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                        let target = target.downcast_mut::<FormHost>().unwrap();
                        callback(target.inner.as_mut().as_any_mut(), ctx);
                    }) as ListenCallback);
                }
                Poll::Pending
            }).await
        })
    }
}

#[derive(Debug, Clone)]
struct EditRequest(Uuid, Option<NavigationEvent>);
impl Event for EditRequest {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Debug, Clone)]
struct EditFinished(Uuid, FormValues, Content, FormComplete);
impl Event for EditFinished {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Debug, Clone)]
struct EditDiscard(Uuid);
impl Event for EditDiscard {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Debug, Clone, Default)]
struct CheckEdits(Rc<RefCell<Option<(Uuid, bool)>>>);
impl Event for CheckEdits {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Clone)]
struct FormValidation(Vec<(Range<usize>, Box<dyn ValidationFn>)>);
impl Validate<Vec<&mut Box<dyn Drawable>>, bool> for FormValidation {
    fn validate(&mut self, ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>) -> bool {
        let mut valid = true;
        for (range, validation) in &mut self.0 {
            let inputs = children[range.clone()].iter_mut().map(|child| &mut **child).collect();
            valid &= validation.validate(ctx, inputs);
        }
        valid
    }
    fn change(&mut self) -> Pin<Box<dyn Future<Output = Option<()>> + '_>> {
        Box::pin(async move {
            let mut waits: Vec<_> =
                self.0.iter_mut().map(|(_, validation)| validation.change()).collect();
            poll_fn(|cx| {
                let mut index = 0;
                while index < waits.len() {
                    match waits[index].as_mut().poll(cx) {
                        Poll::Ready(Some(())) => return Poll::Ready(Some(())),
                        Poll::Ready(None) => {drop(waits.swap_remove(index));}
                        Poll::Pending => {index += 1;}
                    }
                }
                if waits.is_empty() {Poll::Ready(None)} else {Poll::Pending}
            }).await
        })
    }
}

#[derive(Debug, Clone)]
struct SearchEntry {
    item: ListItem,
    text: String,
}
impl SearchEntry {
    pub fn filter(entries: &[Self], history: &[SearchMessage], contacts: bool, query: &str) -> Vec<Display> {
        let query = query.to_lowercase();
        let terms = query.split_whitespace().collect::<Vec<_>>();
        let items: Vec<_> = entries.iter()
            .filter(|entry| terms.iter().all(|term| entry.text.contains(term)))
            .map(|entry| entry.item.clone()).collect();
        let empty = match (contacts, terms.is_empty()) {
            (true, true) => "No contacts yet.", (true, false) => "No matching contacts.",
            (false, true) => "No messages yet.",
            (false, false) => "No matching conversations or messages.",
        };
        if contacts || terms.is_empty() {return vec![Display::list(None, items, Some(empty))];}
        let messages: Vec<_> = history.iter()
            .filter(|entry| terms.iter().all(|term| entry.text.contains(term)))
            .map(|entry| entry.item(&terms)).collect();
        if items.is_empty() && messages.is_empty() {
            return vec![Display::list(None, vec![], Some(empty))];
        }
        let mut sections = Vec::new();
        if !items.is_empty() {sections.push(Display::list(Some("Conversations"), items, None));}
        if !messages.is_empty() {sections.push(Display::list(Some("Messages"), messages, None));}
        sections
    }

    pub fn contacts(ctx: &mut Context, profiles: Vec<Instance<Profile>>) -> Vec<SearchEntry> {
        let me = ctx.air.me();
        let mut entries = profiles.into_iter().filter_map(|mut instance| {
            let name = instance.pending().name?;
            if name == me {return None;}
            let view = Flow::new(vec![Page::profile(ctx, &mut instance)]);
            let profile = instance.pending();
            Some((name, SearchEntry {
                text: format!("{} {}", profile.username, name).to_lowercase(),
                item: ListItem::avatar(profile.avatar.clone(), &profile.username,
                    &profile.name(), None, Some(Action::flow(view))),
            }))
        }).collect::<Vec<_>>();
        entries.sort_by_cached_key(|(name, entry)| (entry.item.title.to_lowercase(), *name));
        entries.into_iter().map(|(_, entry)| entry).collect()
    }

    pub fn messages(
        ctx: &mut Context, rooms: Vec<Instance<ChatRoom>>,
        profiles: Vec<Instance<Profile>>,
        reads: Vec<Instance<RoomReadState>>,
    ) -> Vec<SearchEntry> {
        let me = ctx.air.me();
        let profiles = profiles.into_iter().filter_map(|instance| {
            let profile = instance.pending();
            Some((profile.name?, profile.username.clone()))
        }).collect::<HashMap<Name, String>>();
        let reads = reads.into_iter().filter_map(|instance| {
            let read = instance.pending();
            (read.owner == me).then(|| (read.room, read.clone()))
        }).collect::<HashMap<_, _>>();
        let mut entries = rooms.into_iter().filter(|room| room.pending().members.contains(&me))
            .map(|mut instance| {
                let id = instance.id();
                let chat = Flow::new(vec![Page::messaging(ctx, &mut instance)]);
                let room = instance.pending();
                let members = room.members.iter().copied()
                    .filter(|name| *name != me).collect::<Vec<_>>();
                let name = room.name(ctx);
                let avatar = if room.is_group() {
                    room.group_details().photo
                } else {
                    let profile = Profile::from_name(ctx, members.first().copied().unwrap_or(me));
                    profile.pending().avatar.clone()
                };
                let (timestamp, last) = match room.messages(me).last() {
                    Some(last) => (last.timestamp, last.preview(ctx, room.is_group())),
                    None => (0, "No messages yet.".into()),
                };
                let unread = room.messages(me).any(|message| message.author != me
                    && reads.get(&id).is_none_or(|read| !read.is_read(message)));
                let mut text = format!("{name} {last}");
                for member in &room.members {
                    text.push_str(&format!(" {member}"));
                    if let Some(username) = profiles.get(member) {
                        text.push_str(&format!(" {username}"));
                    }
                }
                (timestamp, id, SearchEntry {
                    text: text.to_lowercase(),
                    item: ListItem::avatar(avatar, &name, &last, None, Some(Action::flow(chat)))
                        .with_badge(unread),
                })
        }).collect::<Vec<_>>();
        entries.sort_by_key(|(timestamp, id, _)| (std::cmp::Reverse(*timestamp), *id));
        entries.into_iter().map(|(_, _, entry)| entry).collect()
    }
}

#[derive(Debug, Clone)]
struct SearchMessage {
    room: Rc<Instance<ChatRoom>>,
    id: Id,
    timestamp: u64,
    avatar: AvatarContent,
    title: String,
    preview: String,
    text: String,
}

impl SearchMessage {
    pub fn history(ctx: &mut Context, rooms: &[Instance<ChatRoom>], profiles: &[Instance<Profile>]) -> Vec<Self> {
        let me = ctx.air.me();
        let profiles = profiles.iter().filter_map(|instance| {
            let profile = instance.pending();
            Some((profile.name?, profile.username.clone()))
        }).collect::<HashMap<_, _>>();
        let mut entries = Vec::new();
        for instance in rooms {
            let room = instance.pending();
            if !room.members.contains(&me) {continue;}
            let title = room.name(ctx);
            let avatar = if room.is_group() {room.group_details().photo} else {
                let name = room.direct_contact(me).unwrap_or(me);
                Profile::from_name(ctx, name).pending().avatar.clone()
            };
            let instance = Rc::new(instance.clone());
            for message in room.messages(me) {
                let preview = message.preview_with(me, room.is_group(), |name| {
                    profiles.get(&name).cloned().unwrap_or_else(|| name.to_string())
                });
                entries.push(SearchMessage {
                    room: instance.clone(), id: message.id, timestamp: message.timestamp,
                    avatar: avatar.clone(), title: title.clone(),
                    text: preview.to_lowercase(), preview,
                });
            }
        }
        entries.sort_by_cached_key(|entry| {
            (std::cmp::Reverse(entry.timestamp), entry.room.id(), entry.id)
        });
        entries
    }

    pub fn excerpt(&self, terms: &[&str]) -> String {
        let text = self.preview.split_whitespace().collect::<Vec<_>>().join(" ");
        let lowercase = text.to_lowercase();
        let matched = terms.iter().filter_map(|term| lowercase.find(term)).min().unwrap_or(0);
        // Lowercasing can expand a Unicode character, so translate the match
        // back to a character offset before cutting the original text.
        let mut bytes = 0;
        let index = text.chars().take_while(|character| {
            bytes += character.to_lowercase().map(char::len_utf8).sum::<usize>();
            bytes <= matched
        }).count();
        let characters = text.chars().collect::<Vec<_>>();
        let start = index.saturating_sub(16);
        let end = (start + 100).min(characters.len());
        format!("{}{}{}", if start > 0 {"…"} else {""},
            characters[start..end].iter().collect::<String>(),
            if end < characters.len() {"…"} else {""})
    }

    pub fn item(&self, terms: &[&str]) -> ListItem {
        let timestamp = ptsd::utils::Timestamp::from_u64(self.timestamp);
        let subtitle = format!("{} · {}\n{}", timestamp.date(), timestamp.time(), self.excerpt(terms));
        let room = self.room.clone();
        let id = self.id;
        let action = Action::custom(move |ctx: &mut Context, theme: &Theme| {
            // A result can become stale between its last refresh and the tap.
            let visible = {
                let data = room.pending();
                let me = ctx.air.me();
                if !data.members.contains(&me) {return;}
                data.messages(me).any(|message| message.id == id)
            };
            if visible {
                let page = Page::messaging_at(ctx, &mut room.as_ref().clone(), id);
                let mut flow = Flow::new(vec![page]);
                (flow.build(ctx, theme))(ctx, theme);
            }
        });
        ListItem::avatar(self.avatar.clone(), &self.title, &subtitle, None, Some(action))
    }
}
