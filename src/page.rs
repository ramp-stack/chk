use prism::air::{Contract, Instance, Name};
use prism::Context;
use ptsd::layout::Offset;
use ptsd::navigation::{AppPage, NavigationEvent};
use ptsd::utils::ValidationFn;

use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

use crate::blocking::Block;
use crate::closure::{
    FormSubmit, NavFn, PageBuilderContractFn, PageBuilderContractMultiplesFn, ReviewItemGetter,
    SuccessGetter,
};
use crate::design_system::{Content, Icons, QrCodeFound, Theme};
use crate::form::FormItem;
use crate::items::{Action, Display, Input, Item};
use crate::messages::{ChatRoom, ChatRoomAction, Photo, RoomNotificationState, Video};
use crate::profiles::{Profile, ProfileAction};
use crate::{
    ActionItem, AvatarIconStyle, AvatarPurpose, ContactInvitations, Flow, FormComplete, FormValidState,
    FormValues, Listener,
};

mod edit;
pub use edit::*;
mod form;
pub use form::*;
mod review;
pub use review::*;
mod messages;
pub use messages::*;
mod root;
pub use root::*;
mod stack;
pub use stack::*;
mod watch;
pub use watch::*;
mod media;
pub use media::*;
mod video;
pub use video::*;

pub trait PageBuilder: Debug + dyn_clone::DynClone {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<()>> + '_>>;
    fn build(&mut self, ctx: &mut Context, theme: &Theme) -> PageType;
}

dyn_clone::clone_trait_object!(PageBuilder);

#[derive(Debug, Clone)]
pub enum Page { Static(Box<PageType>), Refreshing(Box<dyn PageBuilder>) }

impl Page {
    pub fn watching<W: Watcher>(watcher: W, builder: impl FnMut(&mut Context, &Theme, W::Output) -> PageType + Clone + 'static) -> Self {
        Page::Refreshing(Box::new(Watching::new(watcher, builder)))
    }

    pub fn contract_updates<C: Contract>(contract: Instance<C>, builder: impl PageBuilderContractFn<C> + 'static) -> Self {
        let builder: Box<dyn PageBuilderContractFn<C>> = Box::new(builder);
        Page::Refreshing(Box::new(Watching::new(ContractWatch::new(contract), builder)))
    }

    pub fn updates_list_changes<C: Contract>(_ctx: &mut Context, builder: impl PageBuilderContractMultiplesFn<C> + 'static) -> Self {
        let builder: Box<dyn PageBuilderContractMultiplesFn<C>> = Box::new(builder);
        Page::Refreshing(Box::new(Watching::new(ListWatch::<C>::default(), builder)))
    }

    pub fn profile(ctx: &mut Context, profile: &mut Instance<Profile>) -> Self {
        let my_name = profile.pending().name.unwrap();
        let is_me = my_name == ctx.air.me();
        match is_me {
            true => Page::Static(Box::new(PageType::editing(
                is_me, profile.clone(), ctx.window.is_mobile, None,
            ))),
            false => Page::Refreshing(Box::new(Watching::new(
                (ContractWatch::new(profile.clone()), ListWatch::<Block>::default(),
                 ListWatch::<ChatRoom>::default(), ListWatch::<RoomNotificationState>::default()),
                |ctx: &mut Context, _: &Theme, (profile, _, _, _): (Instance<Profile>, _, _, _)| {
                    PageType::profile_view(ctx, profile)
                },
            ))),
        }
    }

    pub fn messaging(_ctx: &mut Context, room: &mut Instance<ChatRoom>) -> Self {
        Page::Refreshing(Box::new(Watching::new(
            (ContractWatch::new(room.clone()), ListWatch::<Block>::default(), ListWatch::<Profile>::default()),
            |_ctx: &mut Context,
             _theme: &Theme,
             (room, _blocks, _profiles): (Instance<ChatRoom>, Vec<Instance<Block>>, Vec<Instance<Profile>>)| {
                PageType::messaging(room)
            },
        )))
    }

    pub fn builder(&self) -> Option<Box<dyn PageBuilder>> {
        match self { Page::Refreshing(p) => Some(p.clone()), _ => None }
    }

    pub fn page_type(&self) -> Option<PageType> {
        match self {Page::Static(p) => Some(p.as_ref().clone()), _ => None}
    }

    pub fn build(&self, ctx: &mut Context, theme: &Theme, mut next_fn: Option<NavFn>, length: usize) -> Box<dyn AppPage> {
        match self {
            Page::Static(page) => {
                let mut page = page.clone();
                page.update(length, next_fn.take());
                page.build(ctx, theme)
            }
            Page::Refreshing(page_builder) => Box::new(Listener::new(
                ctx, theme, page_builder.clone(), false, next_fn.take(), length,
            )),
        }
    }

    pub fn build_root(&self, ctx: &mut Context, theme: &Theme) -> Box<dyn AppPage> {
        match self {
            Page::Static(page) => page.clone().build_root(theme, ctx.window.is_mobile),
            Page::Refreshing(page_builder) => {
                Box::new(Listener::new(ctx, theme, page_builder.clone(), true, None, 0))
            }
        }
    }

    pub fn group_info(room: Instance<ChatRoom>) -> Page {
        Page::Refreshing(Box::new(Watching::new(
            (ContractWatch::new(room), ListWatch::<Profile>::default(),
             ListWatch::<Block>::default(), ListWatch::<RoomNotificationState>::default()),
            |ctx: &mut Context, _: &Theme, (room, _, _, _): (Instance<ChatRoom>, _, _, _)| {
                PageType::chat_info(ctx, room)
            },
        )))
    }

    pub fn chat_info(room: Instance<ChatRoom>) -> Page {
        Page::Refreshing(Box::new(Watching::new(
            (ContractWatch::new(room), ListWatch::<Profile>::default(),
             ListWatch::<Block>::default(), ListWatch::<RoomNotificationState>::default()),
            |ctx: &mut Context, _: &Theme, (room, _, _, _): (Instance<ChatRoom>, _, _, _)| {
                PageType::chat_info(ctx, room)
            },
        )))
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
    Messaging { room: Instance<ChatRoom>, flow_len: usize },
    Camera { on_capture: Box<dyn PhotoCaptured> },
    Photo { photo: Photo, header: Option<(Icons, Action)> },
    Video { video: Video, header: Option<(Icons, Action)> },
    Playback { title: String, player: prism::hardware::Player, size: (u32, u32) },
}

impl PageType {
    pub fn root(
        title: &str, input: Vec<Input>, display: Vec<Display>,
        header: Option<(Icons, Action)>,
        bumper_a: Option<(String, Action)>,
        bumper_b: Option<(String, Action)>,
    ) -> Self {
        PageType::Root { title: title.to_string(), input, display, header, bumper_a, bumper_b }
    }

    pub fn display_and_input(
        title: &str, inputs: Vec<Input>, display: Vec<Display>,
        header: Option<(Icons, Action)>, bumper: Bumper,
    ) -> Self {
        Self::Content {
            title: title.into(),
            items: inputs.into_iter().map(Item::Input)
                .chain(display.into_iter().map(Item::Display)).collect(),
            header, bumper, offset: Offset::Start, flow_len: 1, next: None,
        }
    }

    pub fn content(title: &str, items: Vec<Item>, header: Option<(Icons, Action)>, bumper: Bumper, offset: Offset) -> Self {
        Self::Content {title: title.into(), items, header, bumper, offset, flow_len: 1, next: None}
    }

    pub fn display(title: &str, items: Vec<Display>, header: Option<(Icons, Action)>, bumper: Bumper, offset: Offset) -> Self {
        Self::Content {
            title: title.into(), items: items.into_iter().map(Item::Display).collect(),
            header, bumper, offset, flow_len: 1, next: None,
        }
    }

    pub fn input(title: &str, item: Input, header: Option<(Icons, Action)>, bumper: Bumper) -> Self {
        let offset = item.offset();
        Self::Content {title: title.into(), items: vec![Item::Input(item)], header, bumper, offset, flow_len: 1, next: None}
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

    pub fn edit(
        title: &str, header: Option<(Icons, Action)>, items: Vec<FormItem>,
        display: Vec<Display>, on_save: Box<dyn FormSubmit>,
    ) -> Self {
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

    pub fn messaging(room: Instance<ChatRoom>) -> Self { PageType::Messaging { room, flow_len: 1 } }

    pub fn camera(on_capture: impl PhotoCaptured + 'static) -> Self {
        Self::Camera {on_capture: Box::new(on_capture)}
    }

    pub fn photo(photo: Photo, header: Option<(Icons, Action)>) -> Self {
        PageType::Photo { photo, header }
    }

    pub fn video(video: Video, header: Option<(Icons, Action)>) -> Self {
        PageType::Video { video, header }
    }

    /// Reuse CHK's video page with an application-prepared playback timeline.
    pub fn playback(title: &str, player: prism::hardware::Player, size: (u32, u32)) -> Self {
        Self::Playback { title: title.into(), player, size }
    }

    pub fn scan_qr(instructions: &str, alt: Option<(String, Icons, Action)>, on_find: Box<dyn QrCodeFound>) -> Self {
        let item = Input::qr_code_scanner(instructions, alt, on_find);
        let offset = item.offset();
        Self::Content {
            title: "Scan QR code".into(), items: vec![Item::Input(item)], header: None,
            bumper: Bumper::None, offset, flow_len: 1, next: None,
        }
    }

    pub fn display_qr_code(title: &str, data: &str, instructions: &str, share: bool) -> Self {
        Self::Content {
            title: title.into(), items: vec![Item::Display(Display::qr_code(data, instructions))], header: None,
            bumper: Bumper::custom(
                if share { "Share" } else { "Copy" },
                if share { Action::share(data) } else { Action::copy(data) },
            ), offset: Offset::Center, flow_len: 1, next: None,
        }
    }

    pub fn qr_page(invitations: &ContactInvitations, name: Name) -> PageType {
        Self::Content {
            title: "Share AirName".into(),
            items: vec![Item::Display(Display::qr_code(&name.to_string(), "Scan to share this AirName."))],
            header: None, bumper: Bumper::custom("Share", Action::share_contact(invitations, name)),
            offset: Offset::Center, flow_len: 1, next: None,
        }
    }

    pub fn chat_info(ctx: &mut Context, room: Instance<ChatRoom>) -> PageType {
        if room.pending().is_group() {
            let active = room.pending().members.contains(&ctx.air.me());
            let items = Display::group_info(ctx, room.clone()).into_iter().map(Item::Display).collect();
            let header = active.then(|| (Icons::Edit, Action::deferred_flow(
                move |_: &mut Context, _: &Theme| {
                    Flow::new(vec![Page::Static(Box::new(Self::edit_group(room.clone())))])
                },
            )));
            return Self::Content {
                title: "Group info".into(), items, header,
                bumper: Bumper::None, offset: Offset::Start, flow_len: 1, next: None,
            };
        }
        let me = ctx.air.me();
        let name = room.pending().members.iter().copied().find(|name| *name != me).unwrap_or(me);
        let profile = Profile::from_name(ctx, name);
        if name == me {
            let profile = profile.pending();
            return Self::Content {
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
        let items = Display::profile(ctx, profile.clone(), Some(room))
            .into_iter().map(Item::Display).collect();
        let header = Some((Icons::Edit, Action::deferred_flow(move |ctx: &mut Context, _: &Theme| {
            Flow::new(vec![Page::Static(Box::new(Self::editing(
                false, profile.clone(), ctx.window.is_mobile, None,
            )))])
        })));
        Self::Content {
            title, items, header, bumper: Bumper::None,
            offset: Offset::Start, flow_len: 1, next: None,
        }
    }

    pub fn group_info(ctx: &mut Context, room: Instance<ChatRoom>) -> PageType {
        let active = {
            let data = room.pending();
            data.is_group() && data.members.contains(&ctx.air.me())
        };
        let items = Display::group_info(ctx, room.clone()).into_iter().map(Item::Display).collect();
        let header = active.then(|| (Icons::Edit, Action::deferred_flow(
            move |_: &mut Context, _: &Theme| {
                Flow::new(vec![Page::Static(Box::new(Self::edit_group(room.clone())))])
            },
        )));
        Self::Content {
            title: "Group info".into(), items, header,
            bumper: Bumper::None, offset: Offset::Start, flow_len: 1, next: None,
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
            FormComplete::Action(Action::custom(|ctx: &mut Context, _: &Theme| ctx.emit(NavigationEvent::Pop)))
        }) as Box<dyn FormSubmit>;
        Self::Edit {
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
        Self::Content {
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
        let room = ChatRoom::find_direct(ctx, name);
        let items = Display::profile(ctx, profile.clone(), room)
            .into_iter().map(Item::Display).collect();
        let header = Some((Icons::Edit, Action::deferred_flow(move |ctx: &mut Context, _: &Theme| {
            Flow::new(vec![Page::Static(Box::new(Self::editing(
                is_me, profile.clone(), ctx.window.is_mobile, None,
            )))])
        })));
        Self::Content {
            title, items, header, bumper: Bumper::None,
            offset: Offset::Start, flow_len: 1, next: None,
        }
    }

    pub fn editing(
        is_me: bool, profile: Instance<Profile>, is_mobile: bool,
        contact_actions: Option<Vec<ActionItem>>,
    ) -> PageType {
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

        Self::Edit {
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
            PageType::Root { .. } => self.build_root(theme, ctx.window.is_mobile),
            PageType::Content { title, items, offset, header, bumper, next, flow_len } => {
                Box::new(StackPage::content(
                    ctx, theme, title.to_string(), items.clone(), *offset, header.clone(),
                    bumper.clone(), next.clone(), *flow_len,
                ))
            }
            PageType::Form { id, title, item, next, flow_len: _, validate, on_submit } => {
                let content = Content::new(Offset::Start,
                    item.build(theme, ctx.window.is_mobile).unwrap_or_default(), validate.clone());
                Box::new(FormPage::new(
                    theme, id.clone(), title.to_string(), content, next.clone(), on_submit.clone(),
                ))
            }
            PageType::Edit { title, header, items, display, on_save, flow_len: _ } => {
                Box::new(EditPage::edit_and_display(
                    theme, ctx.window.is_mobile, title.to_string(), header.clone(),
                    items.clone(), display.clone(), on_save.clone(),
                ))
            }
            PageType::Review { title, getter, next, flow_len, on_submit } => {
                Box::new(ReviewPage::new(
                    theme, title.to_string(), getter.clone(), next.clone(), *flow_len,
                    on_submit.clone(),
                ))
            }
            PageType::Success { title, getter, flow_len, on_submit } => Box::new(SuccessPage::new(
                theme, title.to_string(), getter.clone(), *flow_len, on_submit.clone(),
            )),
            PageType::Messaging { room, flow_len } => {
                Box::new(MessagesPage::new(ctx, theme, room.clone(), *flow_len))
            }
            PageType::Camera { on_capture } => Box::new(CameraPage::new(theme, on_capture.clone())),
            PageType::Photo { photo, header } => {
                Box::new(PhotoPage::new(ctx, theme, photo.clone(), header.clone()))
            }
            PageType::Video { video, header } => {
                Box::new(VideoPage::new(ctx, theme, video.clone(), header.clone()))
            }
            PageType::Playback { title, player, size } => {
                Box::new(VideoPage::from_player(theme, title, player.clone(), *size))
            }
        }
    }
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
