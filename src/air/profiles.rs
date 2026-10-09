use pelican_ui::Context;
use pelican_ui::components::avatar::AvatarContent;

use maverick_os::air::{Metadata, Contract, Id, Name, Instance};

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use rand::{seq::SliceRandom, Rng};

use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Profile {
    pub name: Option<Name>,
    pub username: String,
    pub notes: String,
    pub avatar: AvatarContent,
    init: bool,
}

impl Contract for Profile {
    type Init = Name;
    type Message = ProfileAction;
    type Result = ();

    fn id() -> Id {Id::hash("Profile")}

    fn init(init: Self::Init, metadata: Metadata) -> Self {
        Profile {
            name: Some(init),
            username: "Orange Profile".to_string(),
            notes: String::new(),
            avatar: AvatarContent::default(),
            init: false,
        }
    }

    fn apply(&mut self, message: Self::Message, metadata: Metadata) -> Self::Result {
        match message {
            ProfileAction::ChangeUsername(content) => self.username = content.to_string(),
            ProfileAction::ChangeNotes(content) => self.notes = content.to_string(),
            ProfileAction::ChangeAvatar(content) => self.avatar = content.clone(),
            ProfileAction::SetUsernameInit(content) => {
                if !self.init {
                    self.init = true;
                    self.username = content.to_string();
                }
            }
        }

        ()
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum ProfileAction {
    ChangeUsername(String),
    ChangeNotes(String),
    ChangeAvatar(AvatarContent),
    SetUsernameInit(String)
}

impl Profile {
    pub fn create(ctx: &mut Context, name: Name) -> Instance<Profile> {
        // ctx.register::<Profile>();
        // std::thread::sleep(std::time::Duration::from_secs(1));
        let mut profile = ctx.create::<Profile>(name);
        profile.send(ProfileAction::SetUsernameInit(Username::new()));
        profile
    }

    pub fn me(ctx: &mut Context) -> Instance<Profile> {
        let my_name = ctx.me();
        Profile::from_name(ctx, my_name)
    }

    pub fn from_name(ctx: &mut Context, name: Name) -> Instance<Profile> {
        Profile::try_from_name(ctx, name).unwrap_or_else(|| Profile::create(ctx, name))
    }

    pub fn try_from_name(ctx: &mut Context, name: Name) -> Option<Instance<Profile>> {
        ctx.list::<Profile>().iter_mut().find_map(|profile| {
            if profile.1.pending().name == Some(name) {
                Some(profile.1.clone())
            } else {
                None
            }
        })
    }

    pub fn to_pel(&self) -> pelican_ui::components::Profile {
        pelican_ui::components::Profile {
            name: self.name.unwrap(),
            username: self.username.clone(),
            pfp: self.avatar.clone(), // TODO
        }
    }

    pub fn name(&self) -> String {
        let c: Vec<_> = self.name.unwrap().to_string().chars().collect();
        format!("{}...{}", c[..17].iter().collect::<String>(), c[c.len()-4..].iter().collect::<String>())
    }
}

pub struct Username;
impl Username {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> String {
        let parse = |s: &str| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(|l| l.to_string()).collect::<Vec<_>>();
        let mut rng = rand::thread_rng();

        let animals = parse(ANIMALS);
        let adjectives = parse(ADJECTIVES);
        let foods = parse(FOODS);
        let noun = if rng.gen_bool(0.5) { &animals } else { &foods };

        let f = |s: &str| {
            let mut o = String::with_capacity(s.len());
            let mut cap = true;
            for c in s.chars() {
                if c == ' ' || c == '-' { cap = true; continue; }
                if cap { for u in c.to_uppercase() { o.push(u); } cap = false; }
                else { o.push(c); }
            }
            o
        };

        format!("{}{}", f(adjectives.choose(&mut rng).unwrap()), f(noun.choose(&mut rng).unwrap()))
    }
}

static ANIMALS: &str = include_str!("../../usernames/animals.txt");
static FOODS: &str = include_str!("../../usernames/foods.txt");
static ADJECTIVES: &str = include_str!("../../usernames/adjectives.txt");
