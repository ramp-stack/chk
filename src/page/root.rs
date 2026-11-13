use prism::air::{Instance, Name};
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
use crate::design_system::{Theme, Icons};
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
            Self::Root { title, input, display, header, bumper_a, bumper_b } => {
                (title, input.into_iter().map(Item::Input).chain(display.into_iter().map(Item::Display)).collect::<Vec<_>>(), header, bumper_a, bumper_b)
            }
            Self::Content { title, items, header, .. } => {
                (title, items, header, None, None)
            }
            Self::Edit { title, header, items, display, on_save, .. } => {
                return Box::new(EditPage::root(
                    theme, is_mobile, title, header, items, display, on_save,
                ));
            }
            Self::Form { .. } | Self::Review { .. } | Self::Success { .. }
            | Self::Messaging { .. } | Self::Camera { .. } | Self::Photo { .. } | Self::Video { .. } | Self::Playback { .. } => {
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
enum SearchSource {
    Contacts(ListWatch<Profile>),
    Messages((ListWatch<ChatRoom>, ListWatch<Profile>, ListWatch<RoomReadState>)),
}
impl SearchSource {
    pub fn label(&self) -> &'static str {
        match self {Self::Contacts(_) => "Contacts", Self::Messages(_) => "Messages"}
    }
    pub fn entries(&mut self, ctx: &mut Context) -> Vec<SearchEntry> {
        match self {
            Self::Contacts(watch) => {
                let profiles = watch.get(ctx);
                SearchEntry::contacts(ctx, profiles)
            }
            Self::Messages(watch) => {
                let (rooms, profiles, reads) = watch.get(ctx);
                SearchEntry::messages(ctx, rooms, profiles, reads)
            }
        }
    }
    pub async fn next(&mut self) -> Option<()> {
        match self {Self::Contacts(watch) => watch.next().await, Self::Messages(watch) => watch.next().await}
    }
}

#[derive(Debug, Component, Clone)]
struct SearchPage {
    layout: Stack,
    page: PageView,
    #[skip] source: SearchSource,
    #[skip] entries: Vec<SearchEntry>,
    #[skip] theme: Theme,
    #[skip] input_id: Uuid,
    #[skip] query: String,
    #[skip] focus_on_open: bool,
}
impl AppPage for SearchPage {}
impl SearchPage {
    pub fn open(ctx: &mut Context, theme: &Theme, source: SearchSource) {
        let page = Self::new(ctx, theme, source);
        ctx.emit(NavigationEvent::push(crate::FlowWrapper::new(
            ptsd::navigation::Flow::new(vec![Box::new(page)]),
        )));
    }

    pub fn new(ctx: &mut Context, theme: &Theme, mut source: SearchSource) -> Self {
        let input_id = Uuid::new_v4();
        let label = source.label();
        let entries = source.entries(ctx);
        let input = theme.text_input(input_id, None, None, &format!("Search {}...", label.to_lowercase()));
        let page = theme.page(Header::stack(&format!("Search {}", label.to_lowercase()), None),
            Content::new(Offset::Start, vec![input], Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true)), None);
        let mut result = Self {layout: Stack::default(), page, source, entries,
            theme: theme.clone(), input_id, query: String::new(), focus_on_open: true};
        result.filter();
        result
    }

    pub fn filter(&mut self) {
        let query = self.query.to_lowercase();
        let terms = query.split_whitespace().collect::<Vec<_>>();
        let items = self.entries.iter().filter(|entry| terms.iter().all(|term| entry.text.contains(term)))
            .map(|entry| entry.item.clone()).collect();
        let message = match &self.source {
            SearchSource::Contacts(_) if terms.is_empty() => "No contacts yet.",
            SearchSource::Contacts(_) => "No matching contacts.",
            SearchSource::Messages(_) if terms.is_empty() => "No messages yet.",
            SearchSource::Messages(_) => "No matching conversations.",
        };
        let mut results = Display::list(None, items, Some(message));
        // Keep the original editor so live updates cannot reset its cursor or keyboard.
        let children = self.page.content_mut().children_mut();
        children.truncate(1);
        children.extend(results.build(&self.theme).unwrap_or_default());
    }

    pub fn refresh(&mut self, ctx: &mut Context) {
        self.entries = self.source.entries(ctx);
        self.filter();
    }
}

#[on_event]
impl OnEvent<TextInputEvent> for SearchPage {
    fn on(&mut self, _: &mut Context, event: TextInputEvent) -> Vec<Box<dyn DynEvent>> {
        if let TextInputEvent::Changed {id, value} | TextInputEvent::Set {id, value} = &event
            && *id == self.input_id && *value != self.query {
            self.query.clone_from(value);
            self.filter();
            self.page.content_mut().set_scroll(0.0);
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
