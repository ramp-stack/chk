use prism::air::{Id, Instance, Name};
use std::rc::Rc;
use prism::events::{DynEvent, OnEvent, on_event, OnChange, on_change};
use ptsd::navigation::{NavigationEvent, NavigationChanged};
use uuid::Uuid;
use prism::drawable::{Component, Drawable};
use ptsd::layout::{Stack, Offset};
use ptsd::navigation::AppPage;

use crate::ContentSurface;
use crate::{ContactInvitations, Watcher, ListWatch, Form, Flow, ListItem};
use crate::messages::{ChatRoom, RoomReadState};
use crate::profiles::Profile;
use crate::design_system::TextInputEvent;
use crate::ActionItem;
use crate::design_system::{Context};
use crate::design_system::PageView;
use crate::design_system::{AvatarContent, Theme, Icons};
use crate::{Page, Action, PageType};
use crate::page::EditPage;
use crate::items::{Display, Item};
use crate::design_system::{Header, Footer, Content};

type MessageLists = (Vec<Instance<ChatRoom>>, Vec<Instance<Profile>>, Vec<Instance<RoomReadState>>);

pub struct Root(pub Page);
impl Root {
    pub fn new(
        title: &str, items: Vec<Display>, header: Option<(Icons, Action)>,
        bumper_a: Option<(String, Action)>,
        bumper_b: Option<(String, Action)>,
    ) -> Self {
        Root(Page::Static(Box::new(PageType::root(title, vec![], items, header, bumper_a, bumper_b))))
    }

    pub fn custom(page: Page) -> Self { Root(page) }

    pub fn contacts(invitations: &ContactInvitations) -> Root {
        let invitations = invitations.clone();
        Root(Page::watching(ListWatch::<Profile>::default(),
            move |ctx: &mut Context, _theme: &Theme, profiles: Vec<Instance<Profile>>| {
                let items = SearchEntry::contacts(ctx, profiles).into_iter().map(|entry| entry.item).collect();
                let search = Action::custom(|ctx: &mut Context, theme: &Theme| {
                    SearchPage::open(ctx, theme, SearchSource::Contacts(ListWatch::<Profile>::default()));
                });
                let invitations = invitations.clone();
                let new_contact = Action::deferred_flow(move |_ctx: &mut Context, theme: &Theme| {
                    Flow::from_form(Form::new_contact(&invitations, theme))
                });
                PageType::root("Contacts", vec![], vec![Display::list(None, items,
                    Some("No contacts yet.\nGet started by adding a friend."))], Some((Icons::Search, search)),
                    Some(("New Contact".into(), new_contact)), None)
            }))
    }

    pub fn profile(ctx: &mut Context, invitations: &ContactInvitations) -> Root {
        Root(Page::Static(Box::new(PageType::editing(
            true, Profile::me(ctx), ctx.window.is_mobile, Some(ActionItem::contact_actions(invitations, ctx.air.me())),
        ))))
    }

    pub fn messages(ctx: &mut Context, invitations: &ContactInvitations) -> Root {
        ChatRoom::start_notifications(ctx);
        let invitations = invitations.clone();
        Root(Page::watching((
            ListWatch::<ChatRoom>::default(), ListWatch::<Profile>::default(),
            ListWatch::<RoomReadState>::default(),
        ),
            move |ctx: &mut Context, _theme: &Theme, (rooms, profiles, reads): MessageLists| {
                let items = SearchEntry::messages(ctx, rooms, profiles, reads).into_iter().map(|entry| entry.item).collect();
                let search = Action::custom(|ctx: &mut Context, theme: &Theme| {
                    SearchPage::open(ctx, theme, SearchSource::Messages((
                        ListWatch::<ChatRoom>::default(), ListWatch::<Profile>::default(),
                        ListWatch::<RoomReadState>::default(),
                    )));
                });
                let invitations = invitations.clone();
                let new_message = Action::deferred_flow(move |ctx: &mut Context, theme: &Theme| {
                    Flow::from_form(Form::new_message(&invitations, ctx, theme))
                });
                PageType::root("Messages", vec![], vec![Display::list(None, items,
                    Some("No messages yet.\nGet started by messaging a friend."))], Some((Icons::Search, search)),
                    Some(("New Message".into(), new_message)), None)
            }))
    }

}

#[derive(Debug, Component, Clone)]
pub struct RootPage(Stack, PageView);

impl AppPage for RootPage {}
impl PageType {
    pub fn build_root(&self, theme: &Theme, is_mobile: bool) -> Box<dyn AppPage> {
        let (title, items, header, bumper_a, bumper_b) = match self.clone() {
            PageType::Root { title, input, display, header, bumper_a, bumper_b } => {
                (title, input.into_iter().map(Item::Input).chain(display.into_iter().map(Item::Display)).collect::<Vec<_>>(), header, bumper_a, bumper_b)
            }
            PageType::Content { title, items, header, .. } => {
                (title, items, header, None, None)
            }
            PageType::Edit { title, header, items, display, on_save, .. } => {
                return Box::new(EditPage::root(
                    theme, is_mobile, title, header, items, display, on_save,
                ));
            }
            PageType::Form { .. } | PageType::Review { .. } | PageType::Success { .. }
            | PageType::Messaging { .. } | PageType::Camera { .. } | PageType::Photo { .. } | PageType::Video { .. } | PageType::Playback { .. } => {
                panic!("Not an accepted root type");
            }
        };
        let length = items.len();
        let offset = match items.iter().find_map(|item| match item {Item::Display(display) => Some(display), _ => None}) {
            Some(Display::List { .. }) => Offset::Start,
            _ if length <= 1 => Offset::Center, _ => Offset::Start,
        };

        let content = items.into_iter().filter_map(|mut item| item.build(theme, is_mobile))
            .flatten().collect::<Vec<Box<dyn Drawable>>>();
        let action = header.map(|(icon, action)| (icon, action.get()));
        let header = Header::home(&title, action);
        let action = |(label, action): (String, Action)| (ActionItem::format_label(&label), action.get());
        let footer = (bumper_a.is_some() || bumper_b.is_some())
            .then(|| Footer::home(bumper_a.map(action), bumper_b.map(action)));
        let page = theme.page(header, Content::new(offset, content, Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true)), footer);
        Box::new(RootPage(Stack::default(), page))
    }
}

#[derive(Debug, Clone)]
struct SearchEntry {
    item: ListItem,
    text: String,
}
impl SearchEntry {
    pub fn contacts(ctx: &mut Context, profiles: Vec<Instance<Profile>>) -> Vec<SearchEntry> {
        let me = ctx.air.me();
        let mut entries = profiles.into_iter().filter_map(|mut instance| {
            let name = instance.pending().name?;
            if name == me {return None;}
            let view = Flow::new(vec![Page::profile(ctx, &mut instance)]);
            let profile = instance.pending();
            Some((name, SearchEntry {
                text: format!("{} {}", profile.username, name).to_lowercase(),
                item: ListItem::avatar(profile.avatar.clone(), &profile.username,
                    &profile.name(), None, Some(Action::flow(view))),
            }))
        }).collect::<Vec<_>>();
        entries.sort_by_cached_key(|(name, entry)| (entry.item.title.to_lowercase(), *name));
        entries.into_iter().map(|(_, entry)| entry).collect()
    }

    pub fn messages(ctx: &mut Context, rooms: Vec<Instance<ChatRoom>>,
        profiles: Vec<Instance<Profile>>, reads: Vec<Instance<RoomReadState>>) -> Vec<SearchEntry> {
        let me = ctx.air.me();
        let profiles = profiles.into_iter().filter_map(|instance| {
            let profile = instance.pending();
            Some((profile.name?, profile.username.clone()))
        }).collect::<std::collections::HashMap<Name, String>>();
        let reads = reads.into_iter().filter_map(|instance| {
            let read = instance.pending();
            (read.owner == me).then(|| (read.room, read.clone()))
        }).collect::<std::collections::HashMap<_, _>>();
        let mut entries = rooms.into_iter().filter(|room| room.pending().members.contains(&me)).map(|mut instance| {
            let id = instance.id();
            let chat = Flow::new(vec![Page::messaging(ctx, &mut instance)]);
            let room = instance.pending();
            let members = room.members.iter().copied().filter(|name| *name != me).collect::<Vec<_>>();
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
                if let Some(username) = profiles.get(member) {text.push_str(&format!(" {username}"));}
            }
            (timestamp, id, SearchEntry {
                text: text.to_lowercase(),
                item: ListItem::avatar(avatar, &name, &last, None, Some(Action::flow(chat))).with_badge(unread),
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
    fn history(ctx: &mut Context, rooms: &[Instance<ChatRoom>], profiles: &[Instance<Profile>]) -> Vec<Self> {
        let me = ctx.air.me();
        let profiles = profiles.iter().filter_map(|instance| {
            let profile = instance.pending();
            Some((profile.name?, profile.username.clone()))
        }).collect::<std::collections::HashMap<_, _>>();
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
                entries.push(Self {
                    room: instance.clone(), id: message.id, timestamp: message.timestamp,
                    avatar: avatar.clone(), title: title.clone(), text: preview.to_lowercase(), preview,
                });
            }
        }
        entries.sort_by_cached_key(|entry| (std::cmp::Reverse(entry.timestamp), entry.room.id(), entry.id));
        entries
    }

    fn excerpt(&self, terms: &[&str]) -> String {
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

    fn item(&self, terms: &[&str]) -> ListItem {
        let timestamp = ptsd::utils::Timestamp::from_u64(self.timestamp);
        let subtitle = format!("{} · {}\n{}", timestamp.date(), timestamp.time(), self.excerpt(terms));
        let room = self.room.clone();
        let id = self.id;
        let action = Action::custom(move |ctx: &mut Context, theme: &Theme| {
            // A result can become stale between its last refresh and the tap.
            let visible = {
                let data = room.pending();
                let me = ctx.air.me();
                if !data.members.contains(&me) {return;}
                data.messages(me).any(|message| message.id == id)
            };
            if visible {
                let page = Page::messaging_at(ctx, &mut room.as_ref().clone(), id);
                let mut flow = Flow::new(vec![page]);
                (flow.build(ctx, theme))(ctx, theme);
            }
        });
        ListItem::avatar(self.avatar.clone(), &self.title, &subtitle, None, Some(action))
    }
}

#[derive(Debug, Clone)]
enum SearchSource {
    Contacts(ListWatch<Profile>),
    Messages((ListWatch<ChatRoom>, ListWatch<Profile>, ListWatch<RoomReadState>)),
}
impl SearchSource {
    pub fn label(&self) -> &'static str {
        match self {SearchSource::Contacts(_) => "Contacts", SearchSource::Messages(_) => "Messages"}
    }
    pub fn entries(&mut self, ctx: &mut Context) -> (Vec<SearchEntry>, Vec<SearchMessage>) {
        match self {
            SearchSource::Contacts(watch) => {
                let profiles = watch.get(ctx);
                (SearchEntry::contacts(ctx, profiles), vec![])
            }
            SearchSource::Messages(watch) => {
                let (rooms, profiles, reads) = watch.get(ctx);
                let history = SearchMessage::history(ctx, &rooms, &profiles);
                (SearchEntry::messages(ctx, rooms, profiles, reads), history)
            }
        }
    }
    pub async fn next(&mut self) -> Option<()> {
        match self {SearchSource::Contacts(watch) => watch.next().await, SearchSource::Messages(watch) => watch.next().await}
    }
}

#[derive(Debug, Component, Clone)]
struct SearchPage {
    layout: Stack,
    page: PageView,
    #[skip] source: SearchSource,
    #[skip] entries: Vec<SearchEntry>,
    #[skip] messages: Vec<SearchMessage>,
    #[skip] message_limit: usize,
    #[skip] theme: Theme,
    #[skip] input_id: Uuid,
    #[skip] query: String,
    #[skip] focus_on_open: bool,
}
impl AppPage for SearchPage {}
impl SearchPage {
    pub fn open(ctx: &mut Context, theme: &Theme, source: SearchSource) {
        let page = SearchPage::new(ctx, theme, source);
        ctx.emit(NavigationEvent::push(crate::FlowWrapper::new(
            ptsd::navigation::Flow::new(vec![Box::new(page)]),
        )));
    }

    pub fn new(ctx: &mut Context, theme: &Theme, mut source: SearchSource) -> Self {
        let input_id = Uuid::new_v4();
        let label = source.label();
        let (entries, messages) = source.entries(ctx);
        let input = theme.text_input(input_id, None, None, &format!("Search {}...", label.to_lowercase()));
        let page = theme.page(Header::stack(&format!("Search {}", label.to_lowercase()), None),
            Content::new(Offset::Start, vec![input], Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true)), None);
        let mut result = Self {layout: Stack::default(), page, source, entries, messages, message_limit: 50,
            theme: theme.clone(), input_id, query: String::new(), focus_on_open: true};
        result.filter();
        result
    }

    pub fn filter(&mut self) {
        let query = self.query.to_lowercase();
        let terms = query.split_whitespace().collect::<Vec<_>>();
        let items: Vec<_> = self.entries.iter().filter(|entry| terms.iter().all(|term| entry.text.contains(term)))
            .map(|entry| entry.item.clone()).collect();
        let message = match &self.source {
            SearchSource::Contacts(_) if terms.is_empty() => "No contacts yet.",
            SearchSource::Contacts(_) => "No matching contacts.",
            SearchSource::Messages(_) if terms.is_empty() => "No messages yet.",
            SearchSource::Messages(_) => "No matching conversations or messages.",
        };
        let mut results = if matches!(&self.source, SearchSource::Messages(_)) && !terms.is_empty() {
            let mut matches: Vec<_> = self.messages.iter()
                .filter(|entry| terms.iter().all(|term| entry.text.contains(term)))
                .take(self.message_limit.saturating_add(1)).collect();
            let more = matches.len() > self.message_limit;
            matches.truncate(self.message_limit);
            let messages: Vec<_> = matches.into_iter().map(|entry| entry.item(&terms)).collect();
            if items.is_empty() && messages.is_empty() {vec![Display::list(None, vec![], Some(message))]}
            else {
                let mut sections = Vec::new();
                if !items.is_empty() {sections.push(Display::list(Some("Conversations"), items, None));}
                if !messages.is_empty() {sections.push(Display::list(Some("Messages"), messages, None));}
                if more {
                    let id = self.input_id;
                    sections.push(Display::actions(vec![ActionItem::new(
                        Action::custom(move |ctx: &mut Context, _: &Theme| ctx.emit(MoreSearchMessages(id))),
                        "Show more messages", Icons::DownArrow,
                    )], false));
                }
                sections
            }
        } else {vec![Display::list(None, items, Some(message))]};
        // Keep the original editor so live updates cannot reset its cursor or keyboard.
        let children = self.page.content_mut().children_mut();
        children.truncate(1);
        children.extend(results.iter_mut().flat_map(|results| results.build(&self.theme).unwrap_or_default()));
    }

    pub fn refresh(&mut self, ctx: &mut Context) {
        (self.entries, self.messages) = self.source.entries(ctx);
        self.filter();
    }
}

#[on_event]
impl OnEvent<TextInputEvent> for SearchPage {
    fn on(&mut self, _: &mut Context, event: TextInputEvent) -> Vec<Box<dyn DynEvent>> {
        if let TextInputEvent::Changed {id, value} | TextInputEvent::Set {id, value} = &event
            && *id == self.input_id && *value != self.query {
            self.query.clone_from(value);
            self.message_limit = 50;
            self.filter();
            self.page.content_mut().set_scroll(0.0);
        }
        prism::events![event]
    }
}

#[derive(Debug, Clone, Copy)]
struct MoreSearchMessages(Uuid);

impl prism::events::Event for MoreSearchMessages {
    fn pass(&self, _: &mut Context, children: &mut [prism::events::ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(*self);}
    }
}

#[on_event]
impl OnEvent<MoreSearchMessages> for SearchPage {
    fn on(&mut self, _: &mut Context, event: MoreSearchMessages) -> Vec<Box<dyn DynEvent>> {
        if event.0 == self.input_id {
            self.message_limit = self.message_limit.saturating_add(50);
            self.filter();
            return vec![];
        }
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<NavigationChanged> for SearchPage {
    fn on(&mut self, ctx: &mut Context, event: NavigationChanged) -> Vec<Box<dyn DynEvent>> {
        self.refresh(ctx);
        if self.focus_on_open {
            self.focus_on_open = false;
            ctx.emit(TextInputEvent::Focus {id: self.input_id});
        }
        prism::events![event]
    }
}

#[on_change(async {if self.source.next().await.is_none() {std::future::pending::<()>().await;}})]
impl OnChange<()> for SearchPage {
    fn on_change(&mut self, ctx: &mut Context, _: ()) {self.refresh(ctx);}
}
