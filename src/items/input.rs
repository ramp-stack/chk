use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
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

    fn text_placeholder(label: &str) -> String {
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
        Some(match self {
            Input::Text { id, show_label, label, preset, actions } => {
                let mut items: Vec<Box<dyn Drawable>> = vec![Box::new(TextInput::new(
                    theme, *id, preset.as_deref(), show_label.then_some(label),
                    Some(&Self::text_placeholder(label)),
                    None, None,
                ))];
                if let Some(a) = actions.as_ref()
                    && !a.is_empty() {
                    items.push(Box::new(QuickActions::new(
                        theme, Offset::Start,
                        a.iter()
                            .map(|ActionItem(action, label, active, icon)| {
                                let on_click: Box<dyn Callback> = action.get();
                                ActionData {
                                    label: ActionItem::format_label(label),
                                    active: active.as_deref().map(ActionItem::format_label),
                                    icon: *icon,
                                    on_click,
                                }
                            })
                            .collect::<Vec<ActionData>>(),
                    )));
                }

                items
            }
            Input::Enumerator { items } => vec![Box::new(RadioSelector::new(
                theme, 0, items.iter().map(|item| item.get()).collect::<Vec<_>>(),
            ))],
            Input::Currency { instructions } => {
                let mut drawables: Vec<Box<dyn Drawable>> =
                    vec![Box::new(NumericalInput::numerical(theme, instructions))];
                if is_mobile { drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            }
            Input::Number { instructions } => {
                let mut drawables: Vec<Box<dyn Drawable>> =
                    vec![Box::new(NumericalInput::number(theme, instructions))];
                if is_mobile { drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            }
            Input::Date { instructions } => {
                let mut drawables: Vec<Box<dyn Drawable>> =
                    vec![Box::new(NumericalInput::date(theme, instructions))];
                if is_mobile { drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            }
            Input::Time { instructions } => {
                let mut drawables: Vec<Box<dyn Drawable>> =
                    vec![Box::new(NumericalInput::time(theme, instructions))];
                if is_mobile { drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            }
            Input::Avatar { content, flair, action } => {
                vec![Box::new(AvatarInput::new(theme, content.clone(), *flair, action.clone()))]
            }
            Input::Search { id, label, items, actions } => vec![Box::new(SelectionInput::legacy(
                theme, *id, label, items.clone(), actions.as_ref().map(|actions| actions.iter().map(ActionItem::selection_action).collect()).unwrap_or_default(),
            ))],
            Input::Selection(selector) => vec![Box::new(SelectionInput::new(theme, selector.clone()))],
            Input::QRCodeScanner { instructions, alt, on_find } => {
                let mut items: Vec<Box<dyn Drawable>> = vec![
                    Box::new(QRCodeScanner::new(theme, on_find.clone())),
                    Box::new(Text::new(
                        theme, instructions, TextSize::Md, TextStyle::Secondary, Align::Center,
                        None,
                    )),
                ];
                if let Some((label, icon, action)) = alt.as_ref() {
                    items.push(Box::new(Text::new(
                        theme, "or", TextSize::Md, TextStyle::Secondary, Align::Center, None,
                    )));
                    items.push(Box::new(QuickActions::new(
                        theme, Offset::Center,
                        vec![ActionData::new(&ActionItem::format_label(label), None, *icon, action.get())],
                    )));
                }
                items
            }
        })
    }

    pub fn offset(&self) -> Offset {
        match self {
            Input::Text { .. }
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

    pub fn state(child: &dyn Drawable) -> Option<State> {
        if let Some(input) = child.downcast_ref::<TextInput>() {
            Some(State::Text(input.value()))
        } else if let Some(selector) = child.downcast_ref::<RadioSelector>() {
            Some(State::Enumerator(selector.value()))
        } else if let Some(input) = child.downcast_ref::<NumericalInput>() {
            Some(State::Number(input.value()))
        } else if let Some(avatar) = child.downcast_ref::<AvatarInput>() {
            Some(State::Avatar(avatar.content().clone()))
        } else if let Some(selection) = child.downcast_ref::<SelectionInput>() {
            Some(if selection.uses_names() {State::Search(selection.results().iter().filter_map(|key| key.parse().ok()).collect())} else {State::Selection(selection.results())})
        } else {
            child.downcast_ref::<QRCodeScanner>().map(|scanner| State::ScanCode(scanner.found()))
        }
    }
}
