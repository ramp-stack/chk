use pelican_ui::Context;
use pelican_ui::components::avatar::AvatarContent;

use air::Instance;
use air::names::{Id, Name};
use air::{Contract, Reactants, Reactant};

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use rand::{seq::SliceRandom, Rng};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChangeUsername(pub String);
impl Reactant<Profile> for ChangeUsername {
    type Result = ();

    fn id() -> Id {Id::hash("ChangeUsername")}

    fn apply(self, profile: &mut Profile, signer: Name, timestamp: u64) -> Self::Result {
        profile.username = self.0.to_string();
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChangeNotes(pub String);
impl Reactant<Profile> for ChangeNotes {
    type Result = ();

    fn id() -> Id {Id::hash("ChangeNotes")}

    fn apply(self, profile: &mut Profile, signer: Name, timestamp: u64) -> Self::Result {
        profile.notes = self.0.to_string();
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChangeAvatar(pub AvatarContent);
impl Reactant<Profile> for ChangeAvatar {
    type Result = ();

    fn id() -> Id {Id::hash("ChangeAvatar")}

    fn apply(self, profile: &mut Profile, signer: Name, timestamp: u64) -> Self::Result {
        profile.avatar = self.0.clone();
    }
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub name: Option<Name>,
    pub username: String,
    pub notes: String,
    pub avatar: AvatarContent,
    pub id: Id,
}

impl Contract for Profile {
    type Init = Name;

    fn id() -> Id {Id::hash("Profile0.1")}

    fn init(init: Self::Init, signer: Name, _timestamp: u64) -> Self {
        Profile {
            name: Some(init),
            username: Username::new(),
            notes: String::new(),
            avatar: AvatarContent::default(),
            id: Id::random(),
        }
    }

    fn reactants() -> Reactants<Profile> {
        Reactants::default().add::<ChangeUsername>().add::<ChangeNotes>().add::<ChangeAvatar>()
    }
}

impl Profile {
    pub fn create(ctx: &mut Context, name: Name) -> Instance<Profile> {
        // ctx.register::<Profile>();
        // std::thread::sleep(std::time::Duration::from_secs(1));
        ctx.create::<Profile>(name)
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
            if profile.pending().name == Some(name) {
                Some(profile.clone())
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
