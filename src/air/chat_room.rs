use prism::{Context, air::{Contract, Id, Instance, Metadata, Name}};
use prism::events::{DynEvent, WindowEvent};
use ptsd::layout::Offset;
use serde::{Deserialize, Serialize};

use std::{cell::RefCell, rc::Rc, time::Duration};

use crate::blocking::Block;
use crate::design_system::{
    AvatarContent, AvatarIconStyle, Callback, Footer, Gif, Header, Icons,
    MessageBarEvent, MessageContent, PageCallbacks, PageView, Photo, SendEvent, Sticker, SystemMessage,
    TextInputEvent, Theme, Video, VideoClip,
};
use crate::items::{Action, Display, Item};
use crate::profiles::Profile;
use crate::typing_ui::TypingPublisher;
use crate::{Bumper, ContactInvitations, Flow, Page, PageType};
use super::messages::{Message, RoomReadState};

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum ChatRoomAction {
    Share(Name), SendMessage(String), SendSticker(Sticker), SendGif(Gif), SendPhoto(Photo),
    SendVideo(Video), DeleteMessage(Message),
    // Append variants: existing signed actions must keep their discriminants.
    CreateGroup, RenameGroup(String), SetGroupPhoto(AvatarContent), Leave,
    AddMember(Name), LeaveGroup,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GroupDetails {
    pub name: String,
    pub photo: AvatarContent,
}
impl Default for GroupDetails {
    fn default() -> Self {
        Self {
            name: "Group message".into(),
            photo: AvatarContent::icon(Icons::Group, AvatarIconStyle::Secondary),
        }
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
        deserializer.deserialize_tuple(3, RoomVisitor)
    }
}

impl Contract for ChatRoom {
    type Message = ChatRoomAction;
    type Init = Id;
    type Result = usize;

    fn id() -> Id {Id::hash("ChatRoom")}

    fn init(init: Id, metadata: Metadata) -> Self {
        ChatRoom {members: vec![metadata.signer], messages: Vec::new(), id: init, group: None}
    }

    fn apply(&mut self, message: ChatRoomAction, metadata: Metadata) -> usize {
        if !self.members.contains(&metadata.signer) {return self.messages.len();}
        match message {
            ChatRoomAction::Share(recipient) => {
                if !self.members.contains(&recipient) {self.members.push(recipient);}
            }
            ChatRoomAction::SendMessage(message) => {
                self.append(MessageContent::Text(message), metadata);
            }
            ChatRoomAction::SendSticker(sticker) => {
                self.append(MessageContent::Sticker(sticker), metadata);
            }
            ChatRoomAction::SendGif(gif) => {self.append(MessageContent::Gif(gif), metadata);}
            ChatRoomAction::SendPhoto(photo) => {
                self.append(MessageContent::Photo(photo), metadata);
            }
            ChatRoomAction::SendVideo(video) => {
                self.append(MessageContent::Video(video), metadata);
            }
            ChatRoomAction::DeleteMessage(message) => {
                if let Some(i) = self.messages.iter().position(|stored| stored.id == message.id) {
                    let message = &mut self.messages[i];
                    if matches!(message.body, MessageContent::System(_)) {
                        return self.messages.len();
                    }
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
                if self.is_group() {
                    self.group.get_or_insert_with(GroupDetails::default).photo = photo;
                }
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
    pub fn can_message(&self, ctx: &mut Context) -> bool {
        let me = ctx.air.me();
        self.members.contains(&me) && self.direct_contact(me).filter(|name| *name != me)
            .is_none_or(|name| !Block::did_i_block(ctx, name) && !Block::am_i_blocked(ctx, name))
    }

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
            room.send(if group {ChatRoomAction::AddMember(recipient)}
                else {ChatRoomAction::Share(recipient)});
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
        let mut recipients: Vec<_> = recipients.into_iter()
            .filter(|name| !current.members.contains(name)).collect();
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
        let id = Id::hash(&(
            "GroupNotice.v1", self.id, metadata.signer, metadata.timestamp,
            &notice, self.messages.len(),
        ));
        self.messages.push(Message {
            author: metadata.signer, timestamp: metadata.timestamp,
            body: MessageContent::System(notice), id, hidden_from: Vec::new(),
        });
    }

    pub fn page(invitations: &ContactInvitations, ctx: &mut Context, room: Instance<ChatRoom>, target: Option<Id>) -> PageType {
        let room_data = room.pending();
        let my_name = ctx.air.me();
        let is_group = room_data.is_group();
        let has_left = !room_data.members.contains(&my_name);
        let reads = RoomReadState::open(ctx, room.id());
        let messages: Vec<Message> = room_data.messages(my_name).cloned().collect();
        let received: Vec<Id> = messages.iter().filter(|message| message.author != my_name)
            .map(|message| message.id).collect();

        let mut profiles = room_data.members.clone().into_iter()
            .map(|name| Profile::from_name(ctx, name))
            .filter(|profile| profile.pending().name != Some(my_name)).collect::<Vec<_>>();
        let did_i_block = if !is_group && profiles.len() == 1 {
            let profile = profiles[0].pending();
            Block::did_i_block(ctx, profile.name.unwrap()).then_some(profile.username.to_string())
        } else {None};
        let am_i_blocked = if !is_group && profiles.len() == 1 {
            let profile = profiles[0].pending();
            Block::am_i_blocked(ctx, profile.name.unwrap()).then_some(profile.username.to_string())
        } else {None};
        let is_blocked = has_left || did_i_block.is_some() || am_i_blocked.is_some();
        let display_profiles = profiles.iter_mut()
            .map(|profile| profile.pending().to_view()).collect();

        let info_room = room.clone();
        let info_invitations = invitations.clone();
        let info = Some(Box::new(move |ctx: &mut Context, theme: &Theme| {
            let page = Page::chat_info(&info_invitations, info_room.clone());
            (Flow::new(vec![page]).build(ctx, theme))(ctx, theme);
        }) as Box<dyn Callback>);
        let title = room_data.name(ctx);
        let avatar = is_group.then(|| room_data.group_details().photo);
        let notice = if has_left {Some("You left this group.".to_string())}
            else if let Some(username) = am_i_blocked {Some(format!("{} blocked you.", username))}
            else {did_i_block.map(|username| format!("You blocked {}.", username))};
        drop(room_data);

        let mut groups: Vec<Vec<Message>> = Vec::new();
        for message in messages {
            if groups.last().and_then(|group| group.last()).is_some_and(|last| {
                Some(last.id) != target && Some(message.id) != target
                    && !matches!(last.body, MessageContent::System(_))
                    && !matches!(message.body, MessageContent::System(_))
                    && last.author == message.author
                    && last.timestamp.abs_diff(message.timestamp) / 1_000_000_000 <= 60
            }) {groups.last_mut().unwrap().push(message);}
            else {groups.push(vec![message]);}
        }
        let target_index = target.and_then(|target| groups.iter()
            .position(|group| group.iter().any(|message| message.id == target)));
        let mut items: Vec<Item> = groups.into_iter().map(|messages| {
            let rendered = messages.into_iter().map(|message| {
                let room = room.clone();
                let tapped = message.clone();
                let invitations = invitations.clone();
                message.to_view(ctx, move |ctx: &mut Context, theme: &Theme| {
                    let info_room = room.clone();
                    let info_message = tapped.clone();
                    let info_invitations = invitations.clone();
                    let header = Some((Icons::Info, Action::deferred_flow(
                        move |ctx: &mut Context, _: &Theme| {
                            Flow::new(vec![Page::message_details(
                                &info_invitations, ctx, info_room.clone(), info_message.clone(), 2,
                            )])
                        },
                    )));
                    let page = match &tapped.body {
                        MessageContent::Photo(photo) => {
                            Page::Static(Box::new(PageType::photo(photo.clone(), header)))
                        }
                        MessageContent::Video(video) => {
                            Page::Static(Box::new(PageType::video(video.clone(), header)))
                        }
                        _ => Page::message_details(&invitations, ctx, room.clone(), tapped.clone(), 1),
                    };
                    (Flow::new(vec![page]).build(ctx, theme))(ctx, theme);
                })
            }).collect();
            Item::Display(Display::messages(rendered, is_group))
        }).collect();
        if !is_blocked {items.push(Item::Display(Display::Typing {room: room.clone()}));}
        let header = Header::messaging(title, display_profiles, avatar, 1, info);
        let mut send_room = room.clone();
        let footer = match notice {
            Some(notice) => Footer::alert(&notice),
            None => Footer::input("Message...", move |ctx: &mut Context, value: &mut String| {
                if !value.is_empty() && send_room.pending().can_message(ctx) {
                    send_room.send(ChatRoomAction::SendMessage(value.to_string()));
                }
            }, |_: &mut Context, value: &mut String| value.is_empty()),
        };
        let identity = Some(room.id().to_string());
        let lifecycle = Box::new(move |_: &mut Context, theme: &Theme, _: Option<crate::NavFn>, _: usize| -> PageCallbacks {
            let read_room = room.clone();
            let mut reads = reads.clone();
            let received = received.clone();
            let on_show = Box::new(move |ctx: &mut Context| {
                if !read_room.pending().members.contains(&ctx.air.me()) {return;}
                let unread = {
                    let reads = reads.pending();
                    received.iter().copied().filter(|id| !reads.read_flag(*id)).collect::<Vec<_>>()
                };
                if !unread.is_empty() {reads.send(unread);}
            });

            let mut room = room.clone();
            let event_theme = theme.clone();
            let typing = Rc::new(RefCell::new(TypingPublisher::default()));
            let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
                if matches!(event.downcast_ref::<WindowEvent>(), Some(WindowEvent::Focused(false)))
                    || matches!(event.downcast_ref::<ptsd::events::TextInput>(),
                        Some(ptsd::events::TextInput::Focused(false))) {
                    typing.borrow_mut().stop();
                }
                let text = event.downcast_ref::<TextInputEvent>();
                let send = event.downcast_ref::<SendEvent>();
                let capture = event.downcast_ref::<MessageBarEvent>() == Some(&MessageBarEvent::Capture);
                if text.is_none() && send.is_none() && !capture {return false;}
                let allowed = room.pending().can_message(ctx);
                if let Some(text) = text && let Some((field, focused)) = page.composer_state() {
                    match text {
                        TextInputEvent::Changed {id, value} if *id == field => {
                            if page.focused() && allowed && focused {
                                typing.borrow_mut().edit(ctx, &room, !value.is_empty());
                                let typing = typing.clone();
                                page.after(Duration::from_secs(3), move |_: &mut Context| {
                                    typing.borrow_mut().stop();
                                });
                            } else {typing.borrow_mut().stop();}
                        }
                        TextInputEvent::Submit {id} if *id == field => typing.borrow_mut().stop(),
                        _ => {}
                    }
                }
                if capture {
                    typing.borrow_mut().stop();
                    if !allowed {return true;}
                    let camera = PageType::camera(|ctx: &mut Context, photo: Photo| {
                        ctx.emit(ptsd::navigation::NavigationEvent::reset_with_fn(move |ctx: &mut Context| {
                            ctx.emit(crate::design_system::AttachmentEvent(SendEvent::Photo(photo.clone())));
                        }));
                    });
                    (Flow::new(vec![Page::Static(Box::new(camera))]).build(ctx, &event_theme))(
                        ctx, &event_theme,
                    );
                    return true;
                }
                if let Some(send) = send {
                    typing.borrow_mut().stop();
                    if !allowed {return true;}
                    match send {
                        SendEvent::Sticker(sticker) => {
                            room.send(ChatRoomAction::SendSticker(sticker.clone()));
                        }
                        SendEvent::Gif(gif) => {room.send(ChatRoomAction::SendGif(gif.clone()));}
                        SendEvent::Photo(photo) => {room.send(ChatRoomAction::SendPhoto(photo.clone()));}
                        SendEvent::Video(video) => {
                            let members = room.pending().members.clone();
                            let video = video.share(ctx, &members);
                            room.send(ChatRoomAction::SendVideo(video));
                        }
                    }
                }
                false
            });
            (Some(on_event), Some(on_show), None)
        });
        PageType::Content {
            header, items, offset: Offset::End, bumper: Bumper::Footer(footer),
            next: None, flow_len: 1, lifecycle: Some(lifecycle), identity,
            reveal_child: target_index,
        }
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

struct RoomVisitor;
impl<'de> serde::de::Visitor<'de> for RoomVisitor {
    type Value = ChatRoom;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {f.write_str("a chat room")}
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<ChatRoom, A::Error> {
        let members: Vec<Name> = seq.next_element()?
            .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
        let (members, messages, group) = if members.is_empty() {
            let GroupStorage::V1 {members, messages, details} = seq
                .next_element::<GroupStorage<Vec<Name>, Vec<Message>, GroupDetails>>()?
                .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
            (members, messages, Some(details))
        } else {
            let messages = seq.next_element()?
                .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
            (members, messages, None)
        };
        let id = seq.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
        Ok(ChatRoom {members, messages, id, group})
    }
}
