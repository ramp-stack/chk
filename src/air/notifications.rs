use prism::air::{Air, Instance, Instances, Name};
use prism::service::Service;
use super::messages::ChatRoom;

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

// Watch membership even when the conversation is hidden or this device receives
// a leave/rejoin from another device. Only change the push subscription on a transition.
struct RoomNotifications {room: Instance<ChatRoom>, me: Name, enabled: bool}
impl Service for RoomNotifications {
    type Init = Instance<ChatRoom>;
    type Predicate = bool;
    async fn init(air: Air, room: Self::Init) -> Self {
        let me = air.me();
        let enabled = room.pending().members.contains(&me);
        room.notify(enabled);
        Self {room, me, enabled}
    }
    async fn predicate(&mut self) -> bool {
        self.room.next().await;
        self.room.pending().members.contains(&self.me)
    }
    async fn handle(&mut self, enabled: bool) {
        if enabled != self.enabled {
            self.room.notify(enabled);
            self.enabled = enabled;
        }
    }
}

