use prism::events::{DynEvent, OnEvent, on_event};
use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::{Context, Callback};
use ptsd::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::components::{MessageGroups, SendEvent};
use pelican_ui::navigation::NavigationEvent;
use crate::messages::{ChatRoom, Message, ChatRoomAction, MessageContent};
use crate::profiles::Profile;
use crate::blocking::Block;
use crate::{Page, PageType, Bumper};
use crate::flow::Flow;
use crate::items::{Action, Display};
use crate::closure::FlowBuilder;

use prism::air::Instance;

#[derive(Debug, Component, Clone)]
pub struct MessagesPage {
    layout: Stack,
    page: PelicanPage,
    #[skip] room: Instance<ChatRoom>,
    #[skip] is_blocked: bool,
}

#[on_event]
impl OnEvent<SendEvent> for MessagesPage {
    fn on(&mut self, ctx: &mut Context, event: SendEvent) -> Vec<Box<dyn DynEvent>> {
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

        let mut profiles = room_data.members.clone().into_iter()
            .map(|n| Profile::from_name(ctx, n))
            .filter(|p| p.pending().name != Some(my_name))
            .collect::<Vec<_>>();

        let did_i_block = if profiles.len() == 1 {
            let profile = profiles[0].pending();
            Block::did_i_block(ctx, profile.name.unwrap())
                .then_some(profile.username.to_string())
        } else {None};

        let am_i_blocked = if profiles.len() == 1 {
            let profile = profiles[0].pending();
            Block::am_i_blocked(ctx, profile.name.unwrap())
                .then_some(profile.username.to_string())
        } else {None};

        let is_blocked = did_i_block.is_some() || am_i_blocked.is_some();

        let deref_profiles = profiles.iter_mut()
            .map(|p| (*p.pending()).clone())
            .collect::<Vec<Profile>>();

        let is_group = deref_profiles.len() > 1;

        let pel_profiles = deref_profiles.iter()
            .map(|p| p.to_pel()).collect::<Vec<_>>();

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
                    Page::Static(GroupMessageInfoPage::new(ctx, profiles.clone()))
                ]).build(ctx, theme))(ctx, theme);
            }) as Box<dyn Callback>),
        };

        let title = room_data.name(ctx);
        let header = Header::messaging(&ctx.hardware, theme, title, pel_profiles, flow_len, info);

        let mut room_taken = room.clone();

        let bumper = match (did_i_block, am_i_blocked) {
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
        };

        let messages = room_data.messages(my_name).map(|message| {
            let room = room.clone();
            let tapped = message.clone();
            message.to_pel(ctx, move |ctx: &mut Context, theme: &Theme| {
                let page = match &tapped.body {
                    MessageContent::Photo(photo) => PageType::photo(photo.clone(), Some(MessageDetails::icon(room.clone(), tapped.clone()))),
                    MessageContent::Video(video) => PageType::video(video.clone(), Some(MessageDetails::icon(room.clone(), tapped.clone()))),
                    _ => MessageDetails::new(ctx, theme, room.clone(), tapped.clone()),
                };
                (Flow::new(vec![Page::Static(page)]).build(ctx, theme))(ctx, theme);
            })
        }).collect::<Vec<_>>();

        let content = Content::new(
            Offset::End,
            vec![Box::new(MessageGroups::new(ctx, theme, messages.clone(), is_group, false))],
            Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        );

        let page = PelicanPage::new(header, content, Some(bumper));

        MessagesPage { layout: Stack::default(), page, room, is_blocked }
    }

    pub fn take_input_from(&mut self, old: &mut Self) {
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
            Flow::new(vec![Page::Static(Self::page(ctx, theme, room.clone(), message.clone(), 2))])
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

        PageType::display(&title, vec![
            Display::messages(ctx, theme, vec![bubble]),
        ], None, bumper, Offset::Start)
    }
}

pub struct GroupMessageInfoPage;
impl GroupMessageInfoPage {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(ctx: &mut Context, profiles: Vec<Instance<Profile>>) -> PageType {
        let items = profiles.clone().into_iter().flat_map(|profile| {
            let mut p = profile.clone();
            let deref = profile.pending();
            if deref.name.unwrap() != ctx.air.me() {
                let view_contact = Flow::new(vec![Page::profile(ctx, &mut p)]);
                Some(crate::ListItem::avatar(deref.avatar.clone(), &deref.username, &deref.name(), None, Some(view_contact)))
            } else {None}
        }).collect::<Vec<crate::ListItem>>();
        PageType::display("Group members", vec![
            Display::instructions(&format!("This group has {} members.", profiles.len())),
            Display::list(None, items, None),
        ], None, Bumper::None, Offset::Start)
    }
}
