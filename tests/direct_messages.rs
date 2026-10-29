use std::{any::Any, collections::HashMap, sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}}};
use chk::messages::{ChatRoom, ChatRoomAction};
use prism::{Context, air::{Contract, ContractType, Id, Metadata, Name, Request, Value}, traits};

mod support;

fn names() -> [Name; 3] {
    [Name::orange_me(),
        "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".parse().unwrap(),
        "0379be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".parse().unwrap()]
}

fn metadata(signer: Name) -> Metadata {Metadata {signer, timestamp: 1, confirmed: true}}

#[derive(Clone, Debug)]
struct MemoryAir {
    me: Name,
    rooms: Arc<Mutex<HashMap<Id, MemoryRoom>>>,
    created: Arc<Mutex<Vec<Id>>>,
    shares: Arc<AtomicUsize>,
}

impl MemoryAir {
    fn new() -> (Self, Context) {
        let air = Self {me: names()[0], rooms: Default::default(), created: Default::default(), shares: Default::default()};
        let mut ctx = support::context();
        ctx.air = prism::air::Air::new(air.clone());
        (air, ctx)
    }

    fn seed(&self, author: Name, init: Id, members: &[Name], history: bool) -> Id {
        let mut room = ChatRoom::init(init, metadata(author));
        for name in members {room.apply(ChatRoomAction::Share(*name), metadata(author));}
        if history {room.apply(ChatRoomAction::SendMessage("Existing history".into()), metadata(author));}
        // AIR locations depend on the creator as well as the initialization value.
        let id = Id::hash(&(author, init));
        self.rooms.lock().unwrap().insert(id, MemoryRoom {
            id, me: self.me, room: Arc::new(Mutex::new(room)), shares: self.shares.clone(),
        });
        id
    }
}

impl traits::Air for MemoryAir {
    fn me(&self) -> Name {self.me}
    fn instances(&self, contract: ContractType) -> Box<dyn traits::DynInstances> {
        assert_eq!(contract.id, ChatRoom::id());
        Box::new(self.clone())
    }
    fn start(&self, _: prism::service::ServiceInit) {panic!("unexpected service")}
    fn shutdown(&self, _: prism::service::Key) {panic!("unexpected service")}
}

impl traits::DynInstances for MemoryAir {
    fn create(&self, init: Request) -> Box<dyn traits::DynInstance> {
        let init = *init.downcast::<Id>().unwrap();
        self.created.lock().unwrap().push(init);
        let id = Id::hash(&(self.me, init));
        if !self.rooms.lock().unwrap().contains_key(&id) {self.seed(self.me, init, &[], false);}
        Box::new(self.rooms.lock().unwrap()[&id].clone())
    }
    fn list(&self) -> HashMap<Id, Box<dyn traits::DynInstance>> {
        self.rooms.lock().unwrap().iter().map(|(id, room)| (*id, Box::new(room.clone()) as Box<dyn traits::DynInstance>)).collect()
    }
    fn try_next(&mut self) -> Option<Box<dyn traits::DynInstance>> {None}
    fn next(&mut self) -> traits::BoxFuture<'_, Box<dyn traits::DynInstance>> {Box::pin(std::future::pending())}
}

#[derive(Clone, Debug)]
struct MemoryRoom {id: Id, me: Name, room: Arc<Mutex<ChatRoom>>, shares: Arc<AtomicUsize>}

#[derive(Debug)]
struct Snapshot<T>(T);
impl<T: std::fmt::Debug + Send + Sync + 'static> traits::DynRef for Snapshot<T> {
    fn value(&self) -> &(dyn Any + Send + Sync) {&self.0}
}

impl traits::DynInstance for MemoryRoom {
    fn id(&self) -> Id {self.id}
    fn notify(&self, _: bool) {}
    fn send(&mut self, message: Request) -> Box<dyn traits::DynPending> {
        Box::new(Pending(self.room.lock().unwrap().apply(*message.downcast::<ChatRoomAction>().unwrap(), metadata(self.me))))
    }
    fn share(&self, _: Name) {self.shares.fetch_add(1, Ordering::Relaxed);}
    fn pending(&self) -> Box<dyn traits::DynRef + '_> {Box::new(Snapshot(self.room.lock().unwrap().clone()))}
    fn confirmed(&self) -> Option<Box<dyn traits::DynRef + '_>> {Some(self.pending())}
    fn try_next(&mut self) -> Option<traits::ContractUpdate> {None}
    fn next(&mut self) -> traits::BoxFuture<'_, traits::ContractUpdate> {Box::pin(std::future::pending())}
}

#[derive(Clone, Debug)]
struct Pending(usize);
impl traits::DynPending for Pending {
    fn pending(&self) -> Box<dyn traits::DynRef + '_> {Box::new(Snapshot(self.0))}
    fn try_confirmed(&self) -> Option<Value> {Some(Arc::new(self.0))}
    fn try_next(&mut self) -> Option<bool> {None}
    fn next(&mut self) -> traits::BoxFuture<'_, bool> {Box::pin(std::future::pending())}
}

#[test]
fn new_dms_hash_the_name_and_reopen_without_sharing_again() {
    let (air, mut ctx) = MemoryAir::new();
    let [me, other, _] = names();
    let first = ChatRoom::create(&mut ctx, vec![other]);
    for recipients in [vec![other], vec![me, other], vec![other, other, me]] {
        assert_eq!(ChatRoom::create(&mut ctx, recipients).id(), first.id());
    }
    assert_eq!(*air.created.lock().unwrap(), vec![Id::hash(&other)]);
    assert_eq!(air.shares.load(Ordering::Relaxed), 1);
}

#[test]
fn received_legacy_dm_reopens_its_history_instead_of_creating_a_room() {
    let (air, mut ctx) = MemoryAir::new();
    let [me, other, _] = names();
    let existing = air.seed(other, Id::from(42), &[me], true);
    air.seed(me, Id::hash(&other), &[other], false);
    for _ in 0..10 {
        let room = ChatRoom::create(&mut ctx, vec![other]);
        assert_eq!(room.id(), existing);
        assert_eq!(room.pending().messages(me).count(), 1);
    }
    assert!(air.created.lock().unwrap().is_empty());
    assert_eq!(air.shares.load(Ordering::Relaxed), 0);
}

#[test]
fn groups_always_get_new_ids_and_are_never_reused_as_dms() {
    let (air, mut ctx) = MemoryAir::new();
    let [me, other, third] = names();
    let first = ChatRoom::create(&mut ctx, vec![other, third]);
    let second = ChatRoom::create(&mut ctx, vec![third, me, other, third]);
    assert_ne!(first.id(), second.id());
    assert_ne!(first.pending().id, second.pending().id);
    let dm = ChatRoom::create(&mut ctx, vec![other]);
    assert_ne!(dm.id(), first.id());
    assert_ne!(dm.id(), second.id());
    assert_eq!(dm.pending().id, Id::hash(&other));
    assert_eq!(air.created.lock().unwrap().len(), 3);
}

#[test]
fn self_messages_use_a_stable_id_including_empty_recipient_input() {
    let (air, mut ctx) = MemoryAir::new();
    let first = ChatRoom::create(&mut ctx, vec![air.me]);
    assert_eq!(first.pending().id, Id::hash(&air.me));
    assert_eq!(ChatRoom::create(&mut ctx, vec![]).id(), first.id());
    assert_eq!(air.created.lock().unwrap().len(), 1);
}
