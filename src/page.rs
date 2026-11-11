use prism::air::{Instance, Contract};
use pelican_ui::Context;
use pelican_ui::components::QrCodeFound;
use ptsd::layout::Offset;
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::utils::ValidationFn;

use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

use crate::blocking::Block;
use crate::Listener;
use crate::form::FormItem;
use crate::items::{Action, Input, Display, Item};
use crate::closure::{
    PageBuilderContractFn, PageBuilderContractMultiplesFn, FormSubmit, NavFn, ReviewItemGetter,
    SuccessGetter,
};
use crate::messages::{ChatRoom, Photo, Video};
use crate::profiles::{Profile, ProfileAction};
use crate::{ContactInvitations, Flow, ListItem, ActionItem, AvatarPurpose, AvatarIconStyle, Form, FormValues, FormComplete, FormValidState};
use prism::air::Name;
use crate::messages::ChatRoomAction;
use pelican_ui::navigation::NavigationEvent;

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

#[derive(Debug, Clone)]
pub enum Page { Static(Box<PageType>), Refreshing(Box<dyn PageBuilder>) }

impl Page {
    pub fn watching<W: Watcher>(watcher: W, builder: impl FnMut(&mut Context, &Theme, W::Output) -> PageType + Clone + 'static) -> Self {
        Page::Refreshing(Box::new(Watching::new(watcher, builder)))
    }

    pub fn contract_updates<C: Contract>(contract: Instance<C>, builder: impl PageBuilderContractFn<C> + 'static) -> Self {
        let builder: Box<dyn PageBuilderContractFn<C>> = Box::new(builder);
        Page::watching(Watch::contract(contract), builder)
    }

    pub fn updates_list_changes<C: Contract>(_ctx: &mut Context, builder: impl PageBuilderContractMultiplesFn<C> + 'static) -> Self {
        let builder: Box<dyn PageBuilderContractMultiplesFn<C>> = Box::new(builder);
        Page::watching(Watch::list::<C>(), builder)
    }

    pub fn profile(ctx: &mut Context, profile: &mut Instance<Profile>) -> Self {
        let my_name = profile.pending().name.unwrap();
        let is_me = my_name == ctx.air.me();
        match is_me {
            true => Page::Static(Box::new(PageType::editing(
                is_me, profile.clone(), ctx.window.is_mobile,
            ))),
            false => Page::watching(
                (Watch::contract(profile.clone()), Watch::list::<Block>()),
                |ctx: &mut Context,
                 _: &Theme,
                 (profile, _blocks): (Instance<Profile>, Vec<Instance<Block>>)| {
                    PageType::profile_view(ctx, profile)
                },
            ),
        }
    }

    pub fn messaging(_ctx: &mut Context, room: &mut Instance<ChatRoom>) -> Self {
        Page::watching(
            (Watch::contract(room.clone()), Watch::list::<Block>(), Watch::list::<Profile>()),
            |_ctx: &mut Context,
             _theme: &Theme,
             (room, _blocks, _profiles): (Instance<ChatRoom>, Vec<Instance<Block>>, Vec<Instance<Profile>>)| {
                PageType::messaging(room)
            },
        )
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
        Page::watching((Watch::contract(room), Watch::list::<Profile>()),
            |ctx: &mut Context, _: &Theme, (room, _): (Instance<ChatRoom>, Vec<Instance<Profile>>)| {
                PageType::group_info(ctx, room)
            })
    }
}

pub trait PageBuilder: Debug + dyn_clone::DynClone {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<()>> + '_>>;
    fn build(&mut self, ctx: &mut Context, theme: &Theme) -> PageType;
}

dyn_clone::clone_trait_object!(PageBuilder);

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
        Self::content(title, inputs.into_iter().map(Item::Input)
            .chain(display.into_iter().map(Item::Display)).collect(), header, bumper, Offset::Start)
    }

    pub fn content(title: &str, items: Vec<Item>, header: Option<(Icons, Action)>, bumper: Bumper, offset: Offset) -> Self {
        Self::Content {title: title.into(), items, header, bumper, offset, flow_len: 1, next: None}
    }

    pub fn display(title: &str, items: Vec<Display>, header: Option<(Icons, Action)>, bumper: Bumper, offset: Offset) -> Self {
        Self::content(title, items.into_iter().map(Item::Display).collect(), header, bumper, offset)
    }

    pub fn input(title: &str, item: Input, header: Option<(Icons, Action)>, bumper: Bumper) -> Self {
        let offset = item.offset();
        Self::content(title, vec![Item::Input(item)], header, bumper, offset)
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
        PageType::input(
            "Scan QR code", Input::qr_code_scanner(instructions, alt, on_find), None,
            crate::Bumper::None,
        )
    }

    pub fn display_qr_code(title: &str, data: &str, instructions: &str, share: bool) -> Self {
        PageType::display(
            title, vec![Display::qr_code(data, instructions)], None,
            Bumper::custom(
                if share { "Share" } else { "Copy" },
                if share { Action::share(data) } else { Action::copy(data) },
            ), Offset::Center,
        )
    }

    pub fn qr_page(invitations: &ContactInvitations, name: Name) -> PageType {
        PageType::display("Share AirName",
            vec![Display::qr_code(&name.to_string(), "Scan to share this AirName.")], None,
            Bumper::custom("Share", Action::share_contact(invitations, name)), Offset::Center)
    }

    pub fn group_info(ctx: &mut Context, room: Instance<ChatRoom>) -> PageType {
        let data = room.pending().clone();
        let details = data.group_details();
        let me = ctx.air.me();
        let active = data.is_group() && data.members.contains(&me);
        let items = data.members.iter().map(|name| {
            let mut profile = Profile::from_name(ctx, *name);
            let data = profile.pending().clone();
            let title = if *name == me {format!("{} (You)", data.username)} else {data.username.clone()};
            let flow = Flow::new(vec![Page::profile(ctx, &mut profile)]);
            ListItem::avatar(data.avatar.clone(), &title, &data.name(), None, Some(Action::flow(flow)))
        }).collect();
        let mut display = vec![
            Display::avatar(details.photo.clone(), AvatarPurpose::None),
        ];
        if active {
            let add_room = room.clone();
            let leave_room = room.clone();
            display.push(Display::data_list("Group members", Some(&format!("This group has {} members.", data.members.len())), items, Some(vec![
                ActionItem::new(Action::custom(move |ctx: &mut Context, theme: &Theme| {
                    let form = Form::add_members(ctx, theme, add_room.clone());
                    (Flow::from_form(form).build(ctx, theme))(ctx, theme);
                }), "Add Members", Icons::Add),
            ])));
            display.push(Display::cta("Leave group", None, "Remove yourself from this group.", vec![
                ActionItem::new(Action::custom(move |ctx: &mut Context, theme: &Theme| {
                    (Flow::new(vec![Page::Static(Box::new(Self::leave(leave_room.clone())))])
                        .build(ctx, theme))(ctx, theme);
                }), "Leave Group", Icons::Block),
            ]));
        } else {
            display.push(Display::instructions("You are no longer a member of this group."));
            display.push(Display::data_list("Members", None, items, None));
        }
        let edit = active.then(|| (Icons::Edit, Action::deferred_flow(move |_: &mut Context, _: &Theme| {
            Flow::new(vec![Page::Static(Box::new(Self::edit_group(room.clone())))])
        })));
        PageType::display("Group info", display, edit, Bumper::None, Offset::Start)
    }

    pub fn edit_group(mut room: Instance<ChatRoom>) -> PageType {
        let original = room.pending().group_details();
        let photo = original.photo.clone();
        let name = original.name.clone();
        let save = Box::new(move |ctx: &mut Context, values: &FormValues| {
            if !room.pending().members.contains(&ctx.air.me()) {return FormComplete::Error("You are no longer a member of this group.".into());}
            if let Some(photo) = values.avatar("photo") && *photo != original.photo {
                room.send(ChatRoomAction::SetGroupPhoto(photo.clone()));
            }
            if let Some(name) = values.text("name") && name.trim() != original.name {
                room.send(ChatRoomAction::RenameGroup(name.to_owned()));
            }
            FormComplete::Action(Action::custom(|ctx: &mut Context, _: &Theme| ctx.emit(NavigationEvent::Pop)))
        }) as Box<dyn FormSubmit>;
        PageType::edit("Edit group", None, vec![
            FormItem::avatar_with_preset("Group photo", photo, Action::PickPhoto,
                |_: &mut Context, _: String| FormValidState::Valid).named("photo"),
            FormItem::text_with_preset("Group name", &name, None, |_: &mut Context, name: String| {
                if name.trim().is_empty() {FormValidState::InvalidWithData("Enter a group name.".into())}
                else if name.trim().chars().count() > 80 {FormValidState::InvalidWithData("Use 80 characters or fewer.".into())}
                else {FormValidState::Valid}
            }).named("name"),
        ], vec![], save)
    }

    pub fn leave(mut room: Instance<ChatRoom>) -> PageType {
        let details = room.pending().group_details();
        PageType::display("Leave group", vec![
            Display::avatar(details.photo, AvatarPurpose::Custom { icon: Icons::Leave, style: AvatarIconStyle::Danger }),
            Display::confirmation_message(&format!("Leave {}?", details.name)),
        ], None, Bumper::double("Leave", Action::custom(move |ctx: &mut Context, _: &Theme| {
            room.send(ChatRoomAction::LeaveGroup);
            room.notify(false);
            ctx.emit(NavigationEvent::Root(None));
        }), "Cancel", Action::None), Offset::Center)
    }

    pub fn editing(is_me: bool, profile: Instance<Profile>, is_mobile: bool) -> PageType {
        Self::editing_with_contact_actions(is_me, profile, is_mobile, None)
    }

    pub fn profile_view(ctx: &mut Context, profile: Instance<Profile>) -> PageType {
        let p = profile.clone();
        let profile = profile.pending().clone();
        let profile_name = profile.name.unwrap();
        let my_name = ctx.air.me();
        let is_me = profile_name == my_name;
        let about_me = if profile.notes.is_empty() { "No bio yet." } else { &profile.notes };

        let is_blocked = !is_me && Block::did_i_block(ctx, profile.name.unwrap());

        let block_action = match is_blocked {
            true => ActionItem::new(crate::Action::unblock(p.clone()), "Unblock", Icons::Unblock),
            false => ActionItem::new(crate::Action::block(p.clone()), "Block", Icons::Block),
        };

        PageType::display(
            &profile.username,
            vec![
                Display::avatar(profile.avatar.clone(), AvatarPurpose::None),
                Display::actions(
                    vec![
                        ActionItem::new(Action::message(profile_name), "Message", Icons::Messages),
                        block_action,
                    ], true,
                ), Display::cta("About me", None, about_me, vec![]),
                Display::cta(
                    "AirName", None, &profile_name.to_string(),
                    vec![
                        ActionItem::new_with_active(
                            Action::copy(&profile_name.to_string()), "Copy", "Copied", Icons::Copy,
                        ),
                        ActionItem::new(
                            Action::flow(Flow::new(vec![Page::Static(Box::new(PageType::display_qr_code(
                                "Share AirName", &profile_name.to_string(),
                                "Scan to share AirName.", ctx.window.is_mobile,
                            )))])), "Display QR Code", Icons::QrCode,
                        ),
                    ],
                ),
            ],
            Some((
                Icons::Edit,
                Action::deferred_flow(move |ctx: &mut Context, _: &Theme| {
                    Flow::new(vec![Page::Static(Box::new(PageType::editing(
                        is_me, p.clone(), ctx.window.is_mobile,
                    )))])
                }),
            )), Bumper::None, Offset::Start,
        )
    }

    pub(crate) fn editing_with_contact_actions(is_me: bool, profile: Instance<Profile>,
        is_mobile: bool, contact_actions: Option<Vec<ActionItem>>) -> PageType {
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
        let title = if is_me { "My profile" } else { "Edit profile" };
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

        PageType::edit(
            title, None,
            vec![
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
            ], display, closure,
        )
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
                Box::new(FormPage::new(
                    theme, ctx.window.is_mobile, id.clone(), title.to_string(), item.clone(),
                    next.clone(), validate.clone(), on_submit.clone(),
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
