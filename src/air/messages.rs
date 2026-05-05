use pelican_ui::Context;
use pelican_ui::utils::Timestamp;

use air::names::{Id, Name};
use air::contract::{Contract, Substance, Reactants, Reactant, Beaker};

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::profiles::{Contact, Profile};

#[derive(Serialize, Deserialize, Hash)]
pub struct ChatRoom(pub Id);
impl ChatRoom { pub fn new() -> Self {ChatRoom(Id::random())} }
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
pub struct SendMessage(pub String);
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
            author: Profile::from_name(ctx, self.author).0.to_pel(),
        }
    }
}
