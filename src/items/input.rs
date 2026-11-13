use super::*;
use ptsd::selection::{SelectorItem, SelectionChange, SelectionEvent};

/// Data for a selectable list. Clones refer to the same input.
#[derive(Clone, Debug)]
pub struct Selector {
    pub id: Uuid,
    pub label: String,
    pub items: Vec<(SelectorItem, String)>,
    pub limit: usize,
    pub actions: Vec<crate::FormAction>,
    pub chip_subtitles: bool,
    pub sort_by_title: bool,
}

impl PartialEq for Selector {
    fn eq(&self, other: &Self) -> bool {self.id == other.id}
}

impl Selector {
    pub fn new<I: Into<SelectorItem>>(label: &str, items: Vec<(I, String)>) -> Self {
        Self {
            id: Uuid::new_v4(), label: label.into(),
            items: items.into_iter().map(|(item, key)| (item.into(), key)).collect(),
            limit: usize::MAX, actions: vec![], chip_subtitles: false, sort_by_title: false,
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {self.limit = limit; self}
    pub fn with_actions(mut self, actions: Vec<crate::FormAction>) -> Self {self.actions = actions; self}
    pub fn with_chip_subtitles(mut self, show: bool) -> Self {self.chip_subtitles = show; self}
    pub fn with_alphabetical_order(mut self) -> Self {self.sort_by_title = true; self}

    /// Add or update a row and select it without deselecting an existing choice.
    pub fn select(&self, item: impl Into<SelectorItem>, key: impl Into<String>) -> Box<dyn Callback> {
        let event = SelectionEvent(self.id, SelectionChange::Select(item.into(), key.into()));
        Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(event.clone()))
    }
}

impl From<ListItem> for SelectorItem {
    fn from(item: ListItem) -> Self {
        Self {title: item.title, subtitle: item.subtitle, avatar: item.avatar,
            secondary: item.secondary, badge: item.badge, flair: item.flair}
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Slider {start: f32, label: Option<String>, description: Option<String>},
    Text {
        id: Uuid,
        label: String,
        actions: Option<Vec<ActionItem>>,
        show_label: bool,
        preset: Option<String>,
    },
    Currency { instructions: String },
    Number { instructions: String },
    Date { instructions: String },
    Time { instructions: String },
    Enumerator { items: Vec<ValueItem> },
    Avatar {
        content: AvatarContent,
        flair: Option<(Icons, AvatarIconStyle)>,
        action: Option<Action>,
    },
    Search {
        id: Uuid,
        label: String,
        items: Vec<(ListItem, Name)>,
        actions: Option<Vec<ActionItem>>,
    },
    Selection(Selector),
    QRCodeScanner {
        instructions: String,
        alt: Option<(String, Icons, Action)>,
        on_find: Box<dyn QrCodeFound>,
    },
}

impl Input {
    pub fn slider(start: f32, label: Option<&str>, description: Option<&str>) -> Self {
        Self::Slider {start, label: label.map(str::to_owned), description: description.map(str::to_owned)}
    }

    pub fn currency(instructions: &str) -> Self {
        Input::Currency { instructions: instructions.to_string() }
    }

    pub fn date(instructions: &str) -> Self {
        Input::Date { instructions: instructions.to_string() }
    }

    pub fn time(instructions: &str) -> Self {
        Input::Time { instructions: instructions.to_string() }
    }

    pub fn number(instructions: &str) -> Self {
        Input::Number { instructions: instructions.to_string() }
    }

    pub fn enumerator(items: Vec<ValueItem>) -> Self { Input::Enumerator { items } }

    pub fn text(
        id: Uuid, label: &str, show_label: bool, preset: Option<String>,
        actions: Option<Vec<ActionItem>>,
    ) -> Self { Input::Text { id, label: label.to_string(), show_label, preset, actions } }

    pub fn avatar(content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>) -> Self {
        Input::Avatar { content, flair, action }
    }

    pub fn search(id: Uuid, label: &str, items: Vec<(ListItem, Name)>, actions: Option<Vec<ActionItem>>) -> Self {
        Input::Search { id, label: label.to_string(), items, actions }
    }

    pub fn selection(selector: Selector) -> Self {Input::Selection(selector)}

    pub fn qr_code_scanner(instructions: &str, alt: Option<(String, Icons, Action)>, on_find: Box<dyn QrCodeFound>) -> Self {
        Input::QRCodeScanner { instructions: instructions.to_string(), alt, on_find }
    }

    pub(crate) fn text_placeholder(label: &str) -> String {
        let label = label.trim();
        let first_word = label.split_whitespace().next().unwrap_or_default();
        // Labels start sentences; placeholders continue after "Enter". Keep
        // intentional capitalization in names such as AirName and acronyms.
        let mut chars = label.chars();
        let label = match chars.next() {
            Some(first) if !first_word.chars().skip(1).any(char::is_uppercase) => {
                first.to_lowercase().chain(chars).collect()
            }
            _ => label.to_string(),
        };
        format!("Enter {label}...")
    }

    pub fn build(&self, theme: &Theme, is_mobile: bool) -> Option<Vec<Box<dyn Drawable>>> {
        let mut children = match self {
            Self::Slider {start, label, description} => vec![theme.slider_input(*start, label.as_deref(), description.as_deref())],
            Self::Text {id, label, actions, show_label, preset} => {
                let mut children = vec![theme.text_input(*id, preset.as_deref(), show_label.then_some(label.as_str()), &Self::text_placeholder(label))];
                if let Some(actions) = actions.as_ref().filter(|actions| !actions.is_empty()) {
                    children.push(theme.quick_actions(Offset::Start, actions.iter().map(ActionItem::presentation).collect()));
                }
                children
            }
            Self::Enumerator {items} => vec![theme.choice_input(0, items.iter().map(ValueItem::get).collect())],
            Self::Currency {instructions} => vec![theme.currency_input(instructions)],
            Self::Number {instructions} => vec![theme.number_input(instructions)],
            Self::Date {instructions} => vec![theme.date_input(instructions)],
            Self::Time {instructions} => vec![theme.time_input(instructions)],
            Self::Avatar {content, flair, action} => vec![Box::new(crate::Control::new(AvatarInput::new(theme, content.clone(), *flair, action.clone()))) as Box<dyn Drawable>],
            Self::Search {id, label, items, actions} => vec![theme.search_input(*id, label, items.clone(),
                actions.as_ref().map(|actions| actions.iter().map(ActionItem::presentation).collect()).unwrap_or_default())],
            Self::Selection(selector) => vec![theme.selection_input(selector.clone())],
            Self::QRCodeScanner {instructions, alt, on_find} => {
                let mut children = vec![theme.scanner_input(on_find.clone()),
                    theme.text_view(instructions, TextSize::Md, TextStyle::Secondary, Align::Center)];
                if let Some((label, icon, action)) = alt {
                    children.push(theme.text_view("or", TextSize::Md, TextStyle::Secondary, Align::Center));
                    children.push(theme.quick_actions(Offset::Center,
                        vec![ActionItem::new(action.clone(), label, *icon).presentation()]));
                }
                children
            }
        };
        if is_mobile && matches!(self, Self::Currency {..} | Self::Number {..} | Self::Date {..} | Self::Time {..}) {
            children.push(theme.keypad());
        }
        Some(children)
    }

    pub fn offset(&self) -> Offset {
        match self {
            Input::Slider { .. }
            | Input::Text { .. }
            | Input::Number { .. }
            | Input::Enumerator { .. }
            | Input::Currency { .. }
            | Input::Date { .. }
            | Input::Time { .. }
            | Input::Avatar { .. }
            | Input::Search { .. }
            | Input::Selection(..) => Offset::Start,
            Input::QRCodeScanner { .. } => Offset::Center,
        }
    }

    pub fn state(child: &dyn Drawable) -> Option<State> {child.downcast_ref::<crate::Control>().map(crate::Control::state)}
}
