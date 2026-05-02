use ramp::prism;
use pelican_ui::event::{OnEvent, Event, TickEvent};
use pelican_ui::drawable::{Component, Drawable, SizedTree};
use pelican_ui::{Request, drawables, Context, Callback};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::canvas::Align;
use pelican_ui::components::TextInput;
use pelican_ui::components::avatar::{Avatar, AvatarSize, AvatarContent, AvatarIconStyle};
use pelican_ui::components::list_item::{ListItemGroup, ListItem, ListItemInfoLeft};
use pelican_ui::navigation::NavigationEvent;
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::{AppPage, Flow as PelicanFlow};
use pelican_ui::components::text::{ExpandableText, TextSize, TextStyle};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::components::MessageGroups;
use std::fmt::Debug;
use pelican_ui::utils::{ValidationFn, Timestamp};

use crate::FlowWrapper;
use crate::flow::{Flow, State};
use crate::items::{Action, Input, Display};
use crate::closure::{FormSubmit, NavFn, ScreenBuilder, PageBuilder, ReviewItemGetter, SuccessGetter};

use air::names::{Secret, Id, Name};
use air::contract::{Contracts, Contract, Substance, Reactants, Reactant, Beaker};

use std::collections::BTreeMap;
use std::path::{PathBuf, Path};
use std::convert::Infallible;
use std::str::FromStr;

use std::sync::Arc;
use rand::{seq::SliceRandom, Rng};
use std::fs;

use serde::{Serialize, Deserialize};

#[derive(Debug, Component, Clone)]
pub struct MessagesPage(Stack, PelicanPage, #[skip] Id, #[skip] Vec<pelican_ui::components::Message>, #[skip] bool, #[skip] Theme);
impl OnEvent for MessagesPage {
    fn on_event(&mut self, ctx: &mut Context, sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        let messages = if let Some(Substance::Seq(substances)) = ctx.get::<ChatRoom, _>(&self.2, "/messages") {
            substances.iter().flat_map(|substance| {
                let author = if let Ok(Substance::String(name)) = substance.query("/author") { Name::from_str(&name).unwrap() } else {todo!()};
                let body = if let Ok(Substance::String(body)) = substance.query("/body") { body } else {todo!()};
                let timestamp = if let Ok(Substance::Integer(timestamp)) = substance.query("/timestamp") { timestamp } else {todo!()};
                
                Some(Message {author, body, timestamp}.to_pel(ctx))
            }).collect::<Vec<_>>()
        } else {vec![]};

        if messages != self.3 {
            self.3 = messages.clone();
            self.1.content = Content::new(Offset::End, drawables![MessageGroups::new(ctx, &self.5, messages, self.4, false)], Box::new(|_, _| true));
        }

        vec![event]
    }
}
impl AppPage for MessagesPage {}
impl MessagesPage {
    pub fn new(ctx: &mut Context, theme: &Theme, room_id: Id, flow_len: usize) -> Self {
        let profiles = if let Some(Substance::Seq(substances)) = ctx.get::<ChatRoom, _>(&room_id, "/members") {
            substances.iter().flat_map(|substance| {
                if let Substance::String(name) = substance {
                    Name::from_str(name).ok().map(|n| Profile::from_name(ctx, n))
                } else {None}
            }).flatten().collect::<Vec<_>>()
        } else {vec![]};

        let is_group = profiles.len() > 2;

        let info: Box<dyn AppPage> = match is_group {
            false => match profiles.get(0) { 
                Some(first) => Box::new(ProfilePage::new(theme, first.clone())),
                None => Box::new(GroupMessageInfoPage::new(theme, profiles.clone())),
            },
            true => Box::new(GroupMessageInfoPage::new(theme, profiles.clone())),
        };

        let profiles = profiles.iter().map(|p| p.to_pel()).collect::<Vec<_>>();
        let header = Header::messaging(ctx, theme, profiles.clone(), flow_len, Box::new(FlowWrapper::new(PelicanFlow::new(vec![info]))));

        let bumper = Some(PelicanBumper::input(theme, "Message...", move |ctx: &mut Context, val: &mut String| {
            let _ = ctx.send(room_id, "/messages", SendMessage(val.to_string()));
        }));

        // let messages = messages.iter().map(|p| Message::from_id(ctx, p).to_pel()).collect::<Vec<_>>();
        let messages = MessageGroups::new(ctx, theme, vec![], is_group, false);
        let page = PelicanPage::new(header, Content::new(Offset::End, drawables![messages], Box::new(|_, _| true)), bumper);

        MessagesPage(Stack::default(), page, room_id, vec![], is_group, theme.clone())
    }
}


// THIS DOES NOT NEED TO BE A PAGE
#[derive(Debug, Component, Clone)]
pub struct GroupMessageInfoPage(Stack, PelicanPage);
impl OnEvent for GroupMessageInfoPage {}
impl AppPage for GroupMessageInfoPage {}
impl GroupMessageInfoPage {
    pub fn new(theme: &Theme, profiles: Vec<Profile>) -> Self {
        let header = Header::stack(theme, "Group info", None);
        let profiles = ListItemGroup::new(theme, None, profiles.into_iter().map(|p| ListItem::new(theme, Some(p.avatar.clone()),
            ListItemInfoLeft::new(&p.username, Some(&p.name.unwrap().to_string()), None, None), 
            None, None, Some(Icons::Forward), Box::new(move |ctx: &mut Context, theme: &Theme| {
                let page: Box<dyn AppPage> = Box::new(ProfilePage::new(theme, p.clone()));
                let flow = FlowWrapper::new(PelicanFlow::new(vec![page]));
                ctx.emit(NavigationEvent::push(flow));
            })
        )).collect());

        let page = PelicanPage::new(header, Content::new(Offset::Start, drawables![profiles], Box::new(|_, _| true)), None);
        GroupMessageInfoPage(Stack::default(), page)
    }
}

#[derive(Debug, Component, Clone)]
pub struct ProfilePage(Stack, PelicanPage);
impl OnEvent for ProfilePage {}
impl AppPage for ProfilePage {}
impl ProfilePage {
    pub fn new(theme: &Theme, profile: Profile) -> Self {
        let header = Header::stack(theme, &profile.username, None);

        let page = PelicanPage::new(
            header, 
            Content::new(Offset::Start, drawables![
                Avatar::new(theme, profile.avatar, None, false, AvatarSize::Xxl, None),
                TextInput::default(theme),
                TextInput::default(theme)
            ], Box::new(|_, _| true)), 
            None
        );

        ProfilePage(Stack::default(), page)
    }
}


#[derive(Serialize, Deserialize, Hash)]
pub struct ChatRoom(Id);
impl ChatRoom {
    pub fn new() -> Self {ChatRoom(Id::random())}
}
impl Contract for ChatRoom {
    fn id() -> Id {Id::hash("ChatRoom2.7")}

    fn init(self, signer: &Name, _timestamp: u64) -> Substance {Substance::Map(BTreeMap::from([
        ("name".to_string(), Substance::String("myroom".to_string())),
        ("members".to_string(), Substance::Seq(vec![])),
        ("author".to_string(), Substance::String(signer.to_string())),
        ("messages".to_string(), Substance::Seq(vec![]))
    ]))}

    fn routes() -> BTreeMap<PathBuf, Reactants> {
        BTreeMap::from([
            (PathBuf::from("/messages"), Reactants::new().add::<SendMessage>()),
            (PathBuf::from("/members"), Reactants::new().add::<AddMember>()),
        ])
    }
}


#[derive(Serialize, Deserialize, Hash)]
pub struct Contact(Id, Name, String, String);
impl Contact {
    pub fn new(name: Name, username: String, notes: String) -> Self {Contact(Id::random(), name, username, notes)}
}
impl Contract for Contact {
    fn id() -> Id {Id::hash("Contact0.0")}

    fn init(self, signer: &Name, _timestamp: u64) -> Substance {Substance::Map(BTreeMap::from([
        ("username".to_string(), Substance::String(self.2.to_string())),
        ("notes".to_string(), Substance::String(self.3.to_string())),
        ("avatar".to_string(), Substance::String(String::new())),
        ("name".to_string(), Substance::String(self.1.to_string())),
        ("author".to_string(), Substance::String(signer.to_string())),
    ]))}

    fn routes() -> BTreeMap<PathBuf, Reactants> {
        BTreeMap::from([
            (PathBuf::from("/username"), Reactants::new().add::<ChangeUsername>()),
            (PathBuf::from("/notes"), Reactants::new().add::<ChangeNotes>()),
            (PathBuf::from("/avatar"), Reactants::new().add::<ChangeAvatar>()),
        ])
    }
}

#[derive(Serialize, Deserialize, Hash)]
pub struct ChangeUsername(pub String);
impl Reactant for ChangeUsername {
    type Error = Infallible;
    type Contract = Contact;

    fn apply<B: Beaker>(self, _path: &Path, signer: &Name, _timestamp: u64, substance: &mut B) -> Result<(), Self::Error> {
        if substance.query("/author") == Ok(Substance::String(signer.to_string())) {
            let _ = substance.insert("username", Substance::String(self.0));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Hash)]
pub struct ChangeNotes(pub String);
impl Reactant for ChangeNotes {
    type Error = Infallible;
    type Contract = Contact;

    fn apply<B: Beaker>(self, _path: &Path, signer: &Name, _timestamp: u64, substance: &mut B) -> Result<(), Self::Error> {
        if substance.query("/author") == Ok(Substance::String(signer.to_string())) {
            let _ = substance.insert("notes", Substance::String(self.0));
        }
        Ok(())
    }
}


#[derive(Serialize, Deserialize, Hash)]
pub struct ChangeAvatar(pub String);
impl Reactant for ChangeAvatar {
    type Error = Infallible;
    type Contract = Contact;

    fn apply<B: Beaker>(self, _path: &Path, signer: &Name, _timestamp: u64, substance: &mut B) -> Result<(), Self::Error> {
        if substance.query("/author") == Ok(Substance::String(signer.to_string())) {
            let _ = substance.insert("avatar", Substance::String(self.0));
        }
        Ok(())
    }
}


#[derive(Serialize, Deserialize, Hash)]
pub struct AddMember(pub Name);
impl Reactant for AddMember {
    type Error = Infallible;
    type Contract = ChatRoom;

    fn apply<B: Beaker>(self, _path: &Path, signer: &Name, _timestamp: u64, substance: &mut B) -> Result<(), Self::Error> {
        if substance.query("/author") == Ok(Substance::String(signer.to_string())) {
            let _ = substance.insert("/members/-", Substance::String(self.0.to_string()));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Hash)]
pub struct SendMessage(String);
impl Reactant for SendMessage {
    type Error = Infallible;
    type Contract = ChatRoom;

    fn apply<B: Beaker>(self, _path: &Path, signer: &Name, timestamp: u64, substance: &mut B) -> Result<(), Self::Error> {
        let _ = substance.insert("/messages/-", Substance::Map(BTreeMap::from([
            ("author".to_string(), Substance::String(signer.to_string())),
            ("timestamp".to_string(), Substance::Integer(timestamp as i64)),
            ("body".to_string(), Substance::String(self.0)),
        ])));
        Ok(())
    }
}


#[derive(Clone, Debug)]
pub struct Message {
    pub author: Name,
    pub body: String,
    pub timestamp: i64,
}

impl Message {
    pub fn from_id(ctx: &mut Context, id: Id) -> Self {
        let author = if let Some(Substance::String(name)) = ctx.get::<Contact, _>(&id, "/author") { Name::from_str(&name).unwrap() } else {todo!()};
        let body = if let Some(Substance::String(body)) = ctx.get::<Contact, _>(&id, "/body") { body } else {todo!()};
        let timestamp = if let Some(Substance::Integer(timestamp)) = ctx.get::<Contact, _>(&id, "/timestamp") { timestamp } else {todo!()};
        Message {
            author,
            body,
            timestamp,
        }
    }

    pub fn to_pel(&self, ctx: &mut Context) -> pelican_ui::components::Message {
        pelican_ui::components::Message {
            message: self.body.to_string(),
            timestamp: Timestamp::from_i64(self.timestamp),
            author: Profile::from_name(ctx, self.author).unwrap().to_pel(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub name: Option<Name>,
    pub username: String,
    pub notes: String,
    pub avatar: AvatarContent,
}

impl Profile {
    pub fn create(name: Name) -> Self {
        Profile {
            name: Some(name),
            username: Username::new(),
            notes: String::new(),
            avatar: AvatarContent::default(),
        }
    }

    pub fn me(ctx: &mut Context) -> (Self, Id) {
        let me = ctx.list::<Contact>().iter().find_map(|contact| {
            if Some(Substance::String(ctx.me().to_string())) == ctx.get::<Contact, _>(&contact, "/name") {
                Some((Profile::from_id(ctx, *contact), *contact))
            } else {None}
        });
        me.unwrap_or_else(|| {
            let profile = Profile::create(ctx.me());
            let id = ctx.create(Contact::new(ctx.me(), profile.username.to_string(), profile.notes.to_string())).unwrap();
            (profile, id)
        })
    }

    pub fn from_id(ctx: &mut Context, id: Id) -> Self {
        let name = if let Some(Substance::String(name)) = ctx.get::<Contact, _>(&id, "/name") { Name::from_str(&name).ok() } else {None};
        let username = if let Some(Substance::String(name)) = ctx.get::<Contact, _>(&id, "/username") { name } else { String::new() };
        let notes = if let Some(Substance::String(notes)) = ctx.get::<Contact, _>(&id, "/notes") { notes } else { String::new() };
        
        Profile {
            name,
            username,
            notes,
            avatar: AvatarContent::default(),
        }
    }

    pub fn from_name(ctx: &mut Context, name: Name) -> Option<Self> {
        ctx.list::<Contact>().iter().find_map(|contact| {
            if Some(Substance::String(name.to_string())) == ctx.get::<Contact, _>(&contact, "/name") {
                Some(Profile::from_id(ctx, *contact))
            } else {None}
        })
    }

    pub fn to_pel(&self) -> pelican_ui::components::Profile {
        pelican_ui::components::Profile {
            name: self.name.unwrap(),
            username: self.username.clone(),
            pfp: None, // TODO
        }
    }

    pub fn name(&self) -> String {
        let c: Vec<_> = self.name.unwrap().to_string().chars().collect();
        format!("{}...{}", c[..17].iter().collect::<String>(), c[c.len()-4..].iter().collect::<String>())
    }

    pub fn about_me(&self) -> String {
        match self.notes.is_empty() {
            true => "Nothing here yet.".to_string(),
            false => self.notes.to_string()
        }
    }
}

pub struct Username;
impl Username {
    pub fn new() -> String {
        let parse = |s: &str| -> Vec<String> { s.lines().map(|s| str::trim(s).to_string()).filter(|l| !l.is_empty()).collect::<Vec<String>>() };
        let animals = parse(ANIMALS);
        let adjectives = parse(ADJECTIVES);
        let foods = parse(FOODS);

        let mut rng = rand::thread_rng();

        let cap = |s: &str| {
            let s = s.to_lowercase();
            let mut c = s.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        };

        let noun_list = if rng.gen_bool(0.5) { animals } else { foods };

        format!(
            "{}{}",
            cap(adjectives.choose(&mut rng).unwrap()),
            cap(noun_list.choose(&mut rng).unwrap())
        )
    }
}

static ANIMALS: &str = include_str!("../../usernames/animals.txt");
static FOODS: &str = include_str!("../../usernames/foods.txt");
static ADJECTIVES: &str = include_str!("../../usernames/adjectives.txt");
