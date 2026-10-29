use pelican_ui::{Context, theme::Theme};
use pelican_ui::utils::Timestamp;
pub use pelican_ui::components::{MessageContent, Sticker, StickerPack, Gif};
pub use pelican_ui::components::{Photo, Video, VideoClip};

use prism::air::{Id, Name, Metadata, Contract, Instance};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::profiles::Profile;

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum ChatRoomAction {
    Share(Name), SendMessage(String), SendSticker(Sticker), SendGif(Gif), SendPhoto(Photo),
    SendVideo(Video), DeleteMessage(Message),
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
    pub fn to_pel(&self, ctx: &mut Context, on_click: impl FnMut(&mut Context, &Theme) + Clone + 'static) -> pelican_ui::components::Message {
        pelican_ui::components::Message {
            body: self.body.clone(), timestamp: Timestamp::from_u64(self.timestamp),
            author: Profile::from_name(ctx, self.author).pending().to_pel(),
            on_click: Box::new(on_click),
        }
    }
}

/// Private, per-reader flags kept outside the shared message's binary schema.
/// Missing flags mean unread; a reader's own messages are always read.
#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
pub struct RoomReadState {
    pub room: Id,
    pub owner: Name,
    read: BTreeMap<Id, bool>,
}

impl Contract for RoomReadState {
    type Init = Id;
    type Message = Vec<Id>;
    type Result = ();

    fn id() -> Id {Id::hash("Orange.RoomReadState.v1")}
    fn init(room: Id, metadata: Metadata) -> Self {
        Self {room, owner: metadata.signer, read: BTreeMap::new()}
    }
    fn apply(&mut self, messages: Vec<Id>, metadata: Metadata) {
        if metadata.signer == self.owner {
            for id in messages {self.read.insert(id, true);}
        }
    }
}

impl RoomReadState {
    pub fn open(ctx: &Context, room: Id) -> Instance<Self> {
        // AIR includes the creator in the location. Never share this instance
        // or enable notifications: reading must not notify the other members.
        ctx.air.instances::<Self>().create(room)
    }

    pub fn is_read(&self, message: &Message) -> bool {
        message.author == self.owner || self.read_flag(message.id)
    }

    pub fn read_flag(&self, id: Id) -> bool {self.read.get(&id).copied().unwrap_or(false)}

    pub fn unread<'a>(&'a self, room: &'a ChatRoom) -> impl Iterator<Item = Id> + 'a {
        room.messages(self.owner).filter(|message| !self.is_read(message)).map(|message| message.id)
    }
}

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

    fn id() -> Id { Id::hash("ChatRoom") }

    fn init(init: Self::Init, metadata: Metadata) -> Self {
        ChatRoom { members: vec![metadata.signer], messages: Vec::new(), id: init }
    }

    fn apply(&mut self, message: Self::Message, metadata: Metadata) -> Self::Result {
        match message {
            ChatRoomAction::Share(recipient) => {
                if !self.members.contains(&recipient) { self.members.push(recipient); }
            }
            ChatRoomAction::SendMessage(message) => {
                self.append(MessageContent::Text(message), metadata);
            }
            ChatRoomAction::SendSticker(sticker) => {
                self.append(MessageContent::Sticker(sticker), metadata);
            }
            ChatRoomAction::SendGif(gif) => { self.append(MessageContent::Gif(gif), metadata); }
            ChatRoomAction::SendPhoto(photo) => {
                self.append(MessageContent::Photo(photo), metadata);
            }
            ChatRoomAction::SendVideo(video) => {
                self.append(MessageContent::Video(video), metadata);
            }
            ChatRoomAction::DeleteMessage(message) => {
                if let Some(i) = self.messages.iter().position(|m| *m == message) {
                    let message = &mut self.messages[i];
                    if message.author == metadata.signer {
                        self.messages.remove(i);
                    } else if !message.hidden_from.contains(&metadata.signer) {
                        message.hidden_from.push(metadata.signer);
                    }
                }
            }
        }

        self.messages.len()
    }
}

impl ChatRoom {
    pub fn create(ctx: &mut Context, mut recipients: Vec<Name>) -> Instance<ChatRoom> {
        let me = ctx.air.me();
        recipients.retain(|name| *name != me);
        recipients.sort_unstable();
        recipients.dedup();
        let rooms = ctx.air.instances::<ChatRoom>();
        let id = if recipients.len() <= 1 {
            let recipient = recipients.first().copied().unwrap_or(me);
            // Received and older DMs may have a different creator or init ID.
            // Reopen their existing history before creating our name-hashed room.
            let existing = rooms.list().into_values().filter(|instance| {
                let room = instance.pending();
                room.members.contains(&me) && room.members.contains(&recipient)
                    && room.members.iter().all(|name| *name == me || *name == recipient)
            }).min_by_key(|instance| {
                // Prefer actual conversations over empty duplicates, with a
                // stable tie-breaker independent of HashMap iteration order.
                (instance.pending().messages.is_empty(), instance.id())
            });
            if let Some(room) = existing {return room;}
            Id::hash(&recipient)
        } else {
            Id::random()
        };

        let mut room = rooms.create(id);

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
            false => *self.members.iter().find(|p| **p != ctx.air.me()).unwrap(),
        };

        let profile = Profile::from_name(ctx, member);
        let username = &profile.pending().username;

        match member == ctx.air.me() {
            true => format!("{username} (You)"), false => username.to_string(),
        }
    }

    pub fn messages(&self, me: Name) -> impl Iterator<Item = &Message> {
        self.messages.iter().filter(move |m| !m.hidden_from.contains(&me))
    }

    fn append(&mut self, body: MessageContent, metadata: Metadata) {
        let id = Id::hash(&(metadata.signer, metadata.timestamp));
        self.messages.push(Message {
            author: metadata.signer, timestamp: metadata.timestamp, body, id,
            hidden_from: Vec::new(),
        });
    }
}
