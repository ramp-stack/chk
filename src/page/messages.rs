use prism::events::{DynEvent, OnEvent, on_event, OnChange, on_change, WindowEvent};
use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::{Context, Callback};
use ptsd::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::components::{MessageGroups, SendEvent, TextInputEvent};
use pelican_ui::navigation::{NavigationEvent, NavigationChanged};
use crate::messages::{ChatRoom, Message, ChatRoomAction, MessageContent, RoomReadState};
use crate::profiles::Profile;
use crate::blocking::Block;
use crate::{Page, PageType, Bumper, ActionItem, ValueItem, PageBuilder};
use prism::hardware::Data;
use futures::{FutureExt, future::{LocalBoxFuture, Shared}};
use std::{cell::Cell, rc::Rc, sync::Arc};
use crate::flow::Flow;
use crate::items::{Action, Display};


use prism::air::Instance;
use crate::typing_ui::{TypingPanel, TypingPublisher};


#[derive(Debug, Component, Clone)]
pub struct MessagesPage {
    layout: Stack,
    page: PelicanPage,
    #[skip] room: Instance<ChatRoom>,
    #[skip] is_blocked: bool,
    #[skip] reads: Instance<RoomReadState>,
    #[skip] displayed: Vec<prism::air::Id>,
    #[skip] acknowledge: bool,
    #[skip] focused: bool,
    #[skip] typing: TypingPublisher,
    #[skip] theme: Theme,
}

#[on_change(self.typing.next())]
impl OnChange<()> for MessagesPage {
    fn on_change(&mut self, _: &mut Context, _: ()) {self.typing.stop();}
}

#[on_event]
impl OnEvent<TextInputEvent> for MessagesPage {
    fn on(&mut self, ctx: &mut Context, event: TextInputEvent) -> Vec<Box<dyn DynEvent>> {
        if let Some(bar) = self.page.bumper.as_mut().and_then(|bumper| bumper.message_bar()) {
            let field = bar.field();
            match &event {
                TextInputEvent::Changed {id, value} if *id == field.id() => {
                    if self.focused && !self.is_blocked && field.has_focus() {
                        self.typing.edit(ctx, &self.room, !value.is_empty());
                    } else {self.typing.stop();}
                }
                TextInputEvent::Submit {id} if *id == field.id() => self.typing.stop(),
                _ => {}
            }
        }
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<ptsd::events::TextInput> for MessagesPage {
    fn on(&mut self, _: &mut Context, event: ptsd::events::TextInput) -> Vec<Box<dyn DynEvent>> {
        if matches!(event, ptsd::events::TextInput::Focused(false)) {self.typing.stop();}
        prism::events![event]
    }
}

// Only the current page is polled. Constructing a flow for a list row must
// never mark its messages read; hidden and background pages do not acknowledge.
#[on_change(async {
    if !self.acknowledge {std::future::pending::<()>().await;}
})]
impl OnChange<()> for MessagesPage {
    fn on_change(&mut self, ctx: &mut Context, _: ()) {
        self.acknowledge = false;
        if !self.room.pending().members.contains(&ctx.air.me()) {return;}
        let unread = {
            let reads = self.reads.pending();
            self.displayed.iter().copied().filter(|id| !reads.read_flag(*id)).collect::<Vec<_>>()
        };
        if !unread.is_empty() {self.reads.send(unread);}
    }
}

#[on_event]
impl OnEvent<WindowEvent> for MessagesPage {
    fn on(&mut self, _: &mut Context, event: WindowEvent) -> Vec<Box<dyn DynEvent>> {
        if let WindowEvent::Focused(focused) = event {
            self.focused = focused;
            self.acknowledge = focused;
            if !focused {self.typing.stop();}
        }
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<NavigationChanged> for MessagesPage {
    fn on(&mut self, _: &mut Context, event: NavigationChanged) -> Vec<Box<dyn DynEvent>> {
        self.focused = true;
        self.acknowledge = true;
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<pelican_ui::interface::message_bar::MessageBarEvent> for MessagesPage {
    fn on(&mut self, ctx: &mut Context, event: pelican_ui::interface::message_bar::MessageBarEvent) -> Vec<Box<dyn DynEvent>> {
        if event == pelican_ui::interface::message_bar::MessageBarEvent::Capture {
            if self.is_blocked || !self.room.pending().members.contains(&ctx.air.me()) {return vec![];}
            self.typing.stop();
            let page = PageType::camera(|ctx: &mut Context, photo| {
                ctx.emit(pelican_ui::components::AttachmentEvent(SendEvent::Photo(photo)));
            });
            (Flow::new(vec![Page::Static(Box::new(page))]).build(ctx, &self.theme))(ctx, &self.theme);
            return vec![];
        }
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<SendEvent> for MessagesPage {
    fn on(&mut self, ctx: &mut Context, event: SendEvent) -> Vec<Box<dyn DynEvent>> {
        self.typing.stop();
        if self.is_blocked || !self.room.pending().members.contains(&ctx.air.me()) {return vec![];}
        match &event {
            SendEvent::Sticker(sticker) => {self.room.send(ChatRoomAction::SendSticker(sticker.clone()));},
            SendEvent::Gif(gif) => {self.room.send(ChatRoomAction::SendGif(gif.clone()));},
            SendEvent::Photo(photo) => {self.room.send(ChatRoomAction::SendPhoto(photo.clone()));},
            SendEvent::Video(video) => {
                let members = self.room.pending().members.clone();
                let video = video.share(ctx, &members);
                self.room.send(ChatRoomAction::SendVideo(video));
            }
        }
        prism::events![event]
    }
}
impl AppPage for MessagesPage {}

impl MessagesPage {
    pub fn new(ctx: &mut Context, theme: &Theme, room: Instance<ChatRoom>, flow_len: usize ) -> Self {
        let room_data = room.pending().clone();
        let my_name = ctx.air.me();
        let is_group = room_data.is_group();
        let has_left = !room_data.members.contains(&my_name);
        let reads = RoomReadState::open(ctx, room.id());
        let displayed = room_data.messages(my_name).filter(|message| message.author != my_name)
            .map(|message| message.id).collect();

        let mut profiles = room_data.members.clone().into_iter()
            .map(|n| Profile::from_name(ctx, n))
            .filter(|p| p.pending().name != Some(my_name))
            .collect::<Vec<_>>();

        let did_i_block = if !is_group && profiles.len() == 1 {
            let profile = profiles[0].pending();
            Block::did_i_block(ctx, profile.name.unwrap())
                .then_some(profile.username.to_string())
        } else {None};

        let am_i_blocked = if !is_group && profiles.len() == 1 {
            let profile = profiles[0].pending();
            Block::am_i_blocked(ctx, profile.name.unwrap())
                .then_some(profile.username.to_string())
        } else {None};

        let is_blocked = has_left || did_i_block.is_some() || am_i_blocked.is_some();

        let deref_profiles = profiles.iter_mut()
            .map(|p| (*p.pending()).clone())
            .collect::<Vec<Profile>>();

        let pel_profiles = deref_profiles.iter()
            .map(|p| p.to_pel()).collect::<Vec<_>>();

        let info_room = room.clone();
        let info = match (is_group, profiles.first().cloned()) {
            (false, None) => None,
            (false, Some(profile)) => {
                Some(Box::new(move |ctx: &mut Context, theme: &Theme| {
                    let mut profile = profile.clone();
                    (Flow::new(vec![Page::profile(ctx, &mut profile)]).build(ctx, theme))(ctx, theme);
                }) as Box<dyn Callback>)
            }
            (true, _) => Some(Box::new(move |ctx: &mut Context, theme: &Theme| {
                (Flow::new(vec![
                    Page::group_info(info_room.clone())
                ]).build(ctx, theme))(ctx, theme);
            }) as Box<dyn Callback>),
        };

        let title = room_data.name(ctx);
        let mut header = Header::messaging(theme, title.clone(), pel_profiles, flow_len, info);
        if is_group {
            use pelican_ui::components::{avatar::{Avatar, AvatarSize}, text::{Text, TextSize, TextStyle}};
            use prism::drawable::text::Align;
            header.center = Box::new(ptsd::display::Bin(ptsd::layout::Column::center(8.0), (
                Avatar::new(theme, room_data.group_details().photo, None, AvatarSize::Sm, None),
                Text::new(theme, &title, TextSize::H4, TextStyle::Heading, Align::Center, Some(1)),
            )));
        }

        let mut room_taken = room.clone();

        let bumper = if has_left {PelicanBumper::alert(theme, "You left this group.")} else {match (did_i_block, am_i_blocked) {
            (None, None) => PelicanBumper::input(theme,
                "Message...",
                move |_ctx: &mut Context, val: &mut String| {
                    if !val.is_empty() {
                        room_taken.send(ChatRoomAction::SendMessage(val.to_string()));
                    }
                },
                move |_ctx: &mut Context, val: &mut String| val.is_empty()
            ),
            (_, Some(username)) => PelicanBumper::alert(theme, &format!("{} blocked you.", username)),
            (Some(username), _) => PelicanBumper::alert(theme, &format!("You blocked {}.", username))
        }};

        let messages = room_data.messages(my_name).map(|message| {
            let room = room.clone();
            let tapped = message.clone();
            message.to_pel(ctx, move |ctx: &mut Context, theme: &Theme| {
                let page = match &tapped.body {
                    MessageContent::Photo(photo) => Page::Static(Box::new(PageType::photo(photo.clone(), Some(MessageDetails::icon(room.clone(), tapped.clone()))))),
                    MessageContent::Video(video) => Page::Static(Box::new(PageType::video(video.clone(), Some(MessageDetails::icon(room.clone(), tapped.clone()))))),
                    _ => MessageDetails::new(room.clone(), tapped.clone()),
                };
                (Flow::new(vec![page]).build(ctx, theme))(ctx, theme);
            })
        }).collect::<Vec<_>>();

        let mut children: Vec<Box<dyn Drawable>> = vec![Box::new(MessageGroups::new(ctx, theme, messages, is_group, false))];
        if !is_blocked {children.push(Box::new(TypingPanel::new(ctx, theme, room.clone())));}
        let content = Content::new(
            Offset::End,
            children,
            Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        );

        let page = PelicanPage::new(header, content, Some(bumper));

        MessagesPage {
            layout: Stack::default(), page, room, is_blocked, reads, displayed,
            acknowledge: true, focused: true, typing: TypingPublisher::default(), theme: theme.clone(),
        }
    }

    pub fn take_input_from(&mut self, old: &mut Self) {
        self.focused = old.focused;
        self.acknowledge &= self.focused;
        if self.is_blocked {old.typing.stop();}
        else {self.typing = std::mem::take(&mut old.typing);}
        if !self.is_blocked && !old.is_blocked && let Some(bumper) = old.page.bumper.take() {
            self.page.bumper = Some(bumper);
        }
    }
}

pub struct MessageDetails;
impl MessageDetails {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(room: Instance<ChatRoom>, message: Message) -> Page {
        Self::view(room, message, 1)
    }

    pub fn icon(room: Instance<ChatRoom>, message: Message) -> (Icons, Action) {
        (Icons::Info, Action::deferred_flow(move |_: &mut Context, _: &Theme| {
            Flow::new(vec![Self::view(room.clone(), message.clone(), 2)])
        }))
    }

    pub fn view(room: Instance<ChatRoom>, message: Message, pops: usize) -> Page {
        let (sender, receiver) = async_channel::bounded(1);
        let busy = Rc::new(Cell::new(false));
        Page::Refreshing(Box::new(MessageSave {
            room, message, pops, sender, receiver, pending: None, result: None, busy,
        }))
    }

    pub fn page(ctx: &mut Context, theme: &Theme, mut room: Instance<ChatRoom>, message: Message,
        pops: usize, save: Option<ActionItem>) -> PageType {
        let is_mine = message.author == ctx.air.me();
        let bubble = message.to_pel(ctx, |_: &mut Context, _: &Theme| {});

        let m = message.clone();
        let delete = ActionItem::new(Action::custom(move |ctx: &mut Context, _: &Theme| {
            room.send(ChatRoomAction::DeleteMessage(m.clone()));
            (0..pops).for_each(|_| ctx.emit(NavigationEvent::Pop));
        }), "Delete", Icons::Delete);
        let timestamp = pelican_ui::utils::Timestamp::from_u64(message.timestamp);
        let mut actions = vec![delete];
        if let Some(copy) = Self::copy_action(ctx, &message.body) {actions.push(copy);}
        if let Some(save) = save {actions.push(save);}

        let title = match is_mine {
            true => "Sent message",
            false => "Received message",
        };

        PageType::display(title, vec![
            Display::messages(ctx, theme, vec![bubble]),
            Display::table("Info", vec![
                ValueItem::new("Time sent", &timestamp.time()),
                ValueItem::new("Date sent", &timestamp.date()),
            ], Some(actions)),
        ], None, Bumper::None, Offset::Start)
    }

    pub fn save_action(content: &MessageContent, sender: async_channel::Sender<SaveRequest>, busy: Rc<Cell<bool>>) -> Option<ActionItem> {
        if matches!(content, MessageContent::Text(_) | MessageContent::System(_)) {return None;}
        let content = content.clone();
        Some(ActionItem::new(Action::custom(move |ctx: &mut Context, theme: &Theme| {
            if busy.replace(true) {return;}
            let request: SaveRequest = match &content {
                MessageContent::Photo(photo) => Self::export(Arc::from(photo.jpeg()), "jpg").boxed_local(),
                MessageContent::Sticker(sticker) => {
                    let image = sticker.image();
                    async move {Ok(Data::Image(image))}.boxed_local()
                }
                MessageContent::Video(video) => {
                    let bytes = crate::messages::VideoClip::find(ctx, video.clip);
                    async move {
                        let bytes = bytes.ok_or("This video is still downloading. Try again when it finishes.")?;
                        Self::export(bytes, "mp4").await
                    }.boxed_local()
                }
                MessageContent::Gif(gif) => {
                    let (gifs, gif) = (theme.gifs().clone(), gif.clone());
                    async move {
                        let bytes = gifs.bytes(&gif).await?;
                        let extension = Self::image_extension(&bytes)
                            .or_else(|| (bytes.get(4..8) == Some(b"ftyp")).then_some("mp4"))
                            .ok_or("This GIF's format cannot be saved to Photos.")?;
                        Self::export(bytes, extension).await
                    }.boxed_local()
                }
                _ => {busy.set(false); return;}
            };
            if sender.try_send(request).is_err() {busy.set(false);}
        }), "Share", Icons::Send))
    }

    pub fn image_extension(bytes: &[u8]) -> Option<&'static str> {
        if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {Some("gif")}
        else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {Some("png")}
        else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {Some("jpg")}
        else {None}
    }

    async fn export(bytes: Arc<[u8]>, extension: &'static str) -> Result<Data, String> {
        pelican_ui::Task::spawn(move || {
            use std::io::Write;
            // Keep the original encoding, including GIF animation and video audio.
            // The system owns temporary-directory cleanup; the share sheet may
            // still be reading the file after the detail page has been closed.
            let path = std::env::temp_dir().join(format!("message-{}.{}", uuid::Uuid::new_v4(), extension));
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)
                .map_err(|_| "Could not prepare this media for saving.".to_string())?;
            file.write_all(&bytes).map_err(|_| "Could not prepare this media for saving.".to_string())?;
            Ok(Data::File(path))
        }).next().await.ok_or_else(|| "Could not prepare this media for saving.".to_string())?
    }

    pub fn copy_action(ctx: &mut Context, content: &MessageContent) -> Option<ActionItem> {
        let action = match content {
            MessageContent::Text(text) => Action::copy(text),
            MessageContent::Gif(gif) => Action::copy(&gif.url),
            MessageContent::Video(_) => return None,
            MessageContent::Photo(photo) => {
                let photo = photo.clone();
                Action::custom(move |ctx: &mut Context, _: &Theme| {
                    ctx.hardware.set_clipboard(&prism::hardware::Data::Image(photo.image()));
                })
            }
            MessageContent::Sticker(sticker) => {
                let sticker = sticker.clone();
                Action::custom(move |ctx: &mut Context, _: &Theme| {
                    sticker.copy(ctx);
                })
            }
            MessageContent::System(notice) => {
                let profile = Profile::from_name(ctx, notice.member());
                let text = notice.text_for(ctx.air.me(), &profile.pending().username);
                Action::copy(&text)
            }
        };
        Some(ActionItem::new_with_active(action, "Copy", "copied", Icons::Copy))
    }
}

type SaveRequest = LocalBoxFuture<'static, Result<Data, String>>;
#[derive(Clone)]
struct MessageSave {
    room: Instance<ChatRoom>,
    message: Message,
    pops: usize,
    sender: async_channel::Sender<SaveRequest>,
    receiver: async_channel::Receiver<SaveRequest>,
    pending: Option<Shared<SaveRequest>>,
    result: Option<Result<Data, String>>,
    busy: Rc<Cell<bool>>,
}

impl std::fmt::Debug for MessageSave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageSave").field("busy", &self.busy.get()).finish()
    }
}

impl PageBuilder for MessageSave {
    fn next(&mut self) -> LocalBoxFuture<'_, Option<()>> {
        async {
            if self.pending.is_none() {self.pending = Some(self.receiver.recv().await.ok()?.shared());}
            self.result = Some(self.pending.as_mut()?.await);
            self.pending = None;
            Some(())
        }.boxed_local()
    }
    fn build(&mut self, ctx: &mut Context, theme: &Theme) -> PageType {
        let error = match self.result.take() {
            Some(Ok(data)) => {self.busy.set(false); ctx.hardware.share(&data); None}
            Some(Err(error)) => {self.busy.set(false); Some(error)}
            None => None,
        };
        let save = MessageDetails::save_action(&self.message.body, self.sender.clone(), self.busy.clone());
        let mut page = MessageDetails::page(ctx, theme, self.room.clone(), self.message.clone(), self.pops, save);
        if let (Some(error), PageType::Content {items, ..}) = (error, &mut page) {
            items.push(crate::Item::Display(Display::instructions(&error)));
        }
        page
    }
}
