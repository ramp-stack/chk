use std::time::{Duration, Instant};
use futures::{FutureExt, select};
use pelican_ui::{Context, theme::Theme, components::TypingIndicator};
use prism::{air::{Instance, Name}, drawable::Component, events::{Timer, OnChange, on_change}};
use ptsd::layout::Column;
use crate::{Watch, Watcher, ListWatch, messages::ChatRoom, profiles::Profile, blocking::Block};
use crate::air::typing::{TypingState, TypingAction};

#[derive(Clone, Debug, Default)]
pub(super) struct TypingPublisher {
    instance: Option<Instance<TypingState>>,
    activity: Activity,
    idle: Timer,
}

#[derive(Clone, Debug, Default)]
struct Activity {last_sent: Option<Instant>, active: bool}
impl Activity {
    fn edit(&mut self, now: Instant, nonempty: bool) -> Option<bool> {
        if !nonempty {return self.stop();}
        if !self.active || self.last_sent.is_none_or(|sent| now.duration_since(sent) >= Duration::from_secs(2)) {
            self.active = true;
            self.last_sent = Some(now);
            Some(true)
        } else {None}
    }
    fn stop(&mut self) -> Option<bool> {
        std::mem::take(&mut self.active).then_some(false)
    }
}

impl TypingPublisher {
    pub fn edit(&mut self, ctx: &mut Context, room: &Instance<ChatRoom>, nonempty: bool) {
        if !nonempty || !room.pending().members.contains(&ctx.air.me()) {self.stop(); return;}
        self.idle.reset(Duration::from_secs(3));
        if let Some(active) = self.activity.edit(Instant::now(), true) {
            let instance = self.instance.get_or_insert_with(|| {
                let instance = ctx.air.instances::<TypingState>().create(room.id());
                instance.notify(false);
                instance
            });
            let members = room.pending().members.clone();
            for member in members {
                if member == ctx.air.me() || Block::did_i_block(ctx, member) || Block::am_i_blocked(ctx, member) {continue;}
                if !instance.pending().shared_with.contains(&member) {
                    instance.share(member);
                    instance.send(TypingAction::Share(member));
                }
            }
            instance.send(TypingAction::Activity {active, at: TypingState::now()});
        }
    }

    pub fn stop(&mut self) {
        self.idle.cancel();
        if let Some(active) = self.activity.stop() && let Some(instance) = &mut self.instance {
            instance.send(TypingAction::Activity {active, at: TypingState::now()});
        }
    }

    pub async fn next(&mut self) {self.idle.next().await;}
}

#[derive(Clone, Debug)]
struct Feed {
    watch: ListWatch<TypingState>,
    expiry: Timer,
}
impl Feed {
    async fn next(&mut self) {
        select! {
            _ = self.watch.next().fuse() => {},
            _ = self.expiry.next().fuse() => {},
        }
    }
}

#[derive(Clone, Debug, Component)]
pub(super) struct TypingPanel {
    layout: Column,
    indicators: Vec<TypingIndicator>,
    #[skip] feed: Feed,
    #[skip] room: Instance<ChatRoom>,
    #[skip] theme: Theme,
    #[skip] shown: Vec<(Name, String)>,
}
impl TypingPanel {
    pub fn new(ctx: &mut Context, theme: &Theme, room: Instance<ChatRoom>) -> Self {
        let mut panel = Self {
            layout: Column::start(12.0), indicators: vec![],
            feed: Feed {watch: Watch::list(), expiry: Timer::default()},
            room, theme: theme.clone(), shown: vec![],
        };
        panel.refresh(ctx);
        panel
    }

    fn refresh(&mut self, ctx: &mut Context) {
        let room = self.room.pending();
        let me = ctx.air.me();
        let group = room.is_group();
        let now = TypingState::now();
        let mut shown = self.feed.watch.get(ctx).into_iter().filter_map(|instance| {
            let state = instance.pending();
            if state.room != self.room.id() || state.author == me || !room.members.contains(&state.author)
                || Block::did_i_block(ctx, state.author) || Block::am_i_blocked(ctx, state.author) {return None;}
            // AIR message timestamps are nanoseconds; typing timestamps are milliseconds.
            let sent = room.messages(me).filter(|message| message.author == state.author
                && !matches!(message.body, crate::messages::MessageContent::System(_)))
                .map(|message| message.timestamp / 1_000_000).max();
            state.is_typing(now, sent).then(|| {
                (state.author, Profile::from_name(ctx, state.author).pending().username.clone())
            })
        }).collect::<Vec<_>>();
        shown.sort_by_key(|(name, _)| *name);
        shown.dedup_by_key(|(name, _)| *name);
        if shown != self.shown {
            self.indicators = shown.iter().map(|(_, name)| TypingIndicator::new(&self.theme, group.then_some(name.as_str()))).collect();
            self.shown = shown;
        }
        if self.shown.is_empty() {self.feed.expiry.cancel();}
        else {self.feed.expiry.reset(Duration::from_millis(250));}
    }
}

#[on_change(self.feed.next())]
impl OnChange<()> for TypingPanel {
    fn on_change(&mut self, ctx: &mut Context, _: ()) {self.refresh(ctx);}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activity_throttles_edits_and_stops_only_once() {
        let mut activity = Activity::default();
        let now = Instant::now();
        assert_eq!(activity.edit(now, true), Some(true));
        for millis in [1, 100, 500, 1999] {
            assert_eq!(activity.edit(now + Duration::from_millis(millis), true), None);
        }
        assert_eq!(activity.edit(now + Duration::from_secs(2), true), Some(true));
        assert_eq!(activity.edit(now + Duration::from_millis(2001), false), Some(false));
        assert_eq!(activity.stop(), None);
        assert_eq!(activity.edit(now + Duration::from_millis(2002), true), Some(true));
    }
}
