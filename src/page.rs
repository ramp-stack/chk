use prism::air::{Id, Instance};
use prism::Context;
use prism::drawable::{Component, Drawable};
use prism::events::ListenCallback;
use ptsd::layout::{Stack, Offset};
use ptsd::navigation::AppPage;

use std::{fmt::Debug, future::Future, ops::{Deref, DerefMut}, pin::Pin};

use crate::blocking::Block;
use crate::closure::NavFn;
use crate::design_system::{Content, Footer, Header, PageEvent, PageView, Theme};
pub use crate::form::{DeferredSubmit, FormHost, Review, Success};
use crate::messages::{ChatRoom, ContactNotificationState, Message, RoomNotificationState};
pub use crate::page_type::{Bumper, PageType};
pub use crate::Root;
use crate::profiles::Profile;
use crate::{ContactInvitations, Item, Listener};

mod search;
pub use search::SearchSource;
mod watch;
pub use watch::*;
mod media;
pub use media::*;
mod video;
pub use video::*;

pub trait PageBuilder: Debug + dyn_clone::DynClone {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<()>> + '_>>;
    fn build(&mut self, ctx: &mut Context, theme: &Theme) -> PageType;
}

dyn_clone::clone_trait_object!(PageBuilder);

#[derive(Debug, Clone)]
pub enum Page { Static(Box<PageType>), Refreshing(Box<dyn PageBuilder>) }

impl Page {
    pub fn watching<W: Watcher>(watcher: W, builder: impl FnMut(&mut Context, &Theme, W::Output) -> PageType + Clone + 'static) -> Self {
        Page::Refreshing(Box::new(Watching::new(watcher, builder)))
    }

    pub fn profile(invitations: &ContactInvitations, ctx: &mut Context, profile: &mut Instance<Profile>) -> Self {
        let invitations = invitations.clone();
        let my_name = profile.pending().name.unwrap();
        let is_me = my_name == ctx.air.me();
        match is_me {
            true => Page::Static(Box::new(Profile::editing(
                is_me, profile.clone(), ctx.window.is_mobile, None,
            ))),
            false => Page::Refreshing(Box::new(Watching::new(
                (ContractWatch::new(profile.clone()), ListWatch::<Block>::default(),
                 ListWatch::<ChatRoom>::default(),
                 (ListWatch::<RoomNotificationState>::default(),
                  ListWatch::<ContactNotificationState>::default())),
                move |ctx: &mut Context, _: &Theme, (profile, _, _, _): (Instance<Profile>, _, _, _)| {
                    Profile::profile_view(&invitations, ctx, profile)
                },
            ))),
        }
    }

    pub fn messaging(invitations: &ContactInvitations, room: Instance<ChatRoom>, target: Option<Id>) -> Self {
        let invitations = invitations.clone();
        Page::Refreshing(Box::new(Watching::new(
            (ContractWatch::new(room.clone()), ListWatch::<Block>::default(),
             ListWatch::<Profile>::default()),
            move |ctx: &mut Context, _: &Theme, (room, _, _): (Instance<ChatRoom>, _, _)| {
                ChatRoom::page(&invitations, ctx, room, target)
            },
        )))
    }

    pub fn builder(&self) -> Option<Box<dyn PageBuilder>> {
        match self { Page::Refreshing(p) => Some(p.clone()), _ => None }
    }

    pub fn build(&self, ctx: &mut Context, theme: &Theme, mut next_fn: Option<NavFn>, length: usize) -> Box<dyn AppPage> {
        match self {
            Page::Static(page) => {
                let mut page = page.clone();
                page.update(length, next_fn.take());
                page.build(ctx, theme)
            }
            Page::Refreshing(page_builder) => Box::new(Listener::new(
                ctx, theme, page_builder.clone(), false, next_fn.take(), length,
            )),
        }
    }

    pub fn build_root(&self, ctx: &mut Context, theme: &Theme) -> Box<dyn AppPage> {
        match self {
            Page::Static(page) => page.clone().build_root(ctx, theme),
            Page::Refreshing(page_builder) => {
                Box::new(Listener::new(ctx, theme, page_builder.clone(), true, None, 0))
            }
        }
    }

    pub fn chat_info(invitations: &ContactInvitations, room: Instance<ChatRoom>) -> Page {
        let invitations = invitations.clone();
        Page::Refreshing(Box::new(Watching::new(
            (ContractWatch::new(room), ListWatch::<Profile>::default(),
             ListWatch::<Block>::default(),
             (ListWatch::<RoomNotificationState>::default(),
              ListWatch::<ContactNotificationState>::default())),
            move |ctx: &mut Context, _: &Theme, (room, _, _, _): (Instance<ChatRoom>, _, _, _)| {
                PageType::chat_info(&invitations, ctx, room)
            },
        )))
    }

    pub fn message_details(invitations: &ContactInvitations, ctx: &mut Context, room: Instance<ChatRoom>, message: Message, pops: usize) -> Self {
        Page::Static(Box::new(message.page(invitations, ctx, room, pops)))
    }
}

#[derive(Debug, Component, Clone)]
pub struct StackPage<D: Drawable + Clone = PageView>(Stack, D);

impl<D: Drawable + Clone> AppPage for StackPage<D> {}
impl<D: Drawable + Clone> StackPage<D> {
    pub fn new(content: D) -> Self {StackPage(Stack::default(), content)}
}

impl<D: Drawable + Clone> Deref for StackPage<D> {
    type Target = D;
    fn deref(&self) -> &D {&self.1}
}
impl<D: Drawable + Clone> DerefMut for StackPage<D> {
    fn deref_mut(&mut self) -> &mut D {&mut self.1}
}

impl StackPage<Box<dyn Drawable>> {
    pub fn edge_to_edge(theme: &Theme, header: Header, content: Box<dyn Drawable>) -> Self {
        StackPage(Stack::fill(), theme.edge_to_edge(header, content))
    }
}

impl StackPage<PageView> {
    #[allow(clippy::too_many_arguments, reason = "Page setup is supplied directly at construction.")]
    pub fn content(
        ctx: &mut Context, theme: &Theme, header: Header, items: Vec<Item>, offset: Offset,
        footer: Option<Footer>, identity: Option<String>, reveal_child: Option<usize>,
        on_event: Option<Box<dyn PageEvent>>,
        on_show: Option<Box<dyn ptsd::utils::Callback>>, on_create: Option<ListenCallback<PageView>>,
    ) -> Self {
        let items = items.into_iter().flat_map(|item| item.build(ctx, theme)).collect();
        let mut content = Content::new(offset, items,
            Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true));
        if let Some(index) = reveal_child {content.reveal_child(index);}
        StackPage(Stack::default(), theme.page(
            ctx, header, content, footer, vec![], false, identity, on_event, on_show, on_create,
        ))
    }
}
