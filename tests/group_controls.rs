use chk::{AvatarContent, messages::{ChatRoom, ChatRoomAction, Message, MessageContent}};
use prism::air::{Contract, Id, Metadata, Name};
use serde::{Serialize, Deserialize};

fn names() -> [Name; 3] {
    [Name::orange_me(),
        "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".parse().unwrap(),
        "0379be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".parse().unwrap()]
}
fn meta(signer: Name) -> Metadata {Metadata {signer, timestamp: 123, confirmed: true}}
fn group() -> ChatRoom {
    let [a, b, c] = names();
    let mut room = ChatRoom::init(Id::from(47), meta(a));
    // Exercise older groups too: no explicit CreateGroup action.
    for member in [b, c] {room.apply(ChatRoomAction::Share(member), meta(a));}
    room
}

#[test]
fn any_current_member_can_edit_and_leaving_cannot_turn_a_group_into_a_dm() {
    let [a, b, c] = names();
    let mut room = group();
    room.apply(ChatRoomAction::RenameGroup("  Friends  ".into()), meta(b));
    let photo = AvatarContent::default();
    room.apply(ChatRoomAction::SetGroupPhoto(photo.clone()), meta(c));
    assert_eq!(room.group_details().name, "Friends");
    assert_eq!(room.group_details().photo, photo);
    room.apply(ChatRoomAction::RenameGroup("   ".into()), meta(a));
    room.apply(ChatRoomAction::RenameGroup("x".repeat(81)), meta(a));
    assert_eq!(room.group_details().name, "Friends");
    room.apply(ChatRoomAction::Leave, meta(c));
    assert!(room.is_group());
    assert_eq!(room.members, vec![a, b]);
    let left = room.clone();
    for action in [ChatRoomAction::RenameGroup("Hijacked".into()),
        ChatRoomAction::SetGroupPhoto(AvatarContent::default()), ChatRoomAction::Share(c),
        ChatRoomAction::SendMessage("Not a member".into()), ChatRoomAction::Leave] {
        room.apply(action, meta(c));
        assert_eq!(room, left);
    }
    room.apply(ChatRoomAction::Share(c), meta(b));
    room.apply(ChatRoomAction::Share(c), meta(b));
    assert_eq!(room.members, vec![a, b, c]);
    room.apply(ChatRoomAction::SendMessage("Back again".into()), meta(c));
    assert_eq!(room.messages(a).count(), 1);
}

#[test]
fn dms_reject_group_controls_and_outsiders_cannot_join_or_write() {
    let [a, b, c] = names();
    let mut room = ChatRoom::init(Id::from(5), meta(a));
    room.apply(ChatRoomAction::Share(b), meta(a));
    let before = room.clone();
    for action in [ChatRoomAction::RenameGroup("Oops".into()), ChatRoomAction::CreateGroup,
        ChatRoomAction::SetGroupPhoto(AvatarContent::default()), ChatRoomAction::Leave] {
        room.apply(action, meta(b));
        assert_eq!(room, before);
    }
    room.apply(ChatRoomAction::Share(c), meta(c));
    room.apply(ChatRoomAction::SendMessage("Intruder".into()), meta(c));
    assert_eq!(room, before);
}

#[derive(Clone, Serialize, Deserialize)]
struct LegacyRoom {members: Vec<Name>, messages: Vec<Message>, id: Id}

#[test]
fn nested_old_caches_and_empty_group_caches_round_trip_without_consuming_neighbor_fields() {
    let [a, b, c] = names();
    let old = LegacyRoom {members: vec![a, b, c], messages: vec![Message {
        body: MessageContent::Text("History".into()), author: a, timestamp: 10,
        hidden_from: vec![], id: Id::from(7),
    }], id: Id::from(6)};
    // Cached AIR instances contain both confirmed and pending snapshots, plus
    // trailing notification state. A defaulted trailing field would corrupt these.
    let bytes = postcard::to_allocvec(&(Some(old.clone()), Some(old), 99usize, true)).unwrap();
    let (confirmed, pending, count, enabled): (Option<ChatRoom>, Option<ChatRoom>, usize, bool) = postcard::from_bytes(&bytes).unwrap();
    assert_eq!((count, enabled), (99, true));
    assert_eq!(confirmed, pending);
    let mut room = confirmed.unwrap();
    assert!(room.is_group());
    assert_eq!(room.messages(a).count(), 1);
    room.apply(ChatRoomAction::RenameGroup("Saved group".into()), meta(b));
    for member in [a, b, c] {room.apply(ChatRoomAction::Leave, meta(member));}
    assert!(room.is_group());
    assert!(room.members.is_empty());
    let value = (Some(room.clone()), Some(room.clone()), 17usize, false);
    let bytes = postcard::to_allocvec(&value).unwrap();
    let restored: (Option<ChatRoom>, Option<ChatRoom>, usize, bool) = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(restored, value);
    assert_eq!(restored.0.unwrap().group_details().name, "Saved group");
}

#[test]
fn old_signed_actions_keep_their_bytes_and_hashes() {
    #[derive(Serialize, std::hash::Hash)]
    #[allow(dead_code)] // Keep every old discriminant in its original position.
    enum LegacyAction {
        Share(Name), SendMessage(String), SendSticker(chk::messages::Sticker),
        SendGif(chk::messages::Gif), SendPhoto(chk::messages::Photo),
        SendVideo(chk::messages::Video), DeleteMessage(Message),
    }
    let a = names()[0];
    let old = LegacyAction::SendMessage("hello".into());
    let new = ChatRoomAction::SendMessage("hello".into());
    assert_eq!(postcard::to_allocvec(&old).unwrap(), postcard::to_allocvec(&new).unwrap());
    assert_eq!(Id::hash(&old), Id::hash(&new));
    assert_eq!(Id::hash(&LegacyAction::Share(a)), Id::hash(&ChatRoomAction::Share(a)));
}

#[test]
fn membership_and_notice_are_atomic_shared_and_duplicates_do_not_add_messages() {
    use chk::messages::SystemMessage;
    let [a, b, c] = names();
    let mut room = ChatRoom::init(Id::from(100), meta(a));
    room.apply(ChatRoomAction::CreateGroup, meta(a));
    room.apply(ChatRoomAction::AddMember(b), meta(a));
    room.apply(ChatRoomAction::AddMember(c), meta(b));
    assert_eq!(room.members, vec![a, b, c]);
    let messages = room.messages(a).cloned().collect::<Vec<_>>();
    assert_eq!(messages.len(), 2);
    assert_ne!(messages[0].id, messages[1].id, "notices need unique IDs even with matching timestamps");
    assert!(matches!(messages[1].body, MessageContent::System(SystemMessage::MemberAdded(n)) if n == c));
    assert_eq!(messages[1].author, b);
    room.apply(ChatRoomAction::AddMember(c), meta(a));
    room.apply(ChatRoomAction::LeaveGroup, meta(c));
    room.apply(ChatRoomAction::LeaveGroup, meta(c));
    room.apply(ChatRoomAction::AddMember(c), meta(c));
    let left = room.messages(a).last().unwrap().clone();
    assert!(matches!(left.body, MessageContent::System(SystemMessage::MemberLeft(n)) if n == c));
    assert_eq!(room.members, vec![a, b]);
    assert_eq!(room.messages(a).count(), 3);
    room.apply(ChatRoomAction::DeleteMessage(left.clone()), meta(c));
    room.apply(ChatRoomAction::DeleteMessage(left), meta(a));
    assert_eq!(room.messages(a).count(), 3);
    room.apply(ChatRoomAction::AddMember(c), meta(b));
    assert_eq!(room.messages(a).count(), 4);
    for reader in [a, b, c] {assert_eq!(room.messages(reader).count(), 4);}
    let restored: ChatRoom = postcard::from_bytes(&postcard::to_allocvec(&room).unwrap()).unwrap();
    assert_eq!(restored, room);
}

#[test]
fn group_name_and_photo_match_for_existing_and_new_members() {
    let [a, b, c] = names();
    let photo = AvatarContent::image(media::Image::new((600, 400), vec![media::Pixel {
        red: 255, green: 32, blue: 64, alpha: 255,
    }; 600 * 400]));
    let actions = vec![
        (a, ChatRoomAction::CreateGroup),
        (a, ChatRoomAction::AddMember(b)),
        (b, ChatRoomAction::RenameGroup("Friends".into())),
        (b, ChatRoomAction::SetGroupPhoto(photo.clone())),
        (a, ChatRoomAction::AddMember(c)),
    ];
    let mut first = ChatRoom::init(Id::from(101), meta(a));
    for (signer, action) in &actions[..4] {first.apply(action.clone(), meta(*signer));}
    let mut cached: ChatRoom = postcard::from_bytes(&postcard::to_allocvec(&first).unwrap()).unwrap();
    first.apply(actions[4].1.clone(), meta(a));
    cached.apply(actions[4].1.clone(), meta(a));
    let mut newcomer = ChatRoom::init(Id::from(101), meta(a));
    for (signer, action) in actions {
        // A new device receives serialized actions, not the sender's in-memory clones.
        let received = postcard::from_bytes(&postcard::to_allocvec(&action).unwrap()).unwrap();
        newcomer.apply(received, meta(signer));
    }
    assert_eq!(first, cached);
    assert_eq!(first, newcomer);
    assert_eq!(newcomer.group_details().name, "Friends");
    assert_eq!(newcomer.group_details().photo, photo);
}

#[test]
fn group_photo_action_keeps_its_signature_hash_after_transmission() {
    for (width, height) in [(1, 1), (256, 256), (257, 256), (513, 300), (128, 1024)] {
        let pixels = (0..width * height).map(|i| media::Pixel {
            red: (i % 251) as u8, green: (i % 239) as u8, blue: (i % 223) as u8, alpha: (i % 256) as u8,
        }).collect();
        let image = media::Image::new((width, height), pixels);
        let normalized = AvatarContent::image(image.clone());
        let action = ChatRoomAction::SetGroupPhoto(AvatarContent::Image(image));
        let bytes = postcard::to_allocvec(&action).unwrap();
        let received: ChatRoomAction = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(Id::hash(&action), Id::hash(&received),
            "AIR verifies the received action against the sender's original hash: {width}x{height}");
        let ChatRoomAction::SetGroupPhoto(received_photo) = received else {panic!()};
        assert_eq!(normalized, received_photo, "the picker must display the transmitted image");
        let AvatarContent::Image(image) = received_photo else {panic!()};
        assert!(image.dimensions.0 <= 256 && image.dimensions.1 <= 256);
    }
}

#[test]
fn existing_small_avatar_and_icon_signatures_keep_their_original_hashes() {
    use std::hash::{Hash, Hasher};
    struct LegacyHash<'a>(&'a AvatarContent);
    impl Hash for LegacyHash<'_> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            std::mem::discriminant(self.0).hash(state);
            match self.0 {
                AvatarContent::Icon(icon, style) => {icon.hash(state); style.hash(state);}
                AvatarContent::Image(image) => {image.dimensions.hash(state); image.as_ref().hash(state);}
            }
        }
    }
    for avatar in [AvatarContent::default(), AvatarContent::image(media::Image::new((256, 128),
        vec![media::Pixel {red: 12, green: 34, blue: 56, alpha: 78}; 256 * 128]))] {
        assert_eq!(Id::hash(&avatar), Id::hash(&LegacyHash(&avatar)));
    }
}
