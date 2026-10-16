use pelican_ui::{Context, theme::Theme};
use pelican_ui::utils::Timestamp;
pub use pelican_ui::components::{MessageContent, Sticker, Gif};
pub use pelican_ui::components::{Photo, Video, VideoClip};

use maverick_os::air::{Id, Name, Metadata, Contract, Instance};

use serde::{Deserialize, Serialize};

use crate::profiles::Profile;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChatRoom {
    pub members: Vec<Name>,
    messages: Vec<Message>,
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
                let id = Id::hash(&(metadata.signer, metadata.timestamp));
                self.messages.push(Message{author: metadata.signer, timestamp: metadata.timestamp, body: MessageContent::Text(message), id, hidden_from: Vec::new()});
            },
            ChatRoomAction::SendSticker(sticker) => {
                let id = Id::hash(&(metadata.signer, metadata.timestamp));
                self.messages.push(Message{author: metadata.signer, timestamp: metadata.timestamp, body: MessageContent::Sticker(sticker), id, hidden_from: Vec::new()});
            },
            ChatRoomAction::SendGif(gif) => {
                let id = Id::hash(&(metadata.signer, metadata.timestamp));
                self.messages.push(Message{author: metadata.signer, timestamp: metadata.timestamp, body: MessageContent::Gif(gif), id, hidden_from: Vec::new()});
            },
            ChatRoomAction::SendPhoto(photo) => {
                let id = Id::hash(&(metadata.signer, metadata.timestamp));
                self.messages.push(Message{author: metadata.signer, timestamp: metadata.timestamp, body: MessageContent::Photo(photo), id, hidden_from: Vec::new()});
            },
            ChatRoomAction::SendVideo(video) => {
                let id = Id::hash(&(metadata.signer, metadata.timestamp));
                self.messages.push(Message{author: metadata.signer, timestamp: metadata.timestamp, body: MessageContent::Video(video), id, hidden_from: Vec::new()});
            },
            ChatRoomAction::DeleteMessage(message) => {
                if let Some(i) = self.messages.iter().position(|m| *m == message) {
                    let message = &mut self.messages[i];
                    if message.author == metadata.signer {
                        // Your own message: removed for everyone.
                        self.messages.remove(i);
                    } else if !message.hidden_from.contains(&metadata.signer) {
                        // Someone else's: hidden only for whoever deleted it.
                        message.hidden_from.push(metadata.signer);
                    }
                }
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

    pub fn messages(&self, me: Name) -> impl Iterator<Item = &Message> {
        self.messages.iter().filter(move |m| !m.hidden_from.contains(&me))
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum ChatRoomAction {
    Share(Name),
    SendMessage(String),
    SendSticker(Sticker),
    SendGif(Gif),
    SendPhoto(Photo), 
    SendVideo(Video),
    DeleteMessage(Message),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub struct Message {
    pub body: MessageContent,
    pub author: Name,
    pub timestamp: u64,
    pub hidden_from: Vec<Name>,
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