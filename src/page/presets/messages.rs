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
use crate::{Page, PageType, Bumper};
use crate::flow::Flow;
use crate::items::{Action, Display};
use crate::closure::FlowBuilder;

use prism::air::Instance;
use super::typing::{TypingPanel, TypingPublisher};
use super::groups::GroupMessageInfoPage;

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
                    GroupMessageInfoPage::page(info_room.clone())
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
                    MessageContent::Photo(photo) => PageType::photo(photo.clone(), Some(MessageDetails::icon(room.clone(), tapped.clone()))),
                    MessageContent::Video(video) => PageType::video(video.clone(), Some(MessageDetails::icon(room.clone(), tapped.clone()))),
                    _ => MessageDetails::new(ctx, theme, room.clone(), tapped.clone()),
                };
                (Flow::new(vec![Page::Static(Box::new(page))]).build(ctx, theme))(ctx, theme);
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
            acknowledge: true, focused: true, typing: TypingPublisher::default(),
        }
    }

    pub fn take_input_from(&mut self, old: &mut Self) {
        self.focused = old.focused;
        self.acknowledge &= self.focused;
        if self.is_blocked {old.typing.stop();}
        else {self.typing = std::mem::take(&mut old.typing);}
        if !self.is_blocked && !old.is_blocked {
            if let Some(bumper) = old.page.bumper.take() {
                self.page.bumper = Some(bumper);
            }
        }
    }
}

pub struct MessageDetails;
impl MessageDetails {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(ctx: &mut Context, theme: &Theme, room: Instance<ChatRoom>, message: Message) -> PageType {
        Self::page(ctx, theme, room, message, 1)
    }

    pub fn icon(room: Instance<ChatRoom>, message: Message) -> (Icons, Box<dyn FlowBuilder>) {
        (Icons::Info, Box::new(move |ctx: &mut Context, theme: &Theme| {
            Flow::new(vec![Page::Static(Box::new(Self::page(
                ctx, theme, room.clone(), message.clone(), 2,
            )))])
        }))
    }

    fn page(ctx: &mut Context, theme: &Theme, mut room: Instance<ChatRoom>, message: Message, pops: usize) -> PageType {
        let is_mine = message.author == ctx.air.me();
        let bubble = message.to_pel(ctx, |_: &mut Context, _: &Theme| {});

        let m = message.clone();
        let bumper = Bumper::custom("Delete Message", Action::custom(move |ctx: &mut Context, _: &Theme| {
            room.send(ChatRoomAction::DeleteMessage(m.clone()));
            (0..pops).for_each(|_| ctx.emit(NavigationEvent::Pop));
        }));

        let title = match is_mine {
            true => "Sent message",
            false => "Received message",
        };

        PageType::display(title, vec![
            Display::messages(ctx, theme, vec![bubble]),
        ], None, bumper, Offset::Start)
    }
}
