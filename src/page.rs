use prism::air::{Contract, Id, Instance};
use prism::Context;
use ptsd::navigation::AppPage;

use std::{fmt::Debug, future::Future, pin::Pin};

use crate::blocking::Block;
use crate::closure::{NavFn, PageBuilderContractFn, PageBuilderContractMultiplesFn};
use crate::design_system::Theme;
pub use crate::form::{Review, Success};
use crate::messages::{ChatRoom, ContactNotificationState, Message, RoomNotificationState};
pub use crate::page_type::{Bumper, DeferredSubmit, FormHost, PageType, Root, SearchSource};
use crate::profiles::Profile;
use crate::Listener;

mod stack;
pub use stack::*;
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

    pub fn contract_updates<C: Contract>(contract: Instance<C>, builder: impl PageBuilderContractFn<C> + 'static) -> Self {
        let builder: Box<dyn PageBuilderContractFn<C>> = Box::new(builder);
        Page::Refreshing(Box::new(Watching::new(ContractWatch::new(contract), builder)))
    }

    pub fn updates_list_changes<C: Contract>(_ctx: &mut Context, builder: impl PageBuilderContractMultiplesFn<C> + 'static) -> Self {
        let builder: Box<dyn PageBuilderContractMultiplesFn<C>> = Box::new(builder);
        Page::Refreshing(Box::new(Watching::new(ListWatch::<C>::default(), builder)))
    }

    pub fn profile(ctx: &mut Context, profile: &mut Instance<Profile>) -> Self {
        let my_name = profile.pending().name.unwrap();
        let is_me = my_name == ctx.air.me();
        match is_me {
            true => Page::Static(Box::new(PageType::editing(
                is_me, profile.clone(), ctx.window.is_mobile, None,
            ))),
            false => Page::Refreshing(Box::new(Watching::new(
                (ContractWatch::new(profile.clone()), ListWatch::<Block>::default(),
                 ListWatch::<ChatRoom>::default(),
                 (ListWatch::<RoomNotificationState>::default(),
                  ListWatch::<ContactNotificationState>::default())),
                |ctx: &mut Context, _: &Theme, (profile, _, _, _): (Instance<Profile>, _, _, _)| {
                    PageType::profile_view(ctx, profile)
                },
            ))),
        }
    }

    pub fn messaging(_ctx: &mut Context, room: &mut Instance<ChatRoom>) -> Self {
        Page::Refreshing(Box::new(Watching::new(
            (ContractWatch::new(room.clone()), ListWatch::<Block>::default(),
             ListWatch::<Profile>::default()),
            |_: &mut Context, _: &Theme, (room, _, _): (Instance<ChatRoom>, _, _)| {
                PageType::messaging(room)
            },
        )))
    }

    pub fn builder(&self) -> Option<Box<dyn PageBuilder>> {
        match self { Page::Refreshing(p) => Some(p.clone()), _ => None }
    }

    pub fn page_type(&self) -> Option<PageType> {
        match self {Page::Static(p) => Some(p.as_ref().clone()), _ => None}
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

    pub fn chat_info(room: Instance<ChatRoom>) -> Page {
        Page::Refreshing(Box::new(Watching::new(
            (ContractWatch::new(room), ListWatch::<Profile>::default(),
             ListWatch::<Block>::default(),
             (ListWatch::<RoomNotificationState>::default(),
              ListWatch::<ContactNotificationState>::default())),
            |ctx: &mut Context, _: &Theme, (room, _, _, _): (Instance<ChatRoom>, _, _, _)| {
                PageType::chat_info(ctx, room)
            },
        )))
    }

    pub fn messaging_at(_ctx: &mut Context, room: &mut Instance<ChatRoom>, message: Id) -> Self {
        Page::Refreshing(Box::new(crate::Watching::new(
            (crate::ContractWatch::new(room.clone()), crate::ListWatch::<Block>::default(),
             crate::ListWatch::<Profile>::default()),
            move |_: &mut Context, _: &Theme, (room, _, _): (Instance<ChatRoom>, _, _)| {
                PageType::Messaging {room, flow_len: 1, target: Some(message)}
            },
        )))
    }

    pub fn message_details(room: Instance<ChatRoom>, message: Message, pops: usize) -> Self {
        Page::Static(Box::new(PageType::MessageDetails {room, message, pops}))
    }
}
