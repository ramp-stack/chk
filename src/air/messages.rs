use pelican_ui::Context;
use pelican_ui::utils::Timestamp;

use maverick_os::air::{Id, Name, Metadata, Contract, Instance};

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::profiles::Profile;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChatRoom {
    pub members: Vec<Name>,
    pub messages: Vec<Message>,
    pub id: Id,
}

impl Contract for ChatRoom {
    type Init = Id;
    type Message = ChatRoomAction;
    type Result = usize;

    fn id() -> Id {Id::hash("ChatRoom")}

    fn init(init: Self::Init, metadata: Metadata) -> Self {
        ChatRoom {
            members: vec![metadata.signer],
            messages: Vec::new(),
            id: init,
        }
    }

    // Send message
    fn apply(&mut self, message: Self::Message, metadata: Metadata) -> Self::Result {
        match message {
            ChatRoomAction::SendMessage(message) => {
                self.messages.push(Message{author: metadata.signer, timestamp: metadata.timestamp, body: message});
            },
            ChatRoomAction::Share(recipient) => {
                if !self.members.contains(&recipient) {
                    self.members.push(recipient);
                }
            }
        }
        
        self.messages.len()
    }
}

impl ChatRoom {
    pub fn name(&self, ctx: &mut Context) -> String {
        if self.members.len() > 2 {
            "Group message".to_string()
        } else {
            let members = self.members.iter().collect::<Vec<_>>();
            
            members.first().map(|p| {
                let mut profile = Profile::from_name(ctx, **p);
                if **p == ctx.me() {
                    format!("{} (You)", profile.pending().username)
                } else {
                    profile.pending().username.to_string()
                }
            }).unwrap_or("Orange User".to_string())
        }
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum ChatRoomAction {
    SendMessage(String),
    Share(Name)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq, Hash)]
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
