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
use crate::{PageType, FormItem, Bumper};
use crate::flow::Flow;
use crate::form::State;
use crate::items::{Action, Display};
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
pub struct SuccessPage(Stack, pub PelicanPage, #[skip] Box<dyn SuccessGetter>, #[skip] Theme, #[skip] bool);
impl OnEvent for SuccessPage {}
impl AppPage for SuccessPage {}
impl SuccessPage {
    pub fn new(theme: &Theme, title: String, getter: Box<dyn SuccessGetter>, flow_len: usize) -> Self {
        let header = Header::stack_end(theme, &title);
        let bumper = Some(PelicanBumper::stack_end(theme, Some(flow_len)));
        let page = PelicanPage::new(header, Content::new(Offset::Center, vec![], Box::new(|_, _| true)), bumper);
        SuccessPage(Stack::default(), page, getter, theme.clone(), false)
    }

    pub fn on_change(&mut self, new: Vec<State>) {
        if !self.4 {
            self.4 = true;
            use pelican_ui::colors;
            use pelican_ui::components::Icon;
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
    #[skip] room: Instance<ChatRoom>,
    #[skip] profiles: Vec<Profile>,
    #[skip] messages: Vec<pelican_ui::components::Message>,
    #[skip] is_group: bool,
    #[skip] theme: Theme,
    #[skip] flow_len: usize,
}

impl OnEvent for MessagesPage {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if event.downcast_ref::<TickEvent>().is_some() {
            let room = self.room.pending();
            let members = room.members.clone();
            let my_name = ctx.me();
            let mut profiles = members.into_iter().filter(|n| *n != my_name).map(|n| Profile::from_name(ctx, n)).collect::<Vec<Instance<Profile>>>();
            let deref_profiles = profiles.iter_mut().map(|p| p.pending().clone()).collect::<Vec<Profile>>();
            
            if deref_profiles != *self.profiles {
                self.profiles = deref_profiles.clone();
                self.is_group = self.profiles.len() > 1;
                let p = deref_profiles.iter().map(|p| p.to_pel()).collect::<Vec<_>>();

                let info = match (self.is_group, profiles.first().cloned()) {
                    (false, Some(profile)) => {
                        Box::new(move |ctx: &mut Context, theme: &Theme| {
                            let profile = profile.clone();
                            (Flow::new(theme, vec![
                                Box::new(move || PageType::profile(profile.clone()))
                            ]).build(ctx))(ctx, theme);
                        }) as Box<dyn Callback>
                    }
                    _ => Box::new(move |ctx: &mut Context, theme: &Theme| {
                        let profiles = profiles.clone();
                        let t = theme.clone();
                        (Flow::new(theme, vec![
                            Box::new(move || GroupMessageInfoPage::new(&t.clone(), profiles.clone()))
                        ]).build(ctx))(ctx, theme);
                    }) as Box<dyn Callback>,
                };


                self.page.header = Header::messaging(ctx, &self.theme, p.clone(), self.flow_len, info);
            }

            let messages = room.messages.iter().map(|message: &Message| message.to_pel(ctx)).collect::<Vec<_>>();

            if messages != self.messages {
                println!("Updating mesages");
                self.messages = messages.clone();
                self.page.content = Content::new(Offset::End, drawables![MessageGroups::new(ctx, &self.theme, messages, self.is_group, false)], Box::new(|_, _| true));
            }
        }

        vec![event]
    }
}

impl AppPage for MessagesPage {}
impl MessagesPage {
    pub fn new(ctx: &mut Context, theme: &Theme, room: Instance<ChatRoom>, flow_len: usize) -> Self {
        let info = Box::new(|ctx: &mut Context, theme: &Theme|{});
        let header = Header::messaging(ctx, theme, vec![], flow_len, info);
        let mut room_taken = room.clone();
        let bumper = Some(PelicanBumper::input(theme, "Message...", move |ctx: &mut Context, val: &mut String| {
            if !val.is_empty() { room_taken.apply(SendMessage(val.to_string())); }
        }));

        let messages = MessageGroups::new(ctx, theme, vec![], false, false);
        let page = PelicanPage::new(header, Content::new(Offset::End, drawables![messages], Box::new(|_, _| true)), bumper);

        MessagesPage {layout: Stack::default(), page, room, messages: vec![], is_group: false, theme: theme.clone(), profiles: vec![], flow_len}
    }
}


// THIS DOES NOT NEED TO BE A PAGE
pub struct GroupMessageInfoPage;
impl GroupMessageInfoPage {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(theme: &Theme, profiles: Vec<Instance<Profile>>) -> PageType {
        // let header = Header::stack(theme, "Group info", None);
        // let profiles = ListItemGroup::new(theme, None, profiles.into_iter().map(|(p, id)| ListItem::new(theme, Some(p.avatar.clone()),
        //     ListItemInfoLeft::new(&p.username, Some(&p.name.unwrap().to_string()), None, None), 
        //     None, None, Some(Icons::Forward), Box::new(move |ctx: &mut Context, theme: &Theme| {
        //         let page: Box<dyn AppPage> = Box::new(ProfilePage::new(ctx, theme, p.clone(), id));
        //         let flow = FlowWrapper::new(PelicanFlow::new(vec![page]));
        //         ctx.emit(NavigationEvent::push(flow));
        //     })
        // )).collect());

        // let page = PelicanPage::new(header, Content::new(Offset::Start, drawables![profiles], Box::new(|_, _| true)), None);
        // GroupMessageInfoPage(Stack::default(), page)
        let theme = theme.clone();
        PageType::display("Group info", vec![
            Display::list(None, Arc::new(Box::new(move |ctx: &mut Context| {
                profiles.clone().into_iter().flat_map(|mut profile| {
                    let p = profile.clone();
                    let deref = profile.pending();
                    if deref.name.unwrap() != ctx.me() {
                        let view_contact = Flow::new(&theme, vec![Box::new(move || PageType::profile(p.clone()))]);
                        Some(crate::ListItem::avatar(deref.avatar.clone(), &deref.username, &deref.name(), None, Some(view_contact)))
                    } else {None}
                }).collect::<Vec<crate::ListItem>>()
            })), None),
        ], None, Bumper::None, Offset::Start)
    }
}

pub struct ProfilePage;
impl ProfilePage {
    pub fn new(ctx: &mut Context, theme: &Theme, mut profile: Instance<Profile>) -> Box<dyn AppPage> {
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
            None
        }) as Box<dyn FormSubmit>;
        
        let mut avatar = profile.clone();
        let mut username = profile.clone();
        let mut notes = profile.clone();

        let p = profile.pending();
        let title = if p.name.unwrap() == ctx.me() {"My profile"} else {"View contact"};
        let page = PageType::edit_and_display(
            title,
            vec![
                FormItem::avatar_with_preset("Avatar", p.avatar.clone(), move |ctx: &mut Context, a: String| {
                    let current = avatar.pending().avatar.get_image().unwrap_or_default();
                    match current == a {
                        true => Ok(String::new()),
                        false => Err(String::new())
                    }
                }),
                FormItem::text_with_preset("Username", &p.username.clone(), None, move |ctx: &mut Context, a: String| {
                    match a.is_empty() {
                        true => Err("Username cannot be empty".to_string()),
                        false => {
                            match username.pending().username == a {
                                true => Ok(String::new()),
                                false => Err(String::new())
                            }
                        }
                    }
                }),
                FormItem::text_with_preset("About me", &p.notes, None, move |ctx: &mut Context, a: String| {
                    match notes.pending().notes == a {
                        true => Ok(String::new()),
                        false => Err(String::new())
                    }
                }),
            ],
            vec![
                Display::cta("Orange name", None, &p.name.unwrap().to_string(), vec![("Copy".to_string(), Icons::Copy, Action::copy(&p.name.unwrap().to_string()))]),
            ],
            closure
        );

        match p.name.unwrap() == ctx.me() {
            true => page.build_root(ctx, theme),
            false => page.build(ctx, theme)
        }
    }
}

