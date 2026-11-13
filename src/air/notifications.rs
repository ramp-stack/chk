use prism::air::{Air, Instance, Instances, Name};
use prism::service::Service;
use super::messages::{ChatRoom, RoomNotificationState};
use std::{future::{Future, poll_fn}, pin::pin, task::Poll};

pub(crate) struct MessageNotifications(Instances<ChatRoom>, Air);
impl Service for MessageNotifications {
    type Init = ();
    type Predicate = Instance<ChatRoom>;

    async fn init(air: Air, _: ()) -> Self {
        let rooms = air.instances::<ChatRoom>();
        for room in rooms.list().into_values() {air.start::<RoomNotifications>(room);}
        Self(rooms, air)
    }
    async fn predicate(&mut self) -> Instance<ChatRoom> {self.0.next().await}
    async fn handle(&mut self, room: Instance<ChatRoom>) {self.1.start::<RoomNotifications>(room);}
}

// Watch membership and private preferences even when the conversation is hidden.
// Leaving disables pushes; rejoining must preserve the owner's mute preference.
struct RoomNotifications {
    room: Instance<ChatRoom>, preferences: Instance<RoomNotificationState>, me: Name, enabled: bool,
}
impl Service for RoomNotifications {
    type Init = Instance<ChatRoom>;
    type Predicate = bool;
    async fn init(air: Air, room: Self::Init) -> Self {
        let me = air.me();
        let preferences = air.instances::<RoomNotificationState>().create(room.id());
        let enabled = room.pending().members.contains(&me) && !preferences.pending().is_muted();
        room.notify(enabled);
        Self {room, preferences, me, enabled}
    }
    async fn predicate(&mut self) -> bool {
        {
            let mut room = pin!(self.room.next());
            let mut preferences = pin!(self.preferences.next());
            poll_fn(|cx| {
                if room.as_mut().poll(cx).is_ready() || preferences.as_mut().poll(cx).is_ready() {Poll::Ready(())}
                else {Poll::Pending}
            }).await;
        }
        self.room.pending().members.contains(&self.me) && !self.preferences.pending().is_muted()
    }
    async fn handle(&mut self, enabled: bool) {
        if enabled != self.enabled {
            self.room.notify(enabled);
            self.enabled = enabled;
        }
    }
}

