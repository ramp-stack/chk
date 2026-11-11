use crate::{ContactInvitations, Watch, Form, Flow, ListItem};
use crate::messages::{ChatRoom, RoomReadState};
use crate::profiles::Profile;
use prism::air::Instance;
type MessageLists = (Vec<Instance<ChatRoom>>, Vec<Instance<Profile>>, Vec<Instance<RoomReadState>>);
use crate::ActionItem;
use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::{Context};
use ptsd::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};

use crate::{Page, Action, PageType};
use crate::page::EditPage;
use crate::items::{Display, Item};

pub struct Root(pub Page);
impl Root {
    pub fn new(
        title: &str, items: Vec<Display>, header: Option<(Icons, Action)>,
        bumper_a: Option<(String, Action)>,
        bumper_b: Option<(String, Action)>,
    ) -> Self {
        Root(Page::Static(Box::new(PageType::root(title, vec![], items, header, bumper_a, bumper_b))))
    }

    pub fn custom(page: Page) -> Self { Root(page) }

    pub fn contacts(invitations: &ContactInvitations) -> Root {
        let invitations = invitations.clone();
        Root::custom(Page::watching(Watch::list::<Profile>(),
            move |ctx: &mut Context, _theme: &Theme, profiles: Vec<Instance<Profile>>| {
                let me = ctx.air.me();
                let mut items = profiles.into_iter().filter_map(|mut instance| {
                    if instance.pending().name == Some(me) {return None;}
                    let view = Flow::new(vec![Page::profile(ctx, &mut instance)]);
                    let profile = instance.pending();
                    Some(ListItem::avatar(profile.avatar.clone(), &profile.username,
                        &profile.name(), None, Some(Action::flow(view))))
                }).collect::<Vec<_>>();
                items.sort_by(|a, b| a.title.cmp(&b.title));
                let invitations = invitations.clone();
                let new_contact = Action::deferred_flow(move |_ctx: &mut Context, theme: &Theme| {
                    Flow::from_form(Form::new_contact(&invitations, theme))
                });
                PageType::root("Contacts", vec![], vec![Display::list(None, items,
                    Some("No contacts yet.\nGet started by adding a friend."))], None,
                    Some(("New Contact".into(), new_contact)), None)
            }))
    }

    pub fn profile(ctx: &mut Context, invitations: &ContactInvitations) -> Root {
        Root::custom(Page::Static(Box::new(PageType::editing_with_contact_actions(
            true, Profile::me(ctx), ctx.window.is_mobile, Some(ActionItem::contact_actions(invitations, ctx.air.me())),
        ))))
    }

    pub fn messages(ctx: &mut Context, invitations: &ContactInvitations) -> Root {
        ctx.air.start::<crate::air::notifications::MessageNotifications>(());
        let invitations = invitations.clone();
        Root::custom(Page::watching((Watch::list::<ChatRoom>(), Watch::list::<Profile>(), Watch::list::<RoomReadState>()),
            move |ctx: &mut Context, _theme: &Theme, (rooms, _, reads): MessageLists| {
                let me = ctx.air.me();
                let reads = reads.into_iter().filter_map(|instance| {
                    let read = instance.pending();
                    (read.owner == me).then(|| (read.room, read.clone()))
                }).collect::<std::collections::HashMap<_, _>>();
                let mut items = rooms.into_iter().filter(|room| room.pending().members.contains(&me)).map(|mut instance| {
                    let chat = Flow::new(vec![Page::messaging(ctx, &mut instance)]);
                    let room = instance.pending();
                    let members = room.members.iter().copied().filter(|name| *name != me).collect::<Vec<_>>();
                    let name = room.name(ctx);
                    let avatar = if room.is_group() {
                        room.group_details().photo
                    } else {
                        let profile = Profile::from_name(ctx, members.first().copied().unwrap_or(me));
                        profile.pending().avatar.clone()
                    };
                    let (timestamp, last) = match room.messages(me).last() {
                        Some(last) => (last.timestamp, last.preview(ctx, room.is_group())),
                        None => (0, "No messages yet.".into()),
                    };
                    let unread = room.messages(me).any(|message| message.author != me
                        && reads.get(&instance.id()).is_none_or(|read| !read.is_read(message)));
                    (timestamp, ListItem::avatar(avatar, &name, &last, None, Some(Action::flow(chat))).with_badge(unread))
                }).collect::<Vec<_>>();
                items.sort_by_key(|item| std::cmp::Reverse(item.0));
                let items = items.into_iter().map(|(_, item)| item).collect();
                let invitations = invitations.clone();
                let new_message = Action::deferred_flow(move |ctx: &mut Context, theme: &Theme| {
                    Flow::from_form(Form::new_message(&invitations, ctx, theme))
                });
                PageType::root("Messages", vec![], vec![Display::list(None, items,
                    Some("No messages yet.\nGet started by messaging a friend."))], None,
                    Some(("New Message".into(), new_message)), None)
            }))
    }
}

#[derive(Debug, Component, Clone)]
pub struct RootPage(Stack, PelicanPage);

impl AppPage for RootPage {}
impl PageType {
    pub fn build_root(&self, theme: &Theme, is_mobile: bool) -> Box<dyn AppPage> {
        let (title, items, header, mut bumper_a, mut bumper_b) = match self.clone() {
            Self::Root { title, input, display, header, bumper_a, bumper_b } => {
                (title, input.into_iter().map(Item::Input).chain(display.into_iter().map(Item::Display)).collect::<Vec<_>>(), header, bumper_a, bumper_b)
            }
            Self::Content { title, items, header, .. } => {
                (title, items, header, None, None)
            }
            Self::Edit { title, header, items, display, on_save, .. } => {
                return Box::new(EditPage::root(
                    theme, is_mobile, title, header, items, display, on_save,
                ));
            }
            Self::Form { .. } | Self::Review { .. } | Self::Success { .. }
            | Self::Messaging { .. } | Self::Camera { .. } | Self::Photo { .. } | Self::Video { .. } | Self::Playback { .. } => {
                panic!("Not an accepted root type");
            }
        };
        let length = items.len();
        let offset = match items.iter().find_map(|item| match item {Item::Display(display) => Some(display), _ => None}) {
            Some(Display::List { .. }) => Offset::Start,
            _ if length <= 1 => Offset::Center, _ => Offset::Start,
        };

        let header_icon = header.map(|(icon, action)| (icon, action.get()));

        let header = Header::home(theme, &title, header_icon);
        let content = items.into_iter().filter_map(|mut item| item.build(theme, is_mobile))
            .flatten().collect::<Vec<Box<dyn Drawable>>>();

        let second = bumper_b.as_mut().map(|(label, action)| (ActionItem::format_label(label), action.get()));
        let first = bumper_a.as_mut().map(|(label, action)| (ActionItem::format_label(label), action.get()));

        let bumper = match first.is_some() || second.is_some() {
            true => Some(PelicanBumper::home(theme, first, second)), false => None,
        };

        let page = PelicanPage::new(
            header,
            Content::new(
                offset, content,
                Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
            ), bumper,
        );
        Box::new(RootPage(Stack::default(), page))
    }
}
