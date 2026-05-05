use ramp::prism;

use pelican_ui::event::{OnEvent, Event, TickEvent};
use pelican_ui::drawable::{Component, Drawable, SizedTree};
use pelican_ui::{Context, Callback, drawables};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::canvas::Align;
use pelican_ui::components::avatar::AvatarContent;
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::components::text::{ExpandableText, TextSize, TextStyle};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::components::MessageGroups;

use crate::messages::{ChatRoom, Message, SendMessage};
use crate::profiles::{Profile, ChangeNotes, ChangeUsername};
use crate::{PageType, FormItem, Bumper};
use crate::flow::{Flow, State};
use crate::items::{Action, Display};
use crate::closure::{FormSubmit, NavFn, ReviewItemGetter, SuccessGetter};

use air::names::{Id, Name};
use air::contract::{Substance, Beaker};

use std::str::FromStr;
use std::sync::Arc;

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
    #[skip] room_id: Id,
    #[skip] profiles: Vec<(Profile, Id)>,
    #[skip] messages: Vec<pelican_ui::components::Message>,
    #[skip] is_group: bool,
    #[skip] theme: Theme,
    #[skip] flow_len: usize,
}
impl OnEvent for MessagesPage {
    fn on_event(&mut self, ctx: &mut Context, sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        // move this code to the MessageGroups
        let messages = if let Some(Substance::Seq(substances)) = ctx.get::<ChatRoom, _>(&self.room_id, "/messages") {
            substances.iter().flat_map(|substance| {
                let author = if let Ok(Substance::String(name)) = substance.query("/author") { Name::from_str(&name).unwrap() } else {todo!()};
                let body = if let Ok(Substance::String(body)) = substance.query("/body") { body } else {todo!()};
                let timestamp = if let Ok(Substance::Integer(timestamp)) = substance.query("/timestamp") { timestamp } else {todo!()};
                
                Some(Message {author, body, timestamp}.to_pel(ctx))
            }).collect::<Vec<_>>()
        } else {vec![]};

        if messages != self.messages {
            self.messages = messages.clone();
            self.page.content = Content::new(Offset::End, drawables![MessageGroups::new(ctx, &self.theme, messages, self.is_group, false)], Box::new(|_, _| true));
        }

        let profiles = if let Some(Substance::Seq(substances)) = ctx.get::<ChatRoom, _>(&self.room_id, "/members") {
            substances.iter().filter_map(|name| {
                if let Substance::String(n) = name {
                    let name = Name::from_str(&n).unwrap();
                    if name == ctx.me() {None} else {
                        Some(Profile::from_name(ctx, name))
                    }
                } else {None}
            }).collect::<Vec<(Profile, Id)>>()
        } else {vec![]};

        let saved_ids = self.profiles.iter().map(|(_, id)| *id).collect::<Vec<Id>>();
        let old_ids = profiles.iter().map(|(_, id)| *id).collect::<Vec<Id>>();

        if saved_ids != old_ids {
            self.is_group = self.profiles.len() > 1;
            self.profiles = profiles.clone();

            let info = match (self.is_group, profiles.get(0).cloned()) {
                (false, Some((profile, id))) => {
                    Box::new(move |ctx: &mut Context, theme: &Theme| {
                        let profile = profile.clone();
                        let id = id.clone();
                        (Flow::new(&theme, vec![
                            Box::new(move || PageType::profile(profile.clone(), id.clone()))
                        ]).build(ctx))(ctx, theme);
                    }) as Box<dyn Callback>
                }
                _ => Box::new(move |ctx: &mut Context, theme: &Theme| {
                    let profiles = profiles.clone();
                    let t = theme.clone();
                    (Flow::new(&theme, vec![
                        Box::new(move || GroupMessageInfoPage::new(&t.clone(), profiles.clone()))
                    ]).build(ctx))(ctx, theme);
                }) as Box<dyn Callback>,
            };


            let p = self.profiles.iter().map(|p| p.0.to_pel()).collect::<Vec<_>>();
            self.page.header = Header::messaging(ctx, &self.theme, p.clone(), self.flow_len, info);
        }

        vec![event]
    }
}
impl AppPage for MessagesPage {}
impl MessagesPage {
    pub fn new(ctx: &mut Context, theme: &Theme, room_id: Id, flow_len: usize) -> Self {
        let profiles = if let Some(Substance::Seq(substances)) = ctx.get::<ChatRoom, _>(&room_id, "/members") {
            substances.iter().filter_map(|name| {
                if let Substance::String(n) = name {
                    let name = Name::from_str(&n).unwrap();
                    if name == ctx.me() {None} else {
                        Some(Profile::from_name(ctx, name))
                    }
                } else {None}
            }).collect::<Vec<(Profile, Id)>>()
        } else {vec![]};


        let is_group = profiles.len() > 1;
        let all = profiles.clone();

        let info = match (is_group, profiles.get(0).cloned()) {
            (false, Some((profile, id))) => Box::new(move |ctx: &mut Context, theme: &Theme| {
                let profile = profile.clone();
                let id = id.clone();
                (Flow::new(&theme, vec![
                    Box::new(move || PageType::profile(profile.clone(), id.clone()))
                ]).build(ctx))(ctx, theme);
            }) as Box<dyn Callback>,
            _ => Box::new(move |ctx: &mut Context, theme: &Theme| {
                let profiles = all.clone();
                let t = theme.clone();
                (Flow::new(&theme, vec![
                    Box::new(move || GroupMessageInfoPage::new(&t, profiles.clone()))
                ]).build(ctx))(ctx, theme);
            }) as Box<dyn Callback>,
        };


        let p = profiles.iter().map(|p| p.0.to_pel()).collect::<Vec<_>>();
        let header = Header::messaging(ctx, theme, p.clone(), flow_len, info);
        let bumper = Some(PelicanBumper::input(theme, "Message...", move |ctx: &mut Context, val: &mut String| {
            if !val.is_empty() {
                let _ = ctx.send(room_id, "/messages", SendMessage(val.to_string()));
            }
        }));

        // let messages = messages.iter().map(|p| Message::from_id(ctx, p).to_pel()).collect::<Vec<_>>();
        let messages = MessageGroups::new(ctx, theme, vec![], is_group, false);
        let page = PelicanPage::new(header, Content::new(Offset::End, drawables![messages], Box::new(|_, _| true)), bumper);

        MessagesPage {layout: Stack::default(), page, room_id, messages: vec![], is_group, theme: theme.clone(), profiles, flow_len}
    }
}


// THIS DOES NOT NEED TO BE A PAGE
pub struct GroupMessageInfoPage;
impl GroupMessageInfoPage {
    pub fn new(theme: &Theme, profiles: Vec<(Profile, Id)>) -> PageType {
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
                let profiles = profiles.clone();
                profiles.into_iter().filter(|(p, _)| p.name.unwrap() != ctx.me()).map(|(p, id)| {
                    let profile = p.clone();
                    let view_contact = Flow::new(&theme, vec![Box::new(move || PageType::profile(profile.clone(), id.clone()))]);
                    crate::ListItem::avatar(p.avatar.clone(), &p.username, &p.name(), None, Some(view_contact))
                }).collect::<Vec<crate::ListItem>>()
            })), None),
        ], None, Bumper::None, Offset::Start)
    }
}

#[derive(Debug, Component, Clone)]
pub struct ProfilePage(Stack, Box<dyn AppPage>);
impl OnEvent for ProfilePage {}
impl AppPage for ProfilePage {}
impl ProfilePage {
    pub fn new(ctx: &mut Context, theme: &Theme, profile: Profile, contact_id: Id) -> Self {
        let closure = Box::new(move |ctx: &mut Context, objects: &Vec<State>| {
            if let Some(State::Text(result)) = objects.get(1) {
                let _ = ctx.send(contact_id, "/username", ChangeUsername(result.to_string()));
            }
            if let Some(State::Text(result)) = objects.get(2) {
                let _ = ctx.send(contact_id, "/notes", ChangeNotes(result.to_string()));
            }
            None
        }) as Box<dyn FormSubmit>;
        let username_name = profile.name.unwrap().clone();
        let notes_name = username_name.clone();

        let title = if notes_name == ctx.me() {"My profile"} else {"View contact"};
        let page = PageType::edit_and_display(
            title,
            vec![
                FormItem::avatar_with_preset("Avatar", profile.avatar),
                FormItem::text_with_preset("Username", &profile.username.clone(), None, move |ctx: &mut Context, a: String| {
                    match a.is_empty() {
                        true => Err("Username cannot be empty".to_string()),
                        false => {
                            let (current, _) = Profile::from_name(ctx, username_name.clone());
                            match current.username == a {
                                true => Ok(String::new()),
                                false => Err(String::new())
                            }
                        }
                    }
                }),
                FormItem::text_with_preset("About me", &profile.notes, None, move |ctx: &mut Context, a: String| {
                    let (current, _) = Profile::from_name(ctx, notes_name.clone());
                    match current.notes == a {
                        true => Ok(String::new()),
                        false => Err(String::new())
                    }
                }),
            ],
            vec![
                Display::cta("Orange name", None, &profile.name.unwrap().to_string(), vec![("Copy".to_string(), Icons::Copy, Action::copy(&profile.name.unwrap().to_string()))]),
            ],
            closure
        );

        let page = match profile.name.unwrap() == ctx.me() {
            true => page.build_root(ctx, theme),
            false => page.build(ctx, theme)
        };
        ProfilePage(Stack::default(), page)
    }
}

