use ramp::prism;

use pelican_ui::event::{OnEvent, Event, TickEvent};
use pelican_ui::drawable::{Component, Drawable, SizedTree};
use pelican_ui::{Context, Callback, drawables};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::canvas::Align;
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::components::text::{ExpandableText, TextSize, TextStyle};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::components::MessageGroups;

use crate::messages::{ChatRoom, Message, SendMessage};
use crate::profiles::{Profile, ChangeNotes, ChangeUsername, ChangeAvatar};
use crate::{Page, PageBuilder, ActionItem, PageType, FormItem, Bumper, Listener};
use crate::flow::Flow;
use crate::form::{State, FormValidState, FormComplete};
use crate::items::{Action, Display, AvatarPurpose};
use crate::closure::{FormSubmit, NavFn, ReviewItemGetter, SuccessGetter};

use air::Instance;
use air::names::{Id, Name};

use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Success {
    pub title: String,
    pub getter: Box<dyn SuccessGetter>,
}

impl Success {
    pub fn new(title: &str, getter: impl SuccessGetter + 'static) -> Self {
        Success{title: title.to_string(), getter: Box::new(getter)}
    }
}

#[derive(Debug, Clone)]
pub struct Review {
    pub title: String,
    pub getter: Box<dyn ReviewItemGetter>,
}

impl Review {
    pub fn new(title: &str, getter: impl ReviewItemGetter + 'static) -> Self {
        Review{title: title.to_string(), getter: Box::new(getter)}
    }
}

#[derive(Debug, Component, Clone)]
pub struct ReviewPage(Stack, pub PelicanPage, #[skip] Box<dyn ReviewItemGetter>, #[skip] Theme, #[skip] Option<NavFn>, #[skip] Box<dyn FormSubmit>, #[skip] bool);
impl OnEvent for ReviewPage {}
impl AppPage for ReviewPage {}
impl ReviewPage {
    pub fn new(theme: &Theme, title: String, item_getter: Box<dyn ReviewItemGetter>, next: Option<NavFn>, _flow_len: usize, on_submit: Box<dyn FormSubmit>) -> Self {
        let header = Header::stack(theme, &title, None);
        let bumper = PelicanBumper::stack(theme, None, Box::new(|_ctx: &mut Context, _theme: &Theme| {}), None);
        let page = PelicanPage::new(header, Content::new(Offset::Start, Vec::new(), Box::new(|_, _| true)), Some(bumper));
        ReviewPage(Stack::default(), page, item_getter, theme.clone(), next.clone(), on_submit.clone(), false)
    }

    pub fn on_change(&mut self, new: Vec<State>) {
        if !self.6 {
            self.6 = true;
            let theme = &self.3;
            let items = (self.2)(&new);
            let content = items.into_iter().filter_map(|mut i| i.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
            self.1.content = Content::new(Offset::Start, content, Box::new(|_, _| true));
        }

        let mut on_submit = self.5.clone();
        *self.1.bumper.as_mut().unwrap().on_click()[0] = {
            let mut on_click = self.4.clone().map(|n| {
                Box::new(move |ctx: &mut Context, theme: &crate::Theme| {
                    (on_submit)(ctx, &new);
                    (n.borrow_mut())(ctx, theme);
                }) as Box<dyn Callback>
            }).unwrap_or(Box::new(|_ctx: &mut Context, _theme: &crate::Theme| {}));
            let theme = self.3.clone();
            Box::new(move |ctx: &mut Context| (on_click)(ctx, &theme))
        }
    }
}

#[derive(Debug, Component, Clone)]
pub struct SuccessPage(Stack, pub PelicanPage, #[skip] Box<dyn SuccessGetter>, #[skip] Theme, #[skip] bool, #[skip] Option<Box<dyn FormSubmit>>);
impl OnEvent for SuccessPage {}
impl AppPage for SuccessPage {}
impl SuccessPage {
    pub fn new(theme: &Theme, title: String, getter: Box<dyn SuccessGetter>, flow_len: usize, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        let header = Header::stack_end(theme, &title);
        let bumper = Some(PelicanBumper::stack_end(theme, Some(flow_len)));
        let page = PelicanPage::new(header, Content::new(Offset::Center, vec![], Box::new(|_, _| true)), bumper);
        SuccessPage(Stack::default(), page, getter, theme.clone(), false, on_submit.clone())
    }

    pub fn on_change(&mut self, ctx: &mut Context, new: Vec<State>) {
        if !self.4 {
            self.4 = true;
            use pelican_ui::colors;
            use pelican_ui::components::Icon;
            if let Some(on_submit) = &mut self.5 {(on_submit)(ctx, &new);}
            let theme = self.3.clone();
            let (icon, description) = (self.2)(new);
            self.1.content = Content::new(Offset::Center, drawables![
                Icon::new(&theme, icon, Some(theme.colors().get(colors::Text::Heading)), 128.0),
                ExpandableText::new(&theme, &description, TextSize::H4, TextStyle::Heading, Align::Center, None)
            ], Box::new(|_, _| true));
        }
    }
}

#[derive(Debug, Component, Clone)]
pub struct MessagesPage {
    layout: Stack,
    page: PelicanPage,
}

impl OnEvent for MessagesPage {}
impl AppPage for MessagesPage {}

impl MessagesPage {
    pub fn new(ctx: &mut Context, theme: &Theme, mut room: Instance<ChatRoom>, flow_len: usize ) -> Self {
        let room_data = room.load_pending().clone();
        let my_name = ctx.me();

        let mut profiles = room_data.members.clone().into_iter().filter(|n| *n != my_name)
            .map(|n| Profile::from_name(ctx, n)).collect::<Vec<_>>();

        let deref_profiles = profiles.iter_mut()
            .map(|p| p.load_pending().clone()).collect::<Vec<Profile>>();

        let is_group = deref_profiles.len() > 1;

        let pel_profiles = deref_profiles.iter()
            .map(|p| p.to_pel()).collect::<Vec<_>>();

        let info = match (is_group, profiles.first().cloned()) {
            (false, Some(profile)) => {
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    let mut profile = profile.clone();
                    (Flow::new(vec![Page::profile(&mut profile)]).build(ctx, theme))(ctx, theme);
                }) as Box<dyn Callback>
            }
            _ => Box::new(move |ctx: &mut Context, theme: &Theme| {
                let profiles = profiles.clone();
                let t = theme.clone();
                (Flow::new(vec![
                    Page::Static(GroupMessageInfoPage::new(ctx, &t, profiles.clone()))
                ]).build(ctx, theme))(ctx, theme);
            }) as Box<dyn Callback>,
        };

        let header = Header::messaging(ctx, theme, pel_profiles, flow_len, info);

        let mut room_taken = room.clone();
        let bumper = PelicanBumper::input(theme, "Message...",  move |_ctx: &mut Context, val: &mut String| {
            if !val.is_empty() {
                room_taken.apply(SendMessage(val.to_string()));
            }
        });

        let messages = room_data.messages.iter()
            .map(|message| message.to_pel(ctx)).collect::<Vec<_>>();

        let content = Content::new(
            Offset::End,
            drawables![MessageGroups::new(ctx, theme, messages.clone(), is_group, false)],
            Box::new(|_, _| true),
        );

        let page = PelicanPage::new(header, content, Some(bumper));

        MessagesPage { layout: Stack::default(), page }
    }
}

#[derive(Debug, Clone)]
pub struct ViewMessages(Instance<ChatRoom>, ChatRoom, Vec<Profile>);

impl ViewMessages {
    pub fn new(ctx: &mut Context, room: &mut Instance<ChatRoom>) -> Self {
        let profiles = room.load_pending().members.iter().map(|m| Profile::from_name(ctx, *m).load_pending().clone()).collect::<Vec<_>>();
        ViewMessages(room.clone(), room.load_pending().clone(), profiles)
    }
}

impl PageBuilder for ViewMessages {
    fn poll(&mut self, ctx: &mut Context) -> bool {
        let current = self.0.load_pending().clone();
        let profiles = current.members.iter().map(|m| Profile::from_name(ctx, *m).load_pending().clone()).collect::<Vec<_>>();
        let has_changed = current != self.1 || profiles != self.2;
        if has_changed {self.1 = current;}
        has_changed
    }

    fn build(&mut self, _ctx: &mut Context, _theme: &Theme) -> PageType {
        PageType::messaging(self.0.clone())
    }
}

pub struct GroupMessageInfoPage;
impl GroupMessageInfoPage {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(ctx: &mut Context, theme: &Theme, profiles: Vec<Instance<Profile>>) -> PageType {
        let theme = theme.clone();
        let items = profiles.clone().into_iter().flat_map(|mut profile| {
            let mut p = profile.clone();
            let deref = profile.load_pending();
            if deref.name.unwrap() != ctx.me() {
                let view_contact = Flow::new(vec![Page::profile(&mut p)]);
                Some(crate::ListItem::avatar(deref.avatar.clone(), &deref.username, &deref.name(), None, Some(view_contact)))
            } else {None}
        }).collect::<Vec<crate::ListItem>>();
        PageType::display("Group members", vec![
            Display::instructions(&format!("This group has {} members.", profiles.len())),
            Display::list(None, items, None),
        ], None, Bumper::None, Offset::Start)
    }
}

pub struct ProfilePage;
impl ProfilePage {
    pub fn new(ctx: &mut Context, theme: &Theme, mut profile: Instance<Profile>) -> Box<dyn AppPage> {
        let my_name = profile.load_pending().name.unwrap();
        let is_me = my_name == ctx.me();
        match is_me {
            true => ProfilePage::editing(theme, is_me, profile).build_root(ctx, theme),
            false => ProfilePage::view_only(ctx, theme, profile, is_me)
        }
    }

    pub fn view_only(ctx: &mut Context, theme: &Theme, mut p: Instance<Profile>, is_me: bool) -> Box<dyn AppPage> {
        Box::new(Listener::new(ctx, theme, Page::profile(&mut p).builder().unwrap(), false))
    }

    pub fn editing(theme: &Theme, is_me: bool, mut profile: Instance<Profile>) -> PageType {
        let mut p = profile.clone();
        let closure = Box::new(move |ctx: &mut Context, objects: &Vec<State>| {
            println!("Saving profile");
            if let Some(State::Text(result)) = objects.get(1) {
                p.apply(ChangeUsername(result.to_string()));
            }
            if let Some(State::Text(result)) = objects.get(2) {
                p.apply(ChangeNotes(result.to_string()));
            }
            if let Some(State::Avatar(result)) = objects.get(0) {
                p.apply(ChangeAvatar(result.clone()));
            }
            FormComplete::None
        }) as Box<dyn FormSubmit>;
        
        let mut avatar = profile.clone();
        let mut username = profile.clone();
        let mut notes = profile.clone();
        let p = profile.load_pending();
        let my_name = p.name.unwrap();
        let title = if is_me {"My profile"} else {"Edit profile"};
        let display = if is_me {vec![
            Display::cta("Orange name", None, &my_name.to_string(), vec![
                ActionItem::new_with_active(Action::copy(&my_name.to_string()), "Copy", "Copied", Icons::Copy),
                ActionItem::new(Action::flow(Flow::new(vec![
                    Page::Static(PageType::display_qr_code("Share name", &my_name.to_string(), "Scan to share orange name."))
                ])), "Display QR Code", Icons::QrCode)
            ]),
        ]} else {vec![]};
        
        PageType::edit_and_display(title, 
            vec![
                FormItem::avatar_with_preset("Avatar", p.avatar.clone(), move |ctx: &mut Context, a: String| {
                    let current = avatar.load_pending().avatar.get_image().unwrap_or_default();
                    match current == a {
                        true => FormValidState::Valid,
                        false => FormValidState::Invalid,
                    }
                }),
                FormItem::text_with_preset("Username", &p.username.clone(), None, move |ctx: &mut Context, a: String| {
                    match a.as_str() {
                        a if a == &username.load_pending().username => FormValidState::Valid,
                        "" => FormValidState::InvalidWithData("Username cannot be empty".to_string()),
                        _ => FormValidState::Invalid,
                    }
                }),
                FormItem::text_with_preset("About me", &p.notes, None, move |ctx: &mut Context, a: String| {
                    match notes.load_pending().notes == a {
                        true => FormValidState::Valid,
                        false => FormValidState::Invalid,
                    }
                })
            ], 
            display,
            closure
        )
    }
}

// impl ProfileView {
//     Listener::new(ctx, theme, listener_p, profile, page, |ctx: &mut Context, theme: &Theme, page: PageType| page.build(ctx, theme))
// }

pub struct ProfileView;
impl ProfileView {
    pub fn new(ctx: &mut Context, theme: &Theme, mut profile: Instance<Profile>) -> PageType {
        let p = profile.clone();
        let profile = profile.load_pending().clone();
        let saved = p.clone();

        let is_me = profile.name.unwrap() == ctx.me();
        let my_name = profile.name.unwrap();
        let about_me = if profile.notes.is_empty() {"No bio yet."} else {&profile.notes};
        PageType::display(&profile.username,
            vec![
                Display::avatar(profile.avatar.clone(), AvatarPurpose::None),
                Display::actions(vec![
                    ActionItem::new(Action::None, "Bitcoin", Icons::Bitcoin),
                    ActionItem::new(Action::message(my_name), "Message", Icons::Messages),
                    ActionItem::new(Action::unblock(&theme, p.clone()), "Block", Icons::Block),
                ], true),
                Display::cta("About me", None, about_me, vec![]),
                Display::cta("Orange name", None, &my_name.to_string(), vec![
                    ActionItem::new_with_active(Action::copy(&my_name.to_string()), "Copy", "Copied", Icons::Copy),
                    ActionItem::new(Action::flow(Flow::new(vec![
                        Page::Static(PageType::display_qr_code("Share name", &my_name.to_string(), "Scan to share orange name."))
                    ])), "Display QR Code", Icons::QrCode)
                ]),
            ], 
            Some((Icons::Edit, Box::new(move |ctx: &mut Context, theme: &Theme| {
                let p = p.clone();
                let t = theme.clone();
                Flow::new(vec![Page::Static(ProfilePage::editing(&t.clone(), is_me, p.clone()))])
            }))), 
            Bumper::None, Offset::Start,
        )
    }
}
