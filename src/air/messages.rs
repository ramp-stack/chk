use pelican_ui::Context;
use pelican_ui::utils::Timestamp;

use air::names::{Id, Name};
use air::{Metadata, Contract, Reactants, Reactant};
use air::Instance;

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::profiles::Profile;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ChatRoom {
    pub members: Vec<Name>,
    pub messages: Vec<Message>,
    pub name: (String, bool),
    pub id: Id,
}

impl Contract for ChatRoom {
    type Init = Id;

    fn id() -> Id {Id::hash("ChatRoom2.10")}

    fn init(init: Self::Init, metadata: Metadata) -> Self {
        ChatRoom {
            members: vec![metadata.signer],
            messages: Vec::new(),
            name: (String::new(), false), // name, manually changed?
            id: init,
        }
    }

    fn reactants() -> Reactants<ChatRoom> {
        Reactants::default().add::<SendMessage>().add::<AddMember>().add::<ChangeRoomName>()
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct MemberExists;
impl std::error::Error for MemberExists {}
impl std::fmt::Display for MemberExists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "{:?}", self)}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddMember(pub Name, pub String);
impl Reactant<ChatRoom> for AddMember {
    type Output = Result<(), MemberExists>;

    fn id() -> Id {Id::hash("AddMember")}

    fn apply(self, room: &mut ChatRoom, metadata: Metadata) -> Self::Output {
        if room.members.contains(&self.0) {return Err(MemberExists)}
        room.members.push(self.0);

        if !room.name.1 {
            if room.members.len() > 2 {
                room.name.0 = "Group message".to_string();
            } else {
                room.name.0 = self.1.to_string();
            }
        }

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
    type Output = Result<(), MessageExists>;

    fn id() -> Id {Id::hash("SendMessage")}

    fn apply(self, room: &mut ChatRoom, metadata: Metadata) -> Self::Output {
        room.messages.push(Message{author: metadata.signer, timestamp: metadata.timestamp, body: self.0});
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChangeRoomName(pub String);
impl Reactant<ChatRoom> for ChangeRoomName {
    type Output = ();

    fn id() -> Id {Id::hash("ChangeRoomName")}

    fn apply(self, room: &mut ChatRoom, metadata: Metadata) -> Self::Output {
        room.name.0 = self.0.to_string();
        room.name.1 = true;
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
            author: Profile::from_name(ctx, self.author).load_pending().to_pel(),
        }
    }
}
