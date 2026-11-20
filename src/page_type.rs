use prism::air::{Instance, Name};
use prism::drawable::Drawable;
use prism::Context;
use ptsd::layout::Offset;
use ptsd::navigation::{AppPage, NavigationEvent};
use ptsd::utils::ValidationFn;
use uuid::Uuid;

use crate::closure::{FormSubmit, NavFn};
use crate::design_system::{
    AvatarIconStyle, Callback, Content, Footer, Header, Icons, PageContent, PageLifecycle,
    PageView, QrCodeFound, Theme,
};
use crate::form::{FormHost, FormItem, FormValues};
use crate::items::{Action, Display, Input, Item};
use crate::messages::{ChatRoom, ChatRoomAction, Photo, Video};
use crate::profiles::Profile;
use crate::{
    ActionItem, AvatarPurpose, CameraCapture, ContactInvitations, Flow, FormComplete, FormValidState,
    Page, PhotoCaptured, StackPage, VideoPlayback,
};

#[derive(Debug, Clone)]
pub enum Bumper {
    Default,
    Custom { label: String, action: Action, secondary: Option<(String, Action)> },
    Footer(Footer),
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

#[derive(Clone, Debug)]
pub enum PageType {
    Edit {
        title: String,
        header: Option<(Icons, Action)>,
        items: Vec<FormItem>,
        display: Vec<Display>,
        on_save: Box<dyn FormSubmit>,
    },
    Content {
        header: Header,
        items: Vec<Item>,
        offset: Offset,
        lifecycle: Option<Box<dyn PageLifecycle>>,
        bumper: Bumper,
        next: Option<NavFn>,
        flow_len: usize,
        identity: Option<String>,
        reveal_child: Option<usize>,
    },
    Form {
        id: String,
        title: String,
        item: Input,
        next: Option<NavFn>,
        validate: Box<dyn ValidationFn>,
        on_submit: Option<Box<dyn FormSubmit>>,
    },
    EdgeToEdge { header: Header, content: Box<dyn PageContent> },
}

impl PageType {
    pub fn root(
        title: &str, input: Vec<Input>, display: Vec<Display>, header: Option<(Icons, Action)>,
        bumper_a: Option<(String, Action)>, bumper_b: Option<(String, Action)>,
    ) -> Self {
        let items = input.into_iter().map(Item::Input)
            .chain(display.into_iter().map(Item::Display)).collect::<Vec<_>>();
        let offset = match items.iter().find_map(|item| match item {
            Item::Display(display) => Some(display), _ => None,
        }) {
            Some(Display::List { .. }) => Offset::Start,
            _ if items.len() <= 1 => Offset::Center, _ => Offset::Start,
        };
        let action = |(label, action): (String, Action)| {
            (ActionItem::format_label(&label), action.get())
        };
        let bumper = if bumper_a.is_some() || bumper_b.is_some() {
            Bumper::Footer(Footer::home(bumper_a.map(action), bumper_b.map(action)))
        } else {Bumper::None};
        PageType::Content {
            header: Header::home(title, header.map(|(icon, action)| (icon, action.get()))),
            items, bumper, offset, flow_len: 1, next: None, lifecycle: None,
            identity: None, reveal_child: None,
        }
    }

    pub fn content(title: &str, items: Vec<Item>, header: Option<(Icons, Action)>, bumper: Bumper, offset: Offset) -> Self {
        PageType::Content {
            header: Header::stack(title, header.map(|(icon, action)| (icon, action.get()))),
            items, bumper, offset, flow_len: 1, next: None, lifecycle: None,
            identity: None, reveal_child: None,
        }
    }

    pub fn display(title: &str, items: Vec<Display>, header: Option<(Icons, Action)>, bumper: Bumper, offset: Offset) -> Self {
        PageType::Content {
            header: Header::stack(title, header.map(|(icon, action)| (icon, action.get()))),
            items: items.into_iter().map(Item::Display).collect(), bumper, offset,
            flow_len: 1, next: None, lifecycle: None, identity: None, reveal_child: None,
        }
    }

    pub fn form(id: &str, title: &str, item: Input, validate: Box<dyn ValidationFn>, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        PageType::Form {
            id: id.into(),
            title: title.to_string(),
            validate, item,
            next: None,
            on_submit,
        }
    }

    pub fn edit(title: &str, header: Option<(Icons, Action)>, items: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        PageType::Edit {
            title: title.to_string(),
            header, items, display, on_save,
        }
    }

    pub fn edge_to_edge(title: &str, content: impl Drawable, action: Option<(Icons, Action)>) -> Self {
        let content = Box::new(content) as Box<dyn Drawable>;
        PageType::EdgeToEdge {
            header: Header::stack(title, action.map(|(icon, action)| (icon, action.get()))),
            content: Box::new(move |_: &mut Context, _: &Theme| content.clone()),
        }
    }

    pub fn camera(on_capture: impl PhotoCaptured + 'static) -> Self {
        let on_capture = Box::new(on_capture) as Box<dyn PhotoCaptured>;
        PageType::EdgeToEdge {
            header: Header::stack("Camera", None),
            content: Box::new(move |_: &mut Context, theme: &Theme| -> Box<dyn Drawable> {
                Box::new(CameraCapture::new(theme, on_capture.clone()))
            }),
        }
    }

    pub fn photo(photo: Photo, header: Option<(Icons, Action)>) -> Self {
        PageType::EdgeToEdge {
            header: Header::stack("Photo", header.map(|(icon, action)| (icon, action.get()))),
            content: Box::new(move |_: &mut Context, theme: &Theme| theme.photo_view(&photo)),
        }
    }

    pub fn video(video: Video, header: Option<(Icons, Action)>) -> Self {
        PageType::EdgeToEdge {
            header: Header::stack("Video", header.map(|(icon, action)| (icon, action.get()))),
            content: Box::new(move |ctx: &mut Context, theme: &Theme| -> Box<dyn Drawable> {
                Box::new(VideoPlayback::new(ctx, theme, video.clone()))
            }),
        }
    }

    /// Reuse CHK's video page with an application-prepared playback timeline.
    pub fn playback(title: &str, player: prism::hardware::Player, size: (u32, u32)) -> Self {
        PageType::EdgeToEdge {
            header: Header::stack(title, None),
            content: Box::new(move |_: &mut Context, theme: &Theme| -> Box<dyn Drawable> {
                Box::new(VideoPlayback::from_player(theme, player.clone(), size))
            }),
        }
    }

    pub fn scan_qr(instructions: &str, alt: Option<(String, Icons, Action)>, on_find: Box<dyn QrCodeFound>) -> Self {
        let item = Input::qr_code_scanner(instructions, alt, on_find);
        let offset = item.offset();
        PageType::Content {
            header: Header::stack("Scan QR code", None), items: vec![Item::Input(item)],
            bumper: Bumper::None, offset, flow_len: 1, next: None, lifecycle: None,
            identity: None, reveal_child: None,
        }
    }

    pub fn display_qr_code(title: &str, data: &str, instructions: &str, share: bool) -> Self {
        PageType::Content {
            header: Header::stack(title, None),
            items: vec![Item::Display(Display::qr_code(data, instructions))],
            bumper: Bumper::custom(
                if share { "Share" } else { "Copy" },
                if share { Action::share(data) } else { Action::copy(data) },
            ), offset: Offset::Center, flow_len: 1, next: None, lifecycle: None,
            identity: None, reveal_child: None,
        }
    }

    pub fn qr_page(invitations: &ContactInvitations, name: Name) -> PageType {
        PageType::Content {
            header: Header::stack("Share AirName", None),
            items: vec![Item::Display(Display::qr_code(
                &name.to_string(), "Scan to share this AirName.",
            ))],
            bumper: Bumper::custom("Share", Action::share_contact(invitations, name)),
            offset: Offset::Center, flow_len: 1, next: None, lifecycle: None,
            identity: None, reveal_child: None,
        }
    }

    pub fn chat_info(invitations: &ContactInvitations, ctx: &mut Context, room: Instance<ChatRoom>) -> PageType {
        if room.pending().is_group() {
            let active = room.pending().members.contains(&ctx.air.me());
            let items = Display::group_info(invitations, ctx, room.clone()).into_iter()
                .map(Item::Display).collect();
            let header = active.then(|| (Icons::Edit, Action::deferred_flow(
                move |_: &mut Context, _: &Theme| {
                    Flow::new(vec![Page::Static(Box::new(PageType::edit_group(room.clone())))])
                },
            )));
            return PageType::Content {
                header: Header::stack("Group info", header.map(|(icon, action)| (icon, action.get()))),
                items, bumper: Bumper::None, offset: Offset::Start,
                flow_len: 1, next: None, lifecycle: None, identity: None, reveal_child: None,
            };
        }
        let me = ctx.air.me();
        let name = room.pending().members.iter().copied().find(|name| *name != me).unwrap_or(me);
        let profile = Profile::from_name(ctx, name);
        if name == me {
            let profile = profile.pending();
            return PageType::Content {
                header: Header::stack("Chat info", None),
                items: vec![
                    Item::Display(Display::avatar(profile.avatar.clone(), AvatarPurpose::None)),
                    Item::Display(Display::label(&format!("{} (You)", profile.username))),
                    Item::Display(Display::actions(vec![ActionItem::mute(ctx, room)], true)),
                ],
                bumper: Bumper::None, offset: Offset::Start, flow_len: 1, next: None,
                lifecycle: None, identity: None, reveal_child: None,
            };
        }
        let title = profile.pending().username.clone();
        let items = Display::profile(invitations, ctx, profile.clone())
            .into_iter().map(Item::Display).collect();
        let header = Some((Icons::Edit, Action::deferred_flow(move |ctx: &mut Context, _: &Theme| {
            Flow::new(vec![Page::Static(Box::new(Profile::editing(
                false, profile.clone(), ctx.window.is_mobile, None,
            )))])
        })));
        PageType::Content {
            header: Header::stack(&title, header.map(|(icon, action)| (icon, action.get()))),
            items, bumper: Bumper::None, offset: Offset::Start,
            flow_len: 1, next: None, lifecycle: None, identity: None, reveal_child: None,
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
                FormItem::avatar_with_preset("photo", "Group photo", photo, Action::PickPhoto,
                    |_: &mut Context, _: String| FormValidState::Valid),
                FormItem::text_with_preset(
                    "name", Uuid::new_v4(), "Group name", &name, None,
                    |_: &mut Context, name: String| {
                        if name.trim().is_empty() {
                            FormValidState::InvalidWithData("Enter a group name.".into())
                        } else if name.trim().chars().count() > 80 {
                            FormValidState::InvalidWithData("Use 80 characters or fewer.".into())
                        } else {FormValidState::Valid}
                    },
                ),
            ],
            display: vec![], on_save: save,
        }
    }

    pub fn leave(mut room: Instance<ChatRoom>) -> PageType {
        let details = room.pending().group_details();
        PageType::Content {
            header: Header::stack("Leave group", None),
            items: vec![
                Item::Display(Display::avatar(details.photo, AvatarPurpose::Custom {
                    icon: Icons::Leave, style: AvatarIconStyle::Danger,
                })),
                Item::Display(Display::confirmation_message(&format!("Leave {}?", details.name))),
            ],
            bumper: Bumper::double("Leave", Action::custom(move |ctx: &mut Context, _: &Theme| {
                room.send(ChatRoomAction::LeaveGroup);
                room.notify(false);
                ctx.emit(NavigationEvent::Root(None));
            }), "Cancel", Action::None),
            offset: Offset::Center, flow_len: 1, next: None, lifecycle: None,
            identity: None, reveal_child: None,
        }
    }

    pub fn nav_fn(&mut self) -> Option<&mut Option<NavFn>> {
        match self {
            PageType::EdgeToEdge { .. } | PageType::Edit { .. } => None,
            PageType::Content { next, .. }
            | PageType::Form { next, .. } => Some(next),
        }
    }

    pub fn length(&mut self) -> Option<&mut usize> {
        match self {
            PageType::Content { flow_len, .. } => Some(flow_len),
            _ => None,
        }
    }

    pub fn update(&mut self, new_len: usize, new_fn: Option<NavFn>) {
        if let Some(l) = self.length() { *l = new_len; }
        if let Some(nav) = self.nav_fn() { *nav = new_fn.clone(); }
    }

    pub fn build(&self, ctx: &mut Context, theme: &Theme) -> Box<dyn AppPage> {
        match self {
            PageType::Content { .. } => Box::new(self.build_content(ctx, theme, false)),
            PageType::Form { id, title, item, next, validate, on_submit } => {
                let content = Content::new(Offset::Start,
                    item.build(theme, ctx.window.is_mobile), validate.clone());
                Box::new(FormHost::build_form(
                    ctx, theme, id.clone(), title, content, next.clone(), on_submit.clone(),
                ))
            }
            PageType::Edit { .. } => Box::new(FormHost::build_edit(self, ctx, theme, false)),
            PageType::EdgeToEdge { header, content } => {
                Box::new(StackPage::edge_to_edge(theme, header.clone(), content(ctx, theme)))
            }
        }
    }

    pub fn build_content(&self, ctx: &mut Context, theme: &Theme, is_root: bool) -> StackPage<PageView> {
        let PageType::Content {
            mut header, items, offset, bumper, next, flow_len, lifecycle, identity, reveal_child,
        } = self.clone() else {unreachable!()};
        if let Header::Messaging {steps, ..} = &mut header {*steps = flow_len;}
        if is_root {
            header = match header {
                Header::Stack {title, action} => Header::home(&title, action),
                Header::End {title} => Header::home(&title, None),
                header => header,
            };
        }
        let callbacks = lifecycle.map(|build| build(ctx, theme, next.clone(), flow_len));
        let footer = match bumper {
            Bumper::Custom { label, action, secondary } => {
                let on_click = match next {
                    Some(next) => Box::new(move |ctx: &mut Context, theme: &Theme| {
                        (next.borrow_mut())(ctx, theme);
                        (action.clone().get())(ctx, theme);
                    }) as Box<dyn Callback>,
                    None => Box::new(move |ctx: &mut Context, theme: &Theme| {
                        (action.clone().get())(ctx, theme)
                    }) as Box<dyn Callback>,
                };
                let secondary = secondary.map(|(label, action)| {
                    (
                        ActionItem::format_label(&label),
                        Box::new(move |ctx: &mut Context, theme: &Theme| {
                            (action.clone().get())(ctx, theme);
                            if !is_root {ctx.emit(NavigationEvent::Pop);}
                        }) as Box<dyn Callback>,
                    )
                });
                Some(Footer::stack(Some(&ActionItem::format_label(&label)), on_click, secondary))
            }
            Bumper::Default => match next {
                Some(next) => Some(Footer::stack(None,
                    Box::new(move |ctx: &mut Context, theme: &Theme| {
                        (next.borrow_mut())(ctx, theme);
                    }), None)),
                None => {
                    if let Header::Stack {title, ..} = header {header = Header::stack_end(&title);}
                    Some(Footer::stack_end(Some(flow_len)))
                }
            },
            Bumper::Done => {
                if let Header::Stack {title, ..} = header {header = Header::stack_end(&title);}
                Some(Footer::stack_end(Some(flow_len)))
            }
            Bumper::None => None,
            Bumper::Footer(footer) => Some(footer),
        };
        let (on_event, on_show, on_create) = callbacks.unwrap_or((None, None, None));
        StackPage::content(
            ctx, theme, header, items, offset, footer, identity, reveal_child,
            on_event, on_show, on_create,
        )
    }

    pub fn build_root(&self, ctx: &mut Context, theme: &Theme) -> Box<dyn AppPage> {
        match self {
            PageType::Content { .. } => Box::new(self.build_content(ctx, theme, true)),
            PageType::Edit { .. } => Box::new(FormHost::build_edit(self, ctx, theme, true)),
            PageType::Form { .. } | PageType::EdgeToEdge { .. } => {
                panic!("Not an accepted root type");
            }
        }
    }
}
