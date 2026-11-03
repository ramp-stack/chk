//! One searchable list selector for contacts and application-owned item keys.
use pelican_ui::{Context, Callback};
use pelican_ui::components::{TextInput, TextInputEvent, SearchbarEvent};
use pelican_ui::components::button::{ActionData, QuickActions, SecondaryButton};
use pelican_ui::components::list_item::ListItem as Row;
use pelican_ui::components::text::{Text, TextSize, TextStyle};
use prism::drawable::text::Align;
use pelican_ui::interface::general::FormEvent;
use pelican_ui::theme::{Icons, Theme};
use prism::air::Name;
use prism::drawable::Component;
use prism::events::{ChildEvent, DynEvent, Event, OnEvent, on_event};
use ptsd::layout::{Column, Offset, Stack, Wrap};
use uuid::Uuid;

use crate::{Action, FormAction, ActionItem, ListItem, State};

/// A searchable list of application-owned keys. Clones address the same selector.
/// Create a separate handle for each independent input, including identical lists.
#[derive(Clone, Debug)]
pub struct Selector {
    id: Uuid,
    label: String,
    items: Vec<(ListItem, String)>,
    limit: usize,
    actions: Vec<FormAction>,
    chip_subtitles: bool,
}

impl PartialEq for Selector {
    fn eq(&self, other: &Self) -> bool {self.id == other.id}
}

impl Selector {
    pub fn new(label: &str, items: Vec<(ListItem, String)>) -> Self {
        Self {id: Uuid::new_v4(), label: label.into(), items, limit: usize::MAX,
            actions: vec![], chip_subtitles: false}
    }

    pub fn with_limit(mut self, limit: usize) -> Self {self.limit = limit; self}
    pub fn with_actions(mut self, actions: Vec<FormAction>) -> Self {self.actions = actions; self}
    pub fn with_chip_subtitles(mut self, show: bool) -> Self {self.chip_subtitles = show; self}

    /// Add or update a row and select it, for example after creating a contact.
    /// The action only affects this handle, and never deselects an existing choice.
    pub fn select(&self, item: ListItem, key: impl Into<String>) -> Action {
        let event = SelectionEvent(self.id, SelectionChange::Select(item, key.into()));
        Action::custom(move |ctx: &mut Context, _: &Theme| ctx.emit(event.clone()))
    }
}

#[derive(Clone, Debug)]
enum SelectionChange {Toggle(String), Select(ListItem, String)}

#[derive(Clone, Debug)]
struct SelectionEvent(Uuid, SelectionChange);
impl Event for SelectionEvent {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}

#[derive(Clone, Debug, Default)]
struct Selection {
    selected: Vec<String>,
    limit: usize,
}
impl Selection {
    fn select(&mut self, key: &str) -> bool {
        if self.selected.len() >= self.limit || self.selected.iter().any(|value| value == key) {
            return false;
        }
        self.selected.push(key.to_owned());
        true
    }

    fn toggle(&mut self, key: &str) -> bool {
        if let Some(index) = self.selected.iter().position(|value| value == key) {
            self.selected.remove(index);
            true
        } else {self.select(key)}
    }
}

#[derive(Clone, Debug, Component)]
struct Pills(Wrap, Vec<SecondaryButton>);

#[derive(Clone, Debug, Component)]
struct Rows(Column, Vec<Row>);

#[derive(Clone, Debug, Component)]
pub(crate) struct SelectionInput {
    layout: Column,
    input: TextInput,
    actions: Option<QuickActions>,
    pills: Pills,
    rows: Rows,
    status: Option<Text>,
    #[skip] selector: Selector,
    #[skip] selection: Selection,
    #[skip] theme: Theme,
    #[skip] form: Option<Uuid>,
    #[skip] legacy_names: bool,
}

impl SelectionInput {
    pub(crate) fn new(theme: &Theme, selector: Selector) -> Self {
        let actions = selector.actions.iter().map(|action| action.get(selector.id)).collect();
        Self::build(theme, selector, actions, false)
    }

    pub(crate) fn legacy(theme: &Theme, id: Uuid, label: &str,
        items: Vec<(ListItem, Name)>, actions: Vec<ActionItem>) -> Self {
        let mut selector = Selector::new(label, items.into_iter()
            .map(|(item, name)| (item, name.to_string())).collect());
        selector.id = id;
        Self::build(theme, selector, actions, true)
    }

    fn build(theme: &Theme, mut selector: Selector, actions: Vec<ActionItem>, legacy_names: bool) -> Self {
        let mut seen = std::collections::HashSet::new();
        selector.items.retain(|(_, key)| !key.is_empty() && seen.insert(key.clone()));
        let actions = (!actions.is_empty()).then(|| QuickActions::new(theme, Offset::Start,
            actions.iter().map(|ActionItem(action, label, active, icon)| ActionData {
                label: ActionItem::format_label(label), active: active.as_deref().map(ActionItem::format_label),
                icon: *icon, on_click: action.get(),
            }).collect()));
        let mut result = Self {
            layout: Column::start(12.0),
            input: TextInput::new(theme, selector.id, None, None,
                Some(&format!("Search {}...", selector.label.to_lowercase())), None, None),
            actions, pills: Pills(Wrap::start(8.0, 8.0), vec![]),
            rows: Rows(Column::default(), vec![]), status: None,
            selection: Selection {selected: vec![], limit: selector.limit}, selector,
            theme: theme.clone(), form: None, legacy_names,
        };
        result.refresh();
        result
    }

    pub(crate) fn results(&self) -> Vec<String> {self.selection.selected.clone()}

    pub(crate) fn state(&self) -> State {
        if self.legacy_names {
            State::Search(self.selection.selected.iter().filter_map(|key| key.parse().ok()).collect())
        } else {State::Selection(self.results())}
    }

    fn refresh(&mut self) {
        self.pills.1 = self.selection.selected.iter().filter_map(|key| {
            let (item, _) = self.selector.items.iter().find(|(_, id)| id == key)?;
            let (title, subtitle) = item.selection_labels();
            let label = if self.selector.chip_subtitles && !subtitle.is_empty() {
                format!("{title} · {subtitle}")
            } else {title.to_owned()};
            let event = SelectionEvent(self.selector.id, SelectionChange::Toggle(key.clone()));
            Some(SecondaryButton::medium(&self.theme, Icons::Close, &label, None,
                Stack::default(), move |ctx, _| ctx.emit(event.clone())))
        }).collect();
        self.refresh_rows(&self.input.value());
    }

    fn refresh_rows(&mut self, query: &str) {
        if self.selection.selected.len() >= self.selector.limit {
            self.rows.1.clear();
            self.set_status((self.selector.limit > 1).then(|| format!("Choose up to {} items.", self.selector.limit)));
            return;
        }
        self.rows.1 = self.selector.items.iter()
            .filter(|(_, key)| !self.selection.selected.contains(key))
            .filter_map(|(item, key)| {
                let (title, subtitle) = item.selection_labels();
                if !matches_query(title, subtitle, query) {return None;}
                let event = SelectionEvent(self.selector.id, SelectionChange::Toggle(key.clone()));
                let callback: Box<dyn Callback> = Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(event.clone()));
                Some(item.selection_row(&self.theme, callback))
            }).collect();
        let status = if self.selector.items.is_empty() {Some("No items available.".into())}
            else if self.rows.1.is_empty() && !query.trim().is_empty() {Some("No matching items.".into())}
            else {None};
        self.set_status(status);
    }

    fn set_status(&mut self, message: Option<String>) {
        self.status = message.map(|message| Text::new(&self.theme, &message, TextSize::Md,
            TextStyle::Secondary, Align::Center, None));
    }

    fn change(&mut self, change: SelectionChange) -> bool {
        match change {
            SelectionChange::Toggle(key) => self.selector.items.iter().any(|(_, id)| id == &key)
                && self.selection.toggle(&key),
            SelectionChange::Select(item, key) if !key.is_empty() => {
                if let Some((previous, _)) = self.selector.items.iter_mut().find(|(_, id)| id == &key) {
                    *previous = item;
                } else {self.selector.items.push((item, key.clone()));}
                self.selection.select(&key);
                true
            },
            _ => false,
        }
    }

    fn refresh_form(&mut self, ctx: &mut Context) {
        self.refresh();
        if let Some(form) = self.form {ctx.emit(FormEvent::Refresh(form));}
    }
}

fn matches_query(title: &str, subtitle: &str, query: &str) -> bool {
    let title = title.to_lowercase();
    let subtitle = subtitle.to_lowercase();
    query.to_lowercase().split_whitespace().all(|term| title.contains(term) || subtitle.contains(term))
}

#[on_event]
impl OnEvent<TextInputEvent> for SelectionInput {
    fn on(&mut self, _: &mut Context, event: TextInputEvent) -> Vec<Box<dyn DynEvent>> {
        // The child applies the event afterward. Keep that text input, focus and cursor intact.
        if let TextInputEvent::Changed {id, value} | TextInputEvent::Set {id, value} = &event
            && *id == self.selector.id {self.refresh_rows(value);}
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<SelectionEvent> for SelectionInput {
    fn on(&mut self, ctx: &mut Context, event: SelectionEvent) -> Vec<Box<dyn DynEvent>> {
        if event.0 != self.selector.id {return prism::events![event];}
        if self.change(event.1) {self.refresh_form(ctx);}
        vec![]
    }
}

#[on_event]
impl OnEvent<SearchbarEvent> for SelectionInput {
    fn on(&mut self, ctx: &mut Context, event: SearchbarEvent) -> Vec<Box<dyn DynEvent>> {
        // Compatibility for Action::choose_search; generic selectors never consume contact events.
        if !self.legacy_names {return prism::events![event];}
        if let SearchbarEvent::Select(name) = &event {
            let key = name.to_string();
            let item = self.selector.items.iter().find(|(_, id)| id == &key).map(|(item, _)| item.clone())
                .unwrap_or_else(|| {
                    let title = format!("{}...{}", &key[..7], &key[key.len() - 3..]);
                    ListItem::plain(&title, "", None, None)
                });
            if self.change(SelectionChange::Select(item, key)) {self.refresh_form(ctx);}
            return vec![];
        }
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<FormEvent> for SelectionInput {
    fn on(&mut self, _: &mut Context, event: FormEvent) -> Vec<Box<dyn DynEvent>> {
        if let FormEvent::Bind(id) = event {
            self.form = Some(id);
            return vec![];
        }
        prism::events![event]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism::drawable::Drawable;
    mod support {include!("../tests/support/mod.rs");}

    fn item(title: &str) -> ListItem {ListItem::plain(title, "family@example.test", None, None)}

    fn drain(input: &mut SelectionInput, ctx: &mut Context) {
        let mut pending = ctx.events();
        while let Some(event) = pending.pop_front() {
            input.request_width();
            input.request_height(400.0);
            let layout = Drawable::layout(input, (400.0, 800.0));
            input.event(ctx, &layout, event);
            pending.extend(ctx.events());
        }
    }

    fn select(input: &mut SelectionInput, ctx: &mut Context, selector: &Selector, key: &str) {
        selector.select(item(key), key).get()(ctx, &Theme::default());
        drain(input, ctx);
    }

    #[test]
    fn selection_is_scoped_unique_bounded_and_explicit_select_is_idempotent() {
        let theme = Theme::default();
        let selector = Selector::new("items", vec![
            (item("First"), "first".into()), (item("Duplicate"), "first".into()),
            (item("Empty"), String::new()),
        ]).with_limit(2);
        let other = Selector::new("items", vec![]);
        let mut input = SelectionInput::new(&theme, selector.clone());
        let mut ctx = support::context();
        assert_eq!(input.selector.items.len(), 1);
        select(&mut input, &mut ctx, &other, "wrong-selector");
        assert!(input.results().is_empty());
        ctx.emit(SelectionEvent(selector.id, SelectionChange::Toggle("unknown".into())));
        drain(&mut input, &mut ctx);
        assert!(input.results().is_empty());

        select(&mut input, &mut ctx, &selector, "first");
        select(&mut input, &mut ctx, &selector.clone(), "first");
        select(&mut input, &mut ctx, &selector, "second");
        select(&mut input, &mut ctx, &selector, "third");
        assert_eq!(input.state(), State::Selection(vec!["first".into(), "second".into()]));
        assert!(input.rows.1.is_empty(), "at the limit only removable chips remain");
        assert!(input.status.is_some());
        ctx.emit(SelectionEvent(selector.id, SelectionChange::Toggle("first".into())));
        drain(&mut input, &mut ctx);
        assert_eq!(input.results(), ["second"]);
        assert_eq!(input.rows.1.len(), 2);
    }

    #[test]
    fn query_filters_labels_and_survives_selection_without_replacing_input() {
        let theme = Theme::default();
        let selector = Selector::new("passwords", vec![
            (item("Amazon"), "amazon".into()), (item("Gmail"), "gmail".into()),
        ]);
        let mut input = SelectionInput::new(&theme, selector.clone());
        let mut ctx = support::context();
        ctx.emit(TextInputEvent::Set {id: selector.id, value: "AMAZON family".into()});
        drain(&mut input, &mut ctx);
        assert_eq!(input.rows.1.len(), 1);
        assert_eq!(input.rows.1[0].title(), "Amazon");
        let id = input.input.id();
        ctx.emit(SelectionEvent(selector.id, SelectionChange::Toggle("amazon".into())));
        drain(&mut input, &mut ctx);
        assert_eq!(input.input.id(), id);
        assert_eq!(input.input.value(), "AMAZON family");
        assert!(input.rows.1.is_empty());
        assert!(input.status.is_some());
        assert!(!matches_query("Amazon", "family@example.test", "private password"));
    }

    #[test]
    fn legacy_names_use_same_control_but_do_not_select_generic_items() {
        let theme = Theme::default();
        let name = Name::orange_me();
        let mut legacy = SelectionInput::legacy(&theme, Uuid::new_v4(), "contacts", vec![], vec![]);
        let mut generic = SelectionInput::new(&theme, Selector::new("items", vec![]));
        let mut ctx = support::context();
        ctx.emit(SearchbarEvent::Select(name));
        drain(&mut generic, &mut ctx);
        assert_eq!(generic.state(), State::Selection(vec![]));
        ctx.emit(SearchbarEvent::Select(name));
        drain(&mut legacy, &mut ctx);
        ctx.emit(SearchbarEvent::Select(name));
        drain(&mut legacy, &mut ctx);
        assert_eq!(legacy.state(), State::Search(vec![name]));
        assert_eq!(legacy.pills.1.len(), 1);
    }

    #[test]
    fn selection_binds_to_form_and_requires_nonempty_results() {
        let theme = Theme::default();
        let selector = Selector::new("items", vec![(item("First"), "first".into())]);
        let form = crate::FormItem::selection("Choose items", selector.clone());
        let mut input = SelectionInput::new(&theme, selector.clone());
        let mut ctx = support::context();
        let id = Uuid::new_v4();
        assert!(OnEvent::<FormEvent>::on(&mut input, &mut ctx, FormEvent::Bind(id)).is_empty());
        let mut child: Box<dyn Drawable> = Box::new(input.clone());
        assert!(!form.validation().validate(&mut ctx, vec![&mut child]));
        OnEvent::<SelectionEvent>::on(&mut input, &mut ctx,
            SelectionEvent(selector.id, SelectionChange::Toggle("first".into())));
        assert!(ctx.events().iter().any(|event| matches!(event.downcast_ref::<FormEvent>(),
            Some(FormEvent::Refresh(target)) if *target == id)));
        child = Box::new(input.clone());
        assert!(form.validation().validate(&mut ctx, vec![&mut child]));
        assert_eq!(crate::Input::state(child.as_ref()), Some(State::Selection(vec!["first".into()])));
        input.change(SelectionChange::Toggle("first".into()));
        child = Box::new(input);
        assert!(!form.validation().validate(&mut ctx, vec![&mut child]));
    }
}
