use prism::{Context, air::{Contract, Id, Instance, Metadata, Name}};
use prism::events::{DynEvent, ListenCallback};
use prism::hardware::Data;
use ptsd::layout::Offset;
use ptsd::navigation::NavigationEvent;
use ptsd::utils::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::{
    cell::{Cell, RefCell}, collections::BTreeMap, future::Future, io::Write, pin::Pin,
    rc::Rc, sync::Arc,
};

use crate::design_system::{Header, Icons, MessageView, PageCallbacks, PageView, Task, Theme};
use crate::form::SubmitRequest;
use crate::profiles::Profile;
use crate::items::{Action, Display, Item};
use crate::{ActionItem, Bumper, ContactInvitations, ContractWatch, Flow, NavFn, Page, PageType, ValueItem};
pub use crate::design_system::{MessageContent, SystemMessage, Sticker, StickerPack, Gif};
pub use crate::design_system::{Photo, Video, VideoClip};
pub use super::chat_room::{ChatRoom, ChatRoomAction, GroupDetails};
pub use super::notifications::{ContactNotificationState, RoomNotificationState};

type SaveRequest = Pin<Box<dyn Future<Output = Result<Data, String>>>>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub struct Message {
    pub body: MessageContent,
    pub author: Name,
    pub timestamp: u64,
    pub hidden_from: Vec<Name>,
    pub id: Id,
}

impl Message {
    pub fn to_view(&self, ctx: &mut Context, on_click: impl FnMut(&mut Context, &Theme) + Clone + 'static) -> MessageView {
        let display_author = match &self.body {
            MessageContent::System(notice) => notice.member(),
            _ => self.author,
        };
        MessageView {
            body: self.body.clone(), timestamp: Timestamp::from_u64(self.timestamp),
            author: Profile::from_name(ctx, display_author).pending().to_view(),
            on_click: Box::new(on_click),
        }
    }

    pub fn preview(&self, ctx: &mut Context, group: bool) -> String {
        self.preview_with(ctx.air.me(), group, |name| {
            Profile::from_name(ctx, name).pending().username.clone()
        })
    }

    pub fn preview_with(&self, me: Name, group: bool, mut username: impl FnMut(Name) -> String) -> String {
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

    pub fn details(
        &self, invitations: &ContactInvitations, ctx: &mut Context, mut room: Instance<ChatRoom>,
        pops: usize, share: Option<ActionItem>,
    ) -> Vec<Item> {
        let is_mine = self.author == ctx.air.me();
        let author = self.author;
        let sender_name = if is_mine {"You".to_string()}
            else {Profile::from_name(ctx, author).pending().username.clone()};
        let bubble = self.to_view(ctx, |_: &mut Context, _: &Theme| {});

        let mut actions = Vec::new();
        if !matches!(self.body, MessageContent::System(_)) {
            let message = self.clone();
            actions.push(ActionItem::new(Action::custom(move |ctx: &mut Context, _: &Theme| {
                room.send(ChatRoomAction::DeleteMessage(message.clone()));
                (0..pops).for_each(|_| ctx.emit(NavigationEvent::Pop));
            }), "Delete", Icons::Delete));
        }
        let timestamp = Timestamp::from_u64(self.timestamp);
        if let Some(copy) = ActionItem::copy_message(ctx, &self.body) {actions.push(copy);}
        if let Some(share) = share {actions.push(share);}
        let invitations = invitations.clone();
        actions.push(ActionItem::new(Action::deferred_flow(move |ctx: &mut Context, _: &Theme| {
            let mut profile = Profile::from_name(ctx, author);
            Flow::new(vec![Page::profile(&invitations, ctx, &mut profile)])
        }), "View profile", Icons::Profile));

        vec![
            Display::messages(vec![bubble], false),
            Display::table("Info", vec![
                ValueItem::new("Sender", &sender_name),
                ValueItem::new("Time sent", &timestamp.time()),
                ValueItem::new("Date sent", &timestamp.date()),
            ], Some(actions)),
        ].into_iter().map(Item::Display).collect()
    }

    pub fn image_extension(bytes: &[u8]) -> Option<&'static str> {
        if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {Some("gif")}
        else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {Some("png")}
        else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {Some("jpg")}
        else {None}
    }

    pub async fn export(bytes: Arc<[u8]>, extension: &'static str) -> Result<Data, String> {
        Task::spawn(move || {
            // Keep the original encoding, including GIF animation and video audio.
            // The system owns temporary-directory cleanup; the share sheet may
            // still be reading the file after the detail page has been closed.
            let path = std::env::temp_dir()
                .join(format!("message-{}.{}", Uuid::new_v4(), extension));
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)
                .map_err(|_| "Could not prepare this media for saving.".to_string())?;
            file.write_all(&bytes)
                .map_err(|_| "Could not prepare this media for saving.".to_string())?;
            Ok(Data::File(path))
        }).next().await.ok_or_else(|| "Could not prepare this media for saving.".to_string())?
    }

    pub fn page(&self, invitations: &ContactInvitations, ctx: &Context, room: Instance<ChatRoom>, pops: usize) -> PageType {
        let title = if self.author == ctx.air.me() {"Sent message"} else {"Received message"};
        let message = self.clone();
        let invitations = invitations.clone();
        let lifecycle = Box::new(move |ctx: &mut Context, theme: &Theme, _: Option<NavFn>, _: usize| -> PageCallbacks {
            let message = message.clone();
            let room = room.clone();
            let id = Uuid::new_v4();
            let content = message.body.clone();
            let share = (!matches!(content, MessageContent::Text(_) | MessageContent::System(_))).then(|| {
                ActionItem::new(Action::custom(move |ctx: &mut Context, _: &Theme| {
                    ctx.emit(SubmitRequest(id));
                }), "Share", Icons::Send)
            });
            let profile = Profile::from_name(ctx, message.author);
            let error = Rc::new(RefCell::new(None::<String>));
            let visible_error = error.clone();
            let render_theme = theme.clone();
            let invitations = invitations.clone();
            let update = move |ctx: &mut Context, page: &mut PageView, _: Instance<Profile>| {
                let mut items = message.details(&invitations, ctx, room.clone(), pops, share.clone());
                if let Some(error) = visible_error.borrow().as_deref() {
                    items.push(Item::Display(Display::instructions(error)));
                }
                *page.content_mut().children_mut() = items.into_iter()
                    .flat_map(|item| item.build(ctx, &render_theme)).collect();
            };
            let watcher = ContractWatch::new(profile.clone());
            let render = update.clone();
            let on_create: ListenCallback<PageView> = Box::new(move |page, ctx| {
                page.watch(ctx, watcher, render);
            });
            let event_theme = theme.clone();
            let mut request = None;
            let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
                if !matches!(event.downcast_ref::<SubmitRequest>(),
                    Some(SubmitRequest(target)) if *target == id) {
                    return false;
                }
                if request.is_some_and(|request| page.pending(request)) {return true;}
                let future: SaveRequest = match &content {
                    MessageContent::Photo(photo) => {
                        Box::pin(Message::export(Arc::from(photo.jpeg()), "jpg"))
                    }
                    MessageContent::Sticker(sticker) => {
                        let image = sticker.image();
                        Box::pin(async move {Ok(Data::Image(image))})
                    }
                    MessageContent::Video(video) => {
                        let bytes = crate::messages::VideoClip::find(ctx, video.clip);
                        Box::pin(async move {
                            let bytes = bytes.ok_or(
                                "This video is still downloading. Try again when it finishes.",
                            )?;
                            Message::export(bytes, "mp4").await
                        })
                    }
                    MessageContent::Gif(gif) => {
                        let bytes = event_theme.gif_bytes(gif);
                        Box::pin(async move {
                            let bytes = bytes.await?;
                            let extension = Message::image_extension(&bytes)
                                .or_else(|| (bytes.get(4..8) == Some(b"ftyp")).then_some("mp4"))
                                .ok_or("This GIF's format cannot be saved to Photos.")?;
                            Message::export(bytes, extension).await
                        })
                    }
                    _ => return true,
                };
                let (error, update, profile) = (error.clone(), update.clone(), profile.clone());
                let shared = Cell::new(false);
                request = Some(page.spawn(future, move |ctx, page, result| {
                    *error.borrow_mut() = match result {
                        Ok(data) => {
                            if !shared.replace(true) {ctx.hardware.share(&data);}
                            None
                        }
                        Err(error) => Some(error),
                    };
                    update(ctx, page, profile.clone());
                }));
                true
            });
            (Some(on_event), None, Some(on_create))
        });
        PageType::Content {
            header: Header::stack(title, None), items: vec![], offset: Offset::Start,
            bumper: Bumper::None, next: None, flow_len: 1, lifecycle: Some(lifecycle),
            identity: None, reveal_child: None,
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
    type Message = Vec<Id>;
    type Init = Id;
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
        let owner = ctx.air.me();
        ctx.air.instances::<RoomReadState>().list().into_values().find(|instance| {
            let state = instance.pending();
            state.room == room && state.owner == owner
        }).unwrap_or_else(|| ctx.air.instances::<RoomReadState>().create(room))
    }

    pub fn is_read(&self, message: &Message) -> bool {
        message.author == self.owner || self.read_flag(message.id)
    }

    pub fn read_flag(&self, id: Id) -> bool {self.read.get(&id).copied().unwrap_or(false)}

    pub fn unread<'a>(&'a self, room: &'a ChatRoom) -> impl Iterator<Item = Id> + 'a {
        room.messages(self.owner).filter(|message| !self.is_read(message)).map(|message| message.id)
    }
}
