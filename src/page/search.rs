use prism::air::{Id, Instance, Name};
use prism::events::{DynEvent, ListenCallback};
use prism::Context;
use ptsd::layout::Offset;
use ptsd::navigation::NavigationChanged;
use uuid::Uuid;

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::design_system::{AvatarContent, Header, PageCallbacks, PageView, TextInputEvent, Theme};
use crate::items::{Action, Display, Input, Item, ListItem};
use crate::messages::{ChatRoom, RoomReadState};
use crate::profiles::Profile;
use crate::{Bumper, ContactInvitations, Flow, ListWatch, NavFn, Page, PageType, State, Watcher};

type MessageLists = (Vec<Instance<ChatRoom>>, Vec<Instance<Profile>>, Vec<Instance<RoomReadState>>);

#[derive(Debug, Clone)]
pub enum SearchSource {
    Contacts(ListWatch<Profile>),
    Messages((ListWatch<ChatRoom>, ListWatch<Profile>, ListWatch<RoomReadState>)),
}
impl SearchSource {
    pub fn label(&self) -> &'static str {
        match self {SearchSource::Contacts(_) => "Contacts", SearchSource::Messages(_) => "Messages"}
    }

    pub fn contact_items(invitations: &ContactInvitations, ctx: &mut Context, profiles: Vec<Instance<Profile>>) -> Vec<ListItem> {
        SearchEntry::contacts(invitations, ctx, profiles).into_iter()
            .map(|entry| entry.item).collect()
    }

    pub fn message_items(
        invitations: &ContactInvitations, ctx: &mut Context, rooms: Vec<Instance<ChatRoom>>,
        profiles: Vec<Instance<Profile>>, reads: Vec<Instance<RoomReadState>>,
    ) -> Vec<ListItem> {
        SearchEntry::messages(invitations, ctx, rooms, profiles, reads).into_iter()
            .map(|entry| entry.item).collect()
    }

    pub fn page(self, invitations: &ContactInvitations) -> PageType {
        let input_id = Uuid::new_v4();
        let label = self.label().to_lowercase();
        let input = Item::Input(Input::Text {
            id: input_id, label: self.label().into(), actions: None, show_label: false,
            preset: None, placeholder: format!("Search {label}..."),
        });
        let invitations = invitations.clone();
        let lifecycle = Box::new(move |_: &mut Context, theme: &Theme, _: Option<NavFn>, _: usize| -> PageCallbacks {
            let source = self.clone();
            let contacts = matches!(&self, SearchSource::Contacts(_));
            let entries = Rc::new(RefCell::new((Vec::new(), Vec::new())));
            let current = entries.clone();
            let style = theme.clone();
            let live_invitations = invitations.clone();
            let on_create: ListenCallback<PageView> = Box::new(move |page, ctx| {
                let invitations = live_invitations;
                page.watch(ctx, source, move |ctx, page, (rooms, profiles, reads)| {
                    let mut entries = current.borrow_mut();
                    *entries = if contacts {
                        (SearchEntry::contacts(&invitations, ctx, profiles), vec![])
                    } else {
                        let history = SearchMessage::history(ctx, &rooms, &profiles);
                        (SearchEntry::messages(&invitations, ctx, rooms, profiles, reads), history)
                    };
                    let query = page.content().children().first()
                        .and_then(|input| Input::state(&**input));
                    let query = match &query {Some(State::Text(query)) => query.as_str(), _ => ""};
                    let results = SearchEntry::filter(
                        &invitations, &entries.0, &entries.1, contacts, query,
                    );
                    let children = page.content_mut().children_mut();
                    children.truncate(1);
                    children.extend(results.into_iter().flat_map(|item| item.build(ctx, &style)));
                });
            });
            let event_theme = theme.clone();
            let mut focus_on_open = true;
            let mut query = String::new();
            let invitations = invitations.clone();
            let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
                if let Some(TextInputEvent::Changed {id, value} | TextInputEvent::Set {id, value}) =
                    event.downcast_ref() && *id == input_id && *value != query {
                    query.clone_from(value);
                    let entries = entries.borrow();
                    let results = SearchEntry::filter(
                        &invitations, &entries.0, &entries.1, contacts, &query,
                    );
                    let children = page.content_mut().children_mut();
                    children.truncate(1);
                    children.extend(results.into_iter().flat_map(|item| item.build(ctx, &event_theme)));
                    page.content_mut().children.set_scroll(0.0);
                }
                if event.downcast_ref::<NavigationChanged>().is_some() && focus_on_open {
                    focus_on_open = false;
                    ctx.emit(TextInputEvent::Focus {id: input_id});
                }
                false
            });
            (Some(on_event), None, Some(on_create))
        });
        PageType::Content {
            header: Header::stack(&format!("Search {label}"), None), items: vec![input],
            bumper: Bumper::None, offset: Offset::Start, next: None, flow_len: 1,
            lifecycle: Some(lifecycle), identity: None, reveal_child: None,
        }
    }
}
impl Watcher for SearchSource {
    type Output = MessageLists;

    async fn next(&mut self) -> Option<()> {
        match self {
            SearchSource::Contacts(watch) => watch.next().await,
            SearchSource::Messages(watch) => watch.next().await,
        }
    }
    fn get(&mut self, ctx: &mut Context) -> MessageLists {
        match self {
            SearchSource::Contacts(watch) => (vec![], watch.get(ctx), vec![]),
            SearchSource::Messages(watch) => watch.get(ctx),
        }
    }
}

#[derive(Debug, Clone)]
struct SearchEntry {
    item: ListItem,
    text: String,
}
impl SearchEntry {
    pub fn filter(invitations: &ContactInvitations, entries: &[Self], history: &[SearchMessage], contacts: bool, query: &str) -> Vec<Display> {
        let query = query.to_lowercase();
        let terms = query.split_whitespace().collect::<Vec<_>>();
        let items: Vec<_> = entries.iter()
            .filter(|entry| terms.iter().all(|term| entry.text.contains(term)))
            .map(|entry| entry.item.clone()).collect();
        let empty = match (contacts, terms.is_empty()) {
            (true, true) => "No contacts yet.", (true, false) => "No matching contacts.",
            (false, true) => "No messages yet.",
            (false, false) => "No matching conversations or messages.",
        };
        if contacts || terms.is_empty() {return vec![Display::list(None, items, Some(empty))];}
        let messages: Vec<_> = history.iter()
            .filter(|entry| terms.iter().all(|term| entry.text.contains(term)))
            .map(|entry| entry.item(invitations, &terms)).collect();
        if items.is_empty() && messages.is_empty() {
            return vec![Display::list(None, vec![], Some(empty))];
        }
        let mut sections = Vec::new();
        if !items.is_empty() {sections.push(Display::list(Some("Conversations"), items, None));}
        if !messages.is_empty() {sections.push(Display::list(Some("Messages"), messages, None));}
        sections
    }

    pub fn contacts(invitations: &ContactInvitations, ctx: &mut Context, profiles: Vec<Instance<Profile>>) -> Vec<SearchEntry> {
        let me = ctx.air.me();
        let mut entries = profiles.into_iter().filter_map(|mut instance| {
            let name = instance.pending().name?;
            if name == me {return None;}
            let view = Flow::new(vec![Page::profile(invitations, ctx, &mut instance)]);
            let profile = instance.pending();
            Some((name, SearchEntry {
                text: format!("{} {}", profile.username, name).to_lowercase(),
                item: ListItem::avatar(profile.avatar.clone(), &profile.username,
                    &profile.name(), None, Some(Action::flow(view)), false, None),
            }))
        }).collect::<Vec<_>>();
        entries.sort_by_cached_key(|(name, entry)| (entry.item.title.to_lowercase(), *name));
        entries.into_iter().map(|(_, entry)| entry).collect()
    }

    pub fn messages(
        invitations: &ContactInvitations, ctx: &mut Context, rooms: Vec<Instance<ChatRoom>>,
        profiles: Vec<Instance<Profile>>, reads: Vec<Instance<RoomReadState>>,
    ) -> Vec<SearchEntry> {
        let me = ctx.air.me();
        let profiles = profiles.into_iter().filter_map(|instance| {
            let profile = instance.pending();
            Some((profile.name?, profile.username.clone()))
        }).collect::<HashMap<Name, String>>();
        let reads = reads.into_iter().filter_map(|instance| {
            let read = instance.pending();
            (read.owner == me).then(|| (read.room, read.clone()))
        }).collect::<HashMap<_, _>>();
        let mut entries = rooms.into_iter().filter(|room| room.pending().members.contains(&me))
            .map(|instance| {
                let id = instance.id();
                let chat = Flow::new(vec![Page::messaging(invitations, instance.clone(), None)]);
                let room = instance.pending();
                let members = room.members.iter().copied()
                    .filter(|name| *name != me).collect::<Vec<_>>();
                let name = room.name(ctx);
                let avatar = if room.is_group() {
                    room.group_details().photo
                } else {
                    let profile = Profile::from_name(ctx, members.first().copied().unwrap_or(me));
                    profile.pending().avatar.clone()
                };
                let (timestamp, last) = match room.messages(me).last() {
                    Some(last) => (last.timestamp, last.preview(ctx, room.is_group())),
                    None => (0, "No messages yet.".into()),
                };
                let unread = room.messages(me).any(|message| message.author != me
                    && reads.get(&id).is_none_or(|read| !read.is_read(message)));
                let mut text = format!("{name} {last}");
                for member in &room.members {
                    text.push_str(&format!(" {member}"));
                    if let Some(username) = profiles.get(member) {
                        text.push_str(&format!(" {username}"));
                    }
                }
                (timestamp, id, SearchEntry {
                    text: text.to_lowercase(),
                    item: ListItem::avatar(avatar, &name, &last, None, Some(Action::flow(chat)), unread, None),
                })
        }).collect::<Vec<_>>();
        entries.sort_by_key(|(timestamp, id, _)| (std::cmp::Reverse(*timestamp), *id));
        entries.into_iter().map(|(_, _, entry)| entry).collect()
    }
}

#[derive(Debug, Clone)]
struct SearchMessage {
    room: Rc<Instance<ChatRoom>>,
    id: Id,
    timestamp: u64,
    avatar: AvatarContent,
    title: String,
    preview: String,
    text: String,
}

impl SearchMessage {
    pub fn history(ctx: &mut Context, rooms: &[Instance<ChatRoom>], profiles: &[Instance<Profile>]) -> Vec<Self> {
        let me = ctx.air.me();
        let profiles = profiles.iter().filter_map(|instance| {
            let profile = instance.pending();
            Some((profile.name?, profile.username.clone()))
        }).collect::<HashMap<_, _>>();
        let mut entries = Vec::new();
        for instance in rooms {
            let room = instance.pending();
            if !room.members.contains(&me) {continue;}
            let title = room.name(ctx);
            let avatar = if room.is_group() {room.group_details().photo} else {
                let name = room.direct_contact(me).unwrap_or(me);
                Profile::from_name(ctx, name).pending().avatar.clone()
            };
            let instance = Rc::new(instance.clone());
            for message in room.messages(me) {
                let preview = message.preview_with(me, room.is_group(), |name| {
                    profiles.get(&name).cloned().unwrap_or_else(|| name.to_string())
                });
                entries.push(SearchMessage {
                    room: instance.clone(), id: message.id, timestamp: message.timestamp,
                    avatar: avatar.clone(), title: title.clone(),
                    text: preview.to_lowercase(), preview,
                });
            }
        }
        entries.sort_by_cached_key(|entry| {
            (std::cmp::Reverse(entry.timestamp), entry.room.id(), entry.id)
        });
        entries
    }

    pub fn excerpt(&self, terms: &[&str]) -> String {
        let text = self.preview.split_whitespace().collect::<Vec<_>>().join(" ");
        let lowercase = text.to_lowercase();
        let matched = terms.iter().filter_map(|term| lowercase.find(term)).min().unwrap_or(0);
        // Lowercasing can expand a Unicode character, so translate the match
        // back to a character offset before cutting the original text.
        let mut bytes = 0;
        let index = text.chars().take_while(|character| {
            bytes += character.to_lowercase().map(char::len_utf8).sum::<usize>();
            bytes <= matched
        }).count();
        let characters = text.chars().collect::<Vec<_>>();
        let start = index.saturating_sub(16);
        let end = (start + 100).min(characters.len());
        format!("{}{}{}", if start > 0 {"…"} else {""},
            characters[start..end].iter().collect::<String>(),
            if end < characters.len() {"…"} else {""})
    }

    pub fn item(&self, invitations: &ContactInvitations, terms: &[&str]) -> ListItem {
        let timestamp = ptsd::utils::Timestamp::from_u64(self.timestamp);
        let subtitle = format!("{} · {}\n{}",
            timestamp.date(), timestamp.time(), self.excerpt(terms));
        let room = self.room.clone();
        let id = self.id;
        let invitations = invitations.clone();
        let action = Action::custom(move |ctx: &mut Context, theme: &Theme| {
            // A result can become stale between its last refresh and the tap.
            let visible = {
                let data = room.pending();
                let me = ctx.air.me();
                if !data.members.contains(&me) {return;}
                data.messages(me).any(|message| message.id == id)
            };
            if visible {
                let page = Page::messaging(&invitations, room.as_ref().clone(), Some(id));
                let mut flow = Flow::new(vec![page]);
                (flow.build(ctx, theme))(ctx, theme);
            }
        });
        ListItem::avatar(self.avatar.clone(), &self.title, &subtitle, None, Some(action), false, None)
    }
}
