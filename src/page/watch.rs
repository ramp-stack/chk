use futures::FutureExt;
use futures::future::{LocalBoxFuture, Shared, select_all};
use pelican_ui::{Context, theme::Theme};
use prism::air::{Contract, Id, Instance, Instances};

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::future::Future;

use crate::{PageBuilder, PageType};

#[derive(Debug, Clone)]
pub struct ContractWatch<C: Contract>(Instance<C>);

#[derive(Debug)]
pub struct ListWatch<C: Contract> {
    instances: Option<Instances<C>>,
    current: BTreeMap<Id, Instance<C>>,
}
impl<C: Contract> Clone for ListWatch<C> {
    fn clone(&self) -> Self {
        let instances = self.instances.clone();
        let current = instances.as_ref().map(|instances| instances.list().into_iter().collect())
            .unwrap_or_default();
        Self {instances, current}
    }
}

pub trait Watcher: Debug + Clone + 'static {
    type Output;
    fn next(&mut self) -> impl Future<Output = Option<()>>;
    fn get(&mut self, ctx: &mut Context) -> Self::Output;
}
impl<C: Contract> Watcher for ContractWatch<C> {
    type Output = Instance<C>;
    async fn next(&mut self) -> Option<()> {self.0.next().await; Some(())}
    fn get(&mut self, _ctx: &mut Context) -> Instance<C> {
        drop(self.0.pending());
        self.0.clone()
    }
}
impl<C: Contract> Watcher for ListWatch<C> {
    type Output = Vec<Instance<C>>;
    async fn next(&mut self) -> Option<()> {
        let instances = self.instances.as_mut()?;
        let added = {
            let mut waits: Vec<LocalBoxFuture<'_, Option<Instance<C>>>> = self.current.values_mut()
                .map(|instance| async move {instance.next().await; None}.boxed_local()).collect();
            waits.push(async move {Some(instances.next().await)}.boxed_local());
            select_all(waits).await.0
        };
        if let Some(instance) = added {self.current.entry(instance.id()).or_insert(instance);}
        Some(())
    }
    fn get(&mut self, ctx: &mut Context) -> Vec<Instance<C>> {
        let instances = self.instances.get_or_insert_with(|| ctx.air.instances::<C>()).list();
        self.current.retain(|id, _| instances.contains_key(id));
        for (id, instance) in instances {self.current.entry(id).or_insert(instance);}
        self.current.values().map(|instance| {
            drop(instance.pending());
            instance.clone()
        }).collect()
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
            async fn next(&mut self) -> Option<()> {
                let ($($w,)+) = self;
                let mut waits: Vec<LocalBoxFuture<'_, Option<()>>> = vec![
                    $($w.next().boxed_local(),)+
                ];
                while !waits.is_empty() {
                    let (update, _, remaining) = select_all(waits).await;
                    if update.is_some() {return update}
                    waits = remaining;
                }
                None
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

pub struct Watch;
impl Watch {
    pub fn contract<C: Contract>(instance: Instance<C>) -> ContractWatch<C> {ContractWatch(instance)}
    pub fn list<C: Contract>() -> ListWatch<C> {
        ListWatch {instances: None, current: BTreeMap::new()}
    }
}

pub struct Watching<W: Watcher, F> {
    watcher: W,
    builder: F,
    pending: Option<Shared<LocalBoxFuture<'static, Option<()>>>>,
}
impl<W: Watcher, F> Watching<W, F> {
    pub fn new(watcher: W, builder: F) -> Self {
        Self {watcher, builder, pending: None}
    }
}
impl<W: Watcher, F: Clone> Clone for Watching<W, F> {
    fn clone(&self) -> Self {
        Self {
            watcher: self.watcher.clone(), builder: self.builder.clone(),
            pending: Some(async {Some(())}.boxed_local().shared()),
        }
    }
}
impl<W: Watcher, F> Debug for Watching<W, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Watching").field(&self.watcher).finish()
    }
}
impl<W: Watcher, F: FnMut(&mut Context, &Theme, W::Output) -> PageType + Clone + 'static> PageBuilder for Watching<W, F> {
    fn next(&mut self) -> LocalBoxFuture<'_, Option<()>> {
        Box::pin(async {
            let update = self.pending.as_ref()?.clone().await;
            self.pending = None;
            update
        })
    }
    fn build(&mut self, ctx: &mut Context, theme: &Theme) -> PageType {
        self.pending = None;
        self.watcher.get(ctx);
        let mut watcher = self.watcher.clone();
        // Prime deferred subscriptions before the final snapshot and retain them while hidden.
        let pending = async move {watcher.next().await}.boxed_local().shared();
        let _ = pending.clone().now_or_never();
        self.pending = Some(pending);
        let value = self.watcher.get(ctx);
        (self.builder)(ctx, theme, value)
    }
}
