use pelican_ui::{Context, theme::Theme};
use pelican_ui::utils::Timestamp;
pub use pelican_ui::components::{MessageContent, Sticker, StickerPack, Gif};
pub use pelican_ui::components::{Photo, Video, VideoClip};

use prism::air::{Id, Name, Metadata, Contract, Instance};

use serde::{Deserialize, Serialize};

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
    pub fn create(ctx: &mut Context, recipients: Vec<Name>) -> Instance<ChatRoom> {
        let is_dm = recipients.len() <= 1;
        let id = match is_dm {
            true => Id::hash(&recipients[0]), false => Id::random(),
        };

        let mut room = ctx.air.instances::<ChatRoom>().create(id);

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
