use pelican_ui::Context;
use pelican_ui::utils::Timestamp;

use air::names::{Id, Name};
use air::{Contract, Reactants, Reactant};
use air::Instance;

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::profiles::Profile;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct ChatRoom {
    pub members: Vec<Name>,
    pub messages: Vec<Message>,
}

impl Contract for ChatRoom {
    type Init = ();

    fn id() -> Id {Id::hash("ChatRoom2.9")}

    fn init(init: Self::Init, signer: Name, _timestamp: u64) -> Self {
        ChatRoom {
            members: vec![signer],
            messages: Vec::new(),
        }
    }

    fn reactants() -> Reactants<ChatRoom> {
        Reactants::default().add::<SendMessage>().add::<AddMember>()
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct MemberExists(Id);
impl std::error::Error for MemberExists {}
impl std::fmt::Display for MemberExists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "{:?}", self)}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddMember(pub Name);
impl Reactant<ChatRoom> for AddMember {
    type Result = Result<(), MemberExists>;

    fn id() -> Id {Id::hash("AddMember")}

    fn apply(self, room: &mut ChatRoom, signer: Name, timestamp: u64) -> Self::Result {
        // if room.members.any(|m| m.id == self.0) {Err(MemberExists(self.0))?}
        room.members.push(self.0);
        Ok(())
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct MessageExists(Id);
impl std::error::Error for MessageExists {}
impl std::fmt::Display for MessageExists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "{:?}", self)}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SendMessage(pub String);
impl Reactant<ChatRoom> for SendMessage {
    type Result = Result<(), MessageExists>;

    fn id() -> Id {Id::hash("SendMessage")}

    fn apply(self, room: &mut ChatRoom, signer: Name, timestamp: u64) -> Self::Result {
        // if room.messages.find_map(|m| m.id == self.0).is_some() { Err(MessageExists(self.0))?}
        room.messages.push(Message{author: signer, timestamp, body: self.0});
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq)]
pub struct Message {
    pub author: Name,
    pub body: String,
    pub timestamp: u64,
}

impl Message {
    // pub fn from_id(ctx: &mut Context, id: Id) -> Self {
    //     if let Ok(pending) = ctx.air().get_pending(&id) {
    //         let author = if let Some(Substance::String(name)) = pending.get("/author") { Name::from_str(&name).unwrap() } else {todo!()};
    //         let body = if let Some(Substance::String(body)) = pending.get("/body") { body.to_string() } else {todo!()};
    //         let timestamp = if let Some(Substance::Integer(timestamp)) = pending.get("/timestamp") { *timestamp } else {todo!()};
    //         Message { author, body, timestamp }
    //     } else {todo!()}
    // }

    // pub fn from_substance(substance: Substance) -> Self {
    //     let author = if let Ok(Substance::String(name)) = substance.query("/author") { Name::from_str(&name).unwrap() } else {todo!()};
    //     let body = if let Ok(Substance::String(body)) = substance.query("/body") { body } else {todo!()};
    //     let timestamp = if let Ok(Substance::Integer(timestamp)) = substance.query("/timestamp") { timestamp } else {todo!()};
        
    //     Message {author, body, timestamp}
    // }

    pub fn to_pel(&self, ctx: &mut Context) -> pelican_ui::components::Message {
        pelican_ui::components::Message {
            message: self.body.to_string(),
            timestamp: Timestamp::from_u64(self.timestamp),
            author: Profile::from_name(ctx, self.author).pending().to_pel(),
        }
    }
}
