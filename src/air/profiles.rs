use pelican_ui::Context;
use pelican_ui::components::avatar::AvatarContent;

use air::names::{Id, Name};
use air::contract::{Contract, Substance, Reactants, Reactant, Beaker};

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use rand::{seq::SliceRandom, Rng};

use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub name: Option<Name>,
    pub username: String,
    pub notes: String,
    pub avatar: AvatarContent,
}

impl Profile {
    pub fn create(ctx: &mut Context, name: Name) -> (Profile, Id) {
        println!("creating new profile");
        let profile = Profile {
            name: Some(name),
            username: Username::new(),
            notes: String::new(),
            avatar: AvatarContent::default(),
        };

        let id = ctx.create(Contact::new(name, profile.username.to_string(), profile.notes.to_string())).unwrap();
        (profile, id)
    }

    pub fn me(ctx: &mut Context) -> (Self, Id) {Profile::from_name(ctx, ctx.me())}

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

    pub fn from_name(ctx: &mut Context, name: Name) -> (Profile, Id) {
        ctx.list::<Contact>().iter().find_map(|contact| {
            if Some(Substance::String(name.to_string())) == ctx.get::<Contact, _>(contact, "/name") {
                Some((Profile::from_id(ctx, *contact), *contact))
            } else {None}
        }).unwrap_or(Profile::create(ctx, name))
    }

    pub fn from_substance(ctx: &mut Context, substance: &Substance) -> Option<(Self, Id)> {
        if let Substance::String(first) = substance {
            if let Ok(first) = Name::from_str(first) {
                Some(Profile::from_name(ctx, first))
            } else {None}
        } else {None}
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
