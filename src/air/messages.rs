use prism::{Context, air::{Air, Id, Name, Metadata, Contract, Instance}};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use crate::design_system::{Theme, AvatarContent, AvatarIconStyle, Icons};
use crate::profiles::Profile;
pub use crate::design_system::{MessageContent, SystemMessage, Sticker, StickerPack, Gif};
pub use crate::design_system::{Photo, Video, VideoClip};

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum ChatRoomAction {
    Share(Name), SendMessage(String), SendSticker(Sticker), SendGif(Gif), SendPhoto(Photo),
    SendVideo(Video), DeleteMessage(Message),
    // Append variants: existing signed actions must keep their discriminants.
    CreateGroup, RenameGroup(String), SetGroupPhoto(AvatarContent), Leave,
    AddMember(Name), LeaveGroup,
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
    pub fn to_view(&self, ctx: &mut Context, on_click: impl FnMut(&mut Context, &Theme) + Clone + 'static) -> crate::design_system::MessageView {
        let display_author = match &self.body {
            MessageContent::System(notice) => notice.member(),
            _ => self.author,
        };
        crate::design_system::MessageView {
            body: self.body.clone(), timestamp: ptsd::utils::Timestamp::from_u64(self.timestamp),
            author: Profile::from_name(ctx, display_author).pending().to_view(), on_click: Box::new(on_click),
        }
    }

    pub fn preview(&self, ctx: &mut Context, group: bool) -> String {
        self.preview_with(ctx.air.me(), group, |name| {
            Profile::from_name(ctx, name).pending().username.clone()
        })
    }

    pub(crate) fn preview_with(&self, me: Name, group: bool, mut username: impl FnMut(Name) -> String) -> String {
        let text = match &self.body {
            MessageContent::System(notice) => {
                return notice.text_for(me, &username(notice.member()));
            }
            MessageContent::Text(text) => text.as_str(),
            MessageContent::Sticker(_) => "Sticker",
            MessageContent::Gif(_) => "GIF",
            MessageContent::Photo(_) => "Photo",
            MessageContent::Video(_) => "Video",
        };
        if self.author == me {format!("You: {text}")}
        else if group {format!("{}: {text}", username(self.author))}
        else {text.to_string()}
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

/// Private direct-message preference, saved even before a conversation exists.
#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
pub struct ContactNotificationState {
    pub contact: Name,
    pub owner: Name,
    muted: bool,
}

impl Contract for ContactNotificationState {
    type Init = Name;
    type Message = bool;
    type Result = ();

    fn id() -> Id {Id::hash("CHK.ContactNotificationState.v1")}
    fn init(contact: Name, metadata: Metadata) -> Self {
        Self {contact, owner: metadata.signer, muted: false}
    }
    fn apply(&mut self, muted: bool, metadata: Metadata) {
        if metadata.signer == self.owner {self.muted = muted;}
    }
}

impl ContactNotificationState {
    pub fn find(air: &Air, contact: Name) -> Option<Instance<Self>> {
        let owner = air.me();
        air.instances::<Self>().list().into_values().find(|instance| {
            let preferences = instance.pending();
            preferences.contact == contact && preferences.owner == owner
        })
    }

    pub fn is_muted_for(air: &Air, contact: Name) -> bool {
        if let Some(preferences) = ContactNotificationState::find(air, contact) {return preferences.pending().muted;}
        // Preserve old DM mutes until the owner explicitly sets a person preference.
        let owner = air.me();
        let muted_rooms: BTreeSet<_> = air.instances::<RoomNotificationState>().list().into_values()
            .filter_map(|instance| {
                let preferences = instance.pending();
                (preferences.owner == owner && preferences.muted).then_some(preferences.room)
            }).collect();
        !muted_rooms.is_empty() && air.instances::<ChatRoom>().list().into_values().any(|room| {
            muted_rooms.contains(&room.id()) && room.pending().direct_contact(owner) == Some(contact)
        })
    }

    pub fn set_muted(ctx: &Context, contact: Name, muted: bool) {
        let mut preferences = ContactNotificationState::find(&ctx.air, contact)
            .unwrap_or_else(|| ctx.air.instances::<Self>().create(contact));
        if preferences.pending().muted != muted {preferences.send(muted);}
        // Never share this preference or create a room just to apply it.
        let me = ctx.air.me();
        for room in ctx.air.instances::<ChatRoom>().list().into_values() {
            if room.pending().direct_contact(me) == Some(contact) {
                RoomNotificationState::apply_to(ctx, &room);
            }
        }
    }
}

/// Private group preferences and legacy DM preferences; never shared with other members.
#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
pub struct RoomNotificationState {
    pub room: Id,
    pub owner: Name,
    muted: bool,
}

impl Contract for RoomNotificationState {
    type Init = Id;
    type Message = bool;
    type Result = ();

    fn id() -> Id {Id::hash("CHK.RoomNotificationState.v1")}
    fn init(room: Id, metadata: Metadata) -> Self {
        Self {room, owner: metadata.signer, muted: false}
    }
    fn apply(&mut self, muted: bool, metadata: Metadata) {
        if metadata.signer == self.owner {self.muted = muted;}
    }
}

impl RoomNotificationState {
    pub fn find(air: &Air, room: Id) -> Option<Instance<Self>> {
        let owner = air.me();
        air.instances::<Self>().list().into_values().find(|instance| {
            let preferences = instance.pending();
            preferences.room == room && preferences.owner == owner
        })
    }

    pub fn open(ctx: &Context, room: Id) -> Instance<Self> {
        // Creating an existing instance still notifies list watchers. Keep reads
        // quiet so rebuilding chat info cannot trigger another rebuild itself.
        RoomNotificationState::find(&ctx.air, room).unwrap_or_else(|| ctx.air.instances::<Self>().create(room))
    }

    pub fn is_muted(&self) -> bool {self.muted}

    pub fn is_muted_for(air: &Air, room: &Instance<ChatRoom>) -> bool {
        match room.pending().direct_contact(air.me()) {
            Some(contact) => ContactNotificationState::is_muted_for(air, contact),
            None => RoomNotificationState::find(air, room.id()).is_some_and(|preferences| preferences.pending().muted),
        }
    }

    pub fn enabled(air: &Air, room: &Instance<ChatRoom>) -> bool {
        room.pending().members.contains(&air.me()) && !RoomNotificationState::is_muted_for(air, room)
    }

    pub fn set_muted(ctx: &Context, room: &Instance<ChatRoom>, muted: bool) {
        if let Some(contact) = room.pending().direct_contact(ctx.air.me()) {
            ContactNotificationState::set_muted(ctx, contact, muted);
            return;
        }
        let mut preferences = RoomNotificationState::open(ctx, room.id());
        if preferences.pending().is_muted() != muted {preferences.send(muted);}
        RoomNotificationState::apply_to(ctx, room);
    }

    pub fn apply_to(ctx: &Context, room: &Instance<ChatRoom>) {
        room.notify(RoomNotificationState::enabled(&ctx.air, room));
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GroupDetails {
    pub name: String,
    pub photo: AvatarContent,
}
impl Default for GroupDetails {
    fn default() -> Self {
        Self {name: "Group message".into(), photo: AvatarContent::icon(Icons::Group, AvatarIconStyle::Secondary)}
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChatRoom {
    pub members: Vec<Name>,
    messages: Vec<Message>,
    pub id: Id,
    group: Option<GroupDetails>,
}

impl Serialize for ChatRoom {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.group {
            None => (&self.members, &self.messages, self.id).serialize(serializer),
            Some(details) => (Vec::<Name>::new(), GroupStorage::V1 {
                members: &self.members, messages: &self.messages, details,
            }, self.id).serialize(serializer),
        }
    }
}
impl<'de> Deserialize<'de> for ChatRoom {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RoomVisitor;
        impl<'de> serde::de::Visitor<'de> for RoomVisitor {
            type Value = ChatRoom;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {f.write_str("a chat room")}
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<ChatRoom, A::Error> {
                let members: Vec<Name> = seq.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let (members, messages, group) = if members.is_empty() {
                    let GroupStorage::V1 {members, messages, details}: GroupStorage<Vec<Name>, Vec<Message>, GroupDetails> = seq.next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                    (members, messages, Some(details))
                } else {
                    (members, seq.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(1, &self))?, None)
                };
                let id = seq.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
                Ok(ChatRoom {members, messages, id, group})
            }
        }
        deserializer.deserialize_tuple(3, RoomVisitor)
    }
}

impl Contract for ChatRoom {
    type Init = Id;
    type Message = ChatRoomAction;
    type Result = usize;

    fn id() -> Id { Id::hash("ChatRoom") }

    fn init(init: Self::Init, metadata: Metadata) -> Self {
        ChatRoom { members: vec![metadata.signer], messages: Vec::new(), id: init, group: None }
    }

    fn apply(&mut self, message: Self::Message, metadata: Metadata) -> Self::Result {
        if !self.members.contains(&metadata.signer) {return self.messages.len();}
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
                if matches!(message.body, MessageContent::System(_)) {return self.messages.len();}
                if let Some(i) = self.messages.iter().position(|m| *m == message) {
                    let message = &mut self.messages[i];
                    if message.author == metadata.signer {
                        self.messages.remove(i);
                    } else if !message.hidden_from.contains(&metadata.signer) {
                        message.hidden_from.push(metadata.signer);
                    }
                }
            }
            ChatRoomAction::CreateGroup => {
                if self.members.len() == 1 && self.messages.is_empty() {
                    self.group.get_or_insert_with(GroupDetails::default);
                }
            }
            ChatRoomAction::RenameGroup(name) => {
                let name = name.trim();
                if self.is_group() && !name.is_empty() && name.chars().count() <= 80 {
                    self.group.get_or_insert_with(GroupDetails::default).name = name.into();
                }
            }
            ChatRoomAction::SetGroupPhoto(photo) => {
                if self.is_group() {self.group.get_or_insert_with(GroupDetails::default).photo = photo;}
            }
            ChatRoomAction::Leave => {
                if self.is_group() {
                    self.group.get_or_insert_with(GroupDetails::default);
                    self.members.retain(|name| *name != metadata.signer);
                }
            }
            // New actions keep old Share/Leave replays unchanged. Membership
            // and its notice are one AIR update, not two separate notifications.
            ChatRoomAction::AddMember(recipient) => {
                if self.is_group() && !self.members.contains(&recipient) {
                    self.members.push(recipient);
                    self.append_system(SystemMessage::MemberAdded(recipient), metadata);
                }
            }
            ChatRoomAction::LeaveGroup => {
                if self.is_group() {
                    self.group.get_or_insert_with(GroupDetails::default);
                    self.members.retain(|name| *name != metadata.signer);
                    self.append_system(SystemMessage::MemberLeft(metadata.signer), metadata);
                }
            }
        }

        self.messages.len()
    }
}

impl ChatRoom {
    pub fn direct_contact(&self, me: Name) -> Option<Name> {
        if self.is_group() || !self.members.contains(&me) {return None;}
        let contact = self.members.iter().copied().find(|name| *name != me).unwrap_or(me);
        self.members.iter().all(|name| *name == me || *name == contact).then_some(contact)
    }

    pub fn start_notifications(ctx: &Context) {
        ctx.air.start::<super::notifications::MessageNotifications>(());
    }

    pub fn find_direct(ctx: &Context, recipient: Name) -> Option<Instance<Self>> {
        let me = ctx.air.me();
        // Received and older DMs may have a different creator or init ID.
        ctx.air.instances::<Self>().list().into_values().filter(|instance| {
            let room = instance.pending();
            !room.is_group() && room.members.contains(&me) && room.members.contains(&recipient)
                && room.members.iter().all(|name| *name == me || *name == recipient)
        }).min_by_key(|instance| {
            // Prefer actual conversations over empty duplicates, with a
            // stable tie-breaker independent of HashMap iteration order.
            (instance.pending().messages.is_empty(), instance.id())
        })
    }

    pub fn create(ctx: &mut Context, mut recipients: Vec<Name>) -> Instance<ChatRoom> {
        let me = ctx.air.me();
        recipients.retain(|name| *name != me);
        recipients.sort_unstable();
        recipients.dedup();
        let rooms = ctx.air.instances::<ChatRoom>();
        let id = if recipients.len() <= 1 {
            let recipient = recipients.first().copied().unwrap_or(me);
            if let Some(room) = ChatRoom::find_direct(ctx, recipient) {return room;}
            Id::hash(&recipient)
        } else {
            Id::random()
        };

        let mut room = rooms.create(id);
        let group = recipients.len() > 1;
        if group {room.send(ChatRoomAction::CreateGroup);}

        for recipient in recipients {
            room.share(recipient);
            room.send(if group {ChatRoomAction::AddMember(recipient)} else {ChatRoomAction::Share(recipient)});
        }

        room
    }

    pub fn name(&self, ctx: &mut Context) -> String {
        if self.is_group() {return self.group_details().name;}

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

    pub fn messages(&self, me: Name) -> impl DoubleEndedIterator<Item = &Message> {
        self.messages.iter().filter(move |m| !m.hidden_from.contains(&me))
    }

    pub fn is_group(&self) -> bool {self.group.is_some() || self.members.len() > 2}

    pub fn group_details(&self) -> GroupDetails {self.group.clone().unwrap_or_default()}

    pub fn add_members(ctx: &mut Context, room: &mut Instance<Self>, recipients: Vec<Name>) {
        let current = room.pending();
        if !current.is_group() || !current.members.contains(&ctx.air.me()) {return;}
        let mut recipients: Vec<_> = recipients.into_iter().filter(|name| !current.members.contains(name)).collect();
        recipients.sort_unstable();
        recipients.dedup();
        // Video attachments use separate AIR instances; invitees need those too.
        let videos: Vec<_> = current.messages.iter().filter_map(|message| {
            if let MessageContent::Video(video) = &message.body {Some(video.clone())} else {None}
        }).collect();
        drop(current);
        let clips = if videos.is_empty() {Vec::new()} else {
            ctx.air.instances::<VideoClip>().list().into_values().filter(|clip| {
                videos.iter().any(|video| video.clip == clip.pending().id)
            }).collect()
        };
        for recipient in recipients {
            room.send(ChatRoomAction::AddMember(recipient));
            room.share(recipient);
            for clip in &clips {clip.share(recipient);}
        }
    }

    pub fn append(&mut self, body: MessageContent, metadata: Metadata) {
        let id = Id::hash(&(metadata.signer, metadata.timestamp));
        self.messages.push(Message {
            author: metadata.signer, timestamp: metadata.timestamp, body, id,
            hidden_from: Vec::new(),
        });
    }

    pub fn append_system(&mut self, notice: SystemMessage, metadata: Metadata) {
        let id = Id::hash(&("GroupNotice.v1", self.id, metadata.signer, metadata.timestamp, &notice, self.messages.len()));
        self.messages.push(Message {
            author: metadata.signer, timestamp: metadata.timestamp,
            body: MessageContent::System(notice), id, hidden_from: Vec::new(),
        });
    }
}

// Old rooms are (members, messages, id), nested inside AIR's cached instances.
// An empty first vector was impossible in the old schema (the creator never
// left). Reserve it as a version marker, without reading past an old room into
// the surrounding cache. Ordinary DMs retain their exact existing encoding.
#[derive(Serialize, Deserialize)]
enum GroupStorage<M, T, D> {
    V1 {members: M, messages: T, details: D},
}
