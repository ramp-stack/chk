use pelican_ui::Context;
use pelican_ui::theme::Theme;

use prism::air::{Contract, Id, Instance};

use crate::{PageBuilder, PageType};

use std::fmt::Debug;

pub trait Watcher: Debug + Clone + 'static {
    type Output;
    fn poll(&mut self, ctx: &mut Context) -> bool;
    fn get(&mut self, ctx: &mut Context) -> Self::Output;
}

pub struct Watch;
impl Watch {
    pub fn contract<C: Contract + PartialEq>(instance: Instance<C>) -> ContractWatch<C> {
        let snapshot = (*instance.pending()).clone();
        ContractWatch { instance, snapshot }
    }

    pub fn list<C: Contract + PartialEq>() -> ListWatch<C> {
        ListWatch { snapshot: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub struct ContractWatch<C: Contract + PartialEq> {
    instance: Instance<C>,
    snapshot: C,
}

impl<C: Contract + PartialEq> Watcher for ContractWatch<C> {
    type Output = Instance<C>;

    fn poll(&mut self, _ctx: &mut Context) -> bool {
        *self.instance.pending() != self.snapshot
    }

    fn get(&mut self, _ctx: &mut Context) -> Instance<C> {
        self.snapshot = (*self.instance.pending()).clone();
        self.instance.clone()
    }
}

#[derive(Debug, Clone)]
pub struct ListWatch<C: Contract + PartialEq> {
    snapshot: Vec<(Id, C)>,
}

impl<C: Contract + PartialEq> ListWatch<C> {
    fn current(ctx: &mut Context) -> Vec<(Id, Instance<C>)> {
        let mut instances = ctx.air.instances::<C>().list().into_iter().collect::<Vec<_>>();
        instances.sort_by_key(|(id, _)| *id);
        instances
    }

    fn state(instances: &[(Id, Instance<C>)]) -> Vec<(Id, C)> {
        instances.iter().map(|(id, i)| (*id, (*i.pending()).clone())).collect()
    }
}

impl<C: Contract + PartialEq> Watcher for ListWatch<C> {
    type Output = Vec<Instance<C>>;

    fn poll(&mut self, ctx: &mut Context) -> bool {
        Self::state(&Self::current(ctx)) != self.snapshot
    }

    fn get(&mut self, ctx: &mut Context) -> Vec<Instance<C>> {
        let instances = Self::current(ctx);
        self.snapshot = Self::state(&instances);
        instances.into_iter().map(|(_, i)| i).collect()
    }
}

macro_rules! impl_watcher_for_tuple {
    ($first:ident, $($w:ident),+) => {
        impl_watcher_for_tuple!(@impl $first, $($w),+);
        impl_watcher_for_tuple!($($w),+);
    };
    ($last:ident) => {};
    (@impl $($w:ident),+) => {
        impl<$($w: Watcher),+> Watcher for ($($w,)+) {
            type Output = ($($w::Output,)+);

            #[allow(non_snake_case)]
            fn poll(&mut self, ctx: &mut Context) -> bool {
                let ($($w,)+) = self;
                false $(|| $w.poll(ctx))+
            }

            #[allow(non_snake_case)]
            fn get(&mut self, ctx: &mut Context) -> Self::Output {
                let ($($w,)+) = self;
                ($($w.get(ctx),)+)
            }
        }
    };
}

impl_watcher_for_tuple!(A, B, C, D);

#[derive(Clone)]
pub struct Watching<W: Watcher, F> {
    watcher: W,
    builder: F,
}

impl<W: Watcher, F: FnMut(&mut Context, &Theme, W::Output) -> PageType + Clone + 'static> Watching<W, F> {
    pub fn new(watcher: W, builder: F) -> Self {
        Watching { watcher, builder }
    }
}

impl<W: Watcher, F> Debug for Watching<W, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Watching").field(&self.watcher).finish()
    }
}

impl<W: Watcher, F: FnMut(&mut Context, &Theme, W::Output) -> PageType + Clone + 'static> PageBuilder for Watching<W, F> {
    fn poll(&mut self, ctx: &mut Context) -> bool {
        self.watcher.poll(ctx)
    }

    fn build(&mut self, ctx: &mut Context, theme: &Theme) -> PageType {
        let value = self.watcher.get(ctx);
        (self.builder)(ctx, theme, value)
    }
}
