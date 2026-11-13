use prism::air::{Air, Instance, Instances};
use prism::service::Service;
use std::{future::{Future, pending, poll_fn}, pin::pin, task::Poll};
use super::messages::{ChatRoom, ContactNotificationState, RoomNotificationState};

pub struct MessageNotifications(Instances<ChatRoom>, Air);
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
    room: Instance<ChatRoom>,
    preferences: Instance<RoomNotificationState>,
    contacts: Instances<ContactNotificationState>,
    contact: Option<Instance<ContactNotificationState>>,
    air: Air,
    enabled: bool,
}
impl RoomNotifications {
    fn refresh_contact(&mut self) {
        let me = self.air.me();
        let contact = self.room.pending().direct_contact(me);
        // Read through the subscribed catalog before choosing the instance, so
        // a preference first received after this room opened cannot be missed.
        self.contact = self.contacts.list().into_values().find(|instance| {
            let preferences = instance.pending();
            preferences.owner == me && Some(preferences.contact) == contact
        });
        drop(self.preferences.pending());
    }
}
impl Service for RoomNotifications {
    type Init = Instance<ChatRoom>;
    type Predicate = bool;
    async fn init(air: Air, room: Self::Init) -> Self {
        let preferences = air.instances::<RoomNotificationState>().create(room.id());
        let contacts = air.instances::<ContactNotificationState>();
        let mut service = Self {room, preferences, contacts, contact: None, air, enabled: false};
        service.refresh_contact();
        // Apply a saved person preference before ever enabling a newly received DM.
        service.enabled = RoomNotificationState::enabled(&service.air, &service.room.clone());
        service.room.notify(service.enabled);
        service
    }
    async fn predicate(&mut self) -> bool {
        {
            let mut room = pin!(self.room.next());
            let mut preferences = pin!(self.preferences.next());
            let mut contacts = pin!(self.contacts.next());
            let mut contact = pin!(async {
                match &mut self.contact {
                    Some(contact) => {contact.next().await;}
                    None => pending::<()>().await,
                }
            });
            poll_fn(|cx| {
                if room.as_mut().poll(cx).is_ready() || preferences.as_mut().poll(cx).is_ready()
                    || contacts.as_mut().poll(cx).is_ready() || contact.as_mut().poll(cx).is_ready() {Poll::Ready(())}
                else {Poll::Pending}
            }).await;
        }
        self.refresh_contact();
        // Keep membership changes after refresh_contact queued on the watched
        // handle, so the next pass can update its contact subscription too.
        RoomNotificationState::enabled(&self.air, &self.room.clone())
    }
    async fn handle(&mut self, enabled: bool) {
        if enabled != self.enabled {
            self.room.notify(enabled);
            self.enabled = enabled;
        }
    }
}

