use pelican_ui::{Context, theme::Theme};
use pelican_ui::utils::Timestamp;
use pelican_ui::components::{MessageGroups};
pub use pelican_ui::components::{MessageContent, Sticker, Gif};

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
            ChatRoomAction::Share(recipient) => {
                if !self.members.contains(&recipient) {
                    self.members.push(recipient);
                }
            },
            ChatRoomAction::SendMessage(message) => {
                self.messages.push(Message{author: metadata.signer, timestamp: metadata.timestamp, body: MessageContent::Text(message), id: Id::random(), deleted: false});
            },
            ChatRoomAction::SendSticker(sticker) => {
                self.messages.push(Message{author: metadata.signer, timestamp: metadata.timestamp, body: MessageContent::Sticker(sticker), id: Id::random(), deleted: false});
            },
            ChatRoomAction::SendGif(gif) => {
                self.messages.push(Message{author: metadata.signer, timestamp: metadata.timestamp, body: MessageContent::Gif(gif), id: Id::random(), deleted: false});
            },
            ChatRoomAction::DeleteMessage(message) => {
                // if is_mine {
                self.messages.retain(|m| *m != message);
                // } else {
                //     if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message.id) {
                //         msg.deleted = true;
                //     }
                // }
            }
        }
        
        self.messages.len()
    }
}

impl ChatRoom {
    pub fn create(ctx: &mut Context, recipients: Vec<Name>) -> Instance<ChatRoom> {
        let is_dm = recipients.len() <= 1;
        let id = match is_dm {
            true => Id::hash(&recipients[0]),
            false => Id::random(),
        };

        let mut room = ctx.create::<ChatRoom>(id);

        for recipient in recipients {
            room.share(recipient);
            room.send(ChatRoomAction::Share(recipient));
        }

        room
    }
    
    pub fn name(&self, ctx: &mut Context) -> String {
        if self.members.len() > 2 { return "Group message".into(); }

        let member = match self.members.len() == 1 {
            true => self.members[0],
            false => *self.members.iter().find(|p| **p != ctx.me()).unwrap()
        };

        let profile = Profile::from_name(ctx, member);
        let username = &profile.pending().username;

        match member == ctx.me() {
            true => format!("{username} (You)"),
            false => username.to_string()
        }
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum ChatRoomAction {
    Share(Name),
    SendMessage(String),
    SendSticker(Sticker),
    SendGif(Gif),
    DeleteMessage(Message),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub struct Message {
    pub body: MessageContent,
    pub author: Name,
    pub timestamp: u64,
    pub deleted: bool,
    pub id: Id,
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

    pub fn to_pel(&self, ctx: &mut Context, on_click: impl FnMut(&mut Context, &Theme) + Clone + 'static) -> pelican_ui::components::Message {
        pelican_ui::components::Message {
            body: self.body.clone(),
            timestamp: Timestamp::from_u64(self.timestamp),
            author: Profile::from_name(ctx, self.author).pending().to_pel(),
            on_click: Box::new(on_click),
        }
    }
}