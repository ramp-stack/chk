use prism::{Context, air::{Air, Contract, Id, Instance, Instances, Metadata, Name}};
use prism::service::Service;
use serde::{Deserialize, Serialize};

use std::{collections::BTreeSet, future::{Future, pending, poll_fn}, pin::pin, task::Poll};

use super::messages::ChatRoom;

/// Private direct-message preference, saved even before a conversation exists.
#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
pub struct ContactNotificationState {
    pub contact: Name,
    pub owner: Name,
    muted: bool,
}

impl Contract for ContactNotificationState {
    type Message = bool;
    type Init = Name;
    type Result = ();

    fn id() -> Id {Id::hash("CHK.ContactNotificationState.v1")}
    fn init(contact: Name, metadata: Metadata) -> Self {
        Self {contact, owner: metadata.signer, muted: false}
    }
    fn apply(&mut self, muted: bool, metadata: Metadata) {
        if metadata.signer == self.owner {self.muted = muted;}
    }
}

impl ContactNotificationState {
    pub fn find(air: &Air, contact: Name) -> Option<Instance<Self>> {
        let owner = air.me();
        air.instances::<Self>().list().into_values().find(|instance| {
            let preferences = instance.pending();
            preferences.contact == contact && preferences.owner == owner
        })
    }

    pub fn is_muted_for(air: &Air, contact: Name) -> bool {
        if let Some(preferences) = ContactNotificationState::find(air, contact) {
            return preferences.pending().muted;
        }
        // Preserve old DM mutes until the owner explicitly sets a person preference.
        let owner = air.me();
        let muted_rooms: BTreeSet<_> = air.instances::<RoomNotificationState>().list().into_values()
            .filter_map(|instance| {
                let preferences = instance.pending();
                (preferences.owner == owner && preferences.muted).then_some(preferences.room)
            }).collect();
        !muted_rooms.is_empty() && air.instances::<ChatRoom>().list().into_values().any(|room| {
            muted_rooms.contains(&room.id()) && room.pending().direct_contact(owner) == Some(contact)
        })
    }

    pub fn set_muted(ctx: &Context, contact: Name, muted: bool) {
        let mut preferences = ContactNotificationState::find(&ctx.air, contact)
            .unwrap_or_else(|| ctx.air.instances::<Self>().create(contact));
        if preferences.pending().muted != muted {preferences.send(muted);}
        // Never share this preference or create a room just to apply it.
        let me = ctx.air.me();
        for room in ctx.air.instances::<ChatRoom>().list().into_values() {
            if room.pending().direct_contact(me) == Some(contact) {
                RoomNotificationState::apply_to(ctx, &room);
            }
        }
    }
}

/// Private group preferences and legacy DM preferences; never shared with other members.
#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
pub struct RoomNotificationState {
    pub room: Id,
    pub owner: Name,
    muted: bool,
}

impl Contract for RoomNotificationState {
    type Message = bool;
    type Init = Id;
    type Result = ();

    fn id() -> Id {Id::hash("CHK.RoomNotificationState.v1")}
    fn init(room: Id, metadata: Metadata) -> Self {
        Self {room, owner: metadata.signer, muted: false}
    }
    fn apply(&mut self, muted: bool, metadata: Metadata) {
        if metadata.signer == self.owner {self.muted = muted;}
    }
}

impl RoomNotificationState {
    pub fn find(air: &Air, room: Id) -> Option<Instance<Self>> {
        let owner = air.me();
        air.instances::<Self>().list().into_values().find(|instance| {
            let preferences = instance.pending();
            preferences.room == room && preferences.owner == owner
        })
    }

    pub fn open(ctx: &Context, room: Id) -> Instance<Self> {
        // Creating an existing instance still notifies list watchers. Keep reads
        // quiet so rebuilding chat info cannot trigger another rebuild itself.
        RoomNotificationState::find(&ctx.air, room)
            .unwrap_or_else(|| ctx.air.instances::<RoomNotificationState>().create(room))
    }

    pub fn is_muted(&self) -> bool {self.muted}

    pub fn is_muted_for(air: &Air, room: &Instance<ChatRoom>) -> bool {
        match room.pending().direct_contact(air.me()) {
            Some(contact) => ContactNotificationState::is_muted_for(air, contact),
            None => RoomNotificationState::find(air, room.id())
                .is_some_and(|preferences| preferences.pending().muted),
        }
    }

    pub fn enabled(air: &Air, room: &Instance<ChatRoom>) -> bool {
        room.pending().members.contains(&air.me()) && !RoomNotificationState::is_muted_for(air, room)
    }

    pub fn set_muted(ctx: &Context, room: &Instance<ChatRoom>, muted: bool) {
        if let Some(contact) = room.pending().direct_contact(ctx.air.me()) {
            ContactNotificationState::set_muted(ctx, contact, muted);
            return;
        }
        let mut preferences = RoomNotificationState::open(ctx, room.id());
        if preferences.pending().is_muted() != muted {preferences.send(muted);}
        RoomNotificationState::apply_to(ctx, room);
    }

    pub fn apply_to(ctx: &Context, room: &Instance<ChatRoom>) {
        room.notify(RoomNotificationState::enabled(&ctx.air, room));
    }
}

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
    pub fn refresh_contact(&mut self) {
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
    async fn init(air: Air, room: Instance<ChatRoom>) -> Self {
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
                    || contacts.as_mut().poll(cx).is_ready() || contact.as_mut().poll(cx).is_ready() {
                    Poll::Ready(())
                } else {Poll::Pending}
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
