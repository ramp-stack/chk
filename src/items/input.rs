use prism::air::Name;
use prism::drawable::{Drawable, text::Align};
use ptsd::layout::Offset;
use ptsd::utils::ValidationFn;
use ptsd::Validate;
use ptsd::selection::{SelectionChange, SelectionEvent, SelectorItem};
use uuid::Uuid;

use std::{collections::HashSet, future::Future, pin::Pin};

use crate::design_system::{
    AvatarContent, AvatarIconStyle, Callback, Context, Icons, QrCodeFound, TextSize, TextStyle,
    Theme,
};
use crate::items::{Action, ActionItem, AvatarInput, ListItem, ValueItem};
use crate::form::{FormComplete, FormValues};
use crate::profiles::Profile;
use crate::closure::{FormSubmit, ValidityFn};
use crate::{ContactInvitations, Control};

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Slider(f32),
    Text(String),
    Enumerator(usize),
    Number(String),
    Avatar(AvatarContent),
    Selection(Vec<String>),
    ScanCode(Option<String>),
}

#[derive(Clone, Debug)]
pub enum NumberVariant {
    Currency,
    Date,
    Time,
    Number,
}

/// Data for a selectable list. Clones refer to the same input.
#[derive(Clone, Debug)]
pub struct Selector {
    pub id: Uuid,
    pub label: String,
    pub items: Vec<(SelectorItem, String)>,
    pub limit: usize,
    pub actions: Vec<FormAction>,
    pub chip_subtitles: bool,
    pub sort_by_title: bool,
}

impl PartialEq for Selector {
    fn eq(&self, other: &Self) -> bool {self.id == other.id}
}

impl Selector {
    pub fn new<I: Into<SelectorItem>>(
        id: Uuid, label: &str, items: Vec<(I, String)>, limit: usize,
        actions: Vec<FormAction>, chip_subtitles: bool, sort_by_title: bool,
    ) -> Selector {
        Selector {
            id, label: label.into(),
            items: items.into_iter().map(|(item, key)| (item.into(), key)).collect(),
            limit, actions, chip_subtitles, sort_by_title,
        }
    }

    pub fn contact_submit(
        selector_id: Uuid, invitations: &ContactInvitations, theme: &Theme,
        excluded: Vec<Name>, input_id: Uuid,
    ) -> Box<dyn FormSubmit> {
        let invitations = invitations.clone();
        let theme = theme.clone();
        Box::new(move |ctx: &mut Context, values: &FormValues| {
            let value = values.text("airname").unwrap_or_default();
            let name = match invitations.validate_contact(ctx, value, &excluded, true) {
                Ok(name) => name,
                Err(_) => {
                    ctx.emit(crate::design_system::TextInputEvent::Changed {
                        id: input_id, value: value.to_string(),
                    });
                    return FormComplete::Action(Action::None);
                }
            };
            let profile = Profile::from_name(ctx, name);
            let profile = profile.pending();
            let Some(row) = ListItem::recipient(ctx, &theme, &profile) else {
                return FormComplete::None;
            };
            FormComplete::Return(Selector::select(selector_id, row, name.to_string()))
        })
    }

    /// Add or update a row and select it without deselecting an existing choice.
    pub fn select(id: Uuid, item: impl Into<SelectorItem>, key: impl Into<String>) -> Box<dyn Callback> {
        let event = SelectionEvent(id, SelectionChange::Select(item.into(), key.into()));
        Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(event.clone()))
    }
}

#[derive(Debug, Clone)]
pub enum Input {
    Slider {start: f32, label: Option<String>, description: Option<String>},
    Text {
        id: Uuid,
        label: String,
        placeholder: String,
        actions: Option<Vec<ActionItem>>,
        show_label: bool,
        preset: Option<String>,
    },
    Currency {instructions: String},
    Number {instructions: String},
    Date {instructions: String},
    Time {instructions: String},
    Enumerator {items: Vec<ValueItem>},
    Avatar {
        content: AvatarContent,
        flair: Option<(Icons, AvatarIconStyle)>,
        action: Option<Action>,
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
        Input::Slider {
            start, label: label.map(str::to_owned), description: description.map(str::to_owned),
        }
    }

    pub fn currency(instructions: &str) -> Self {
        Input::Currency { instructions: instructions.to_string() }
    }

    pub fn date(instructions: &str) -> Self {Input::Date { instructions: instructions.to_string() }}

    pub fn time(instructions: &str) -> Self {Input::Time { instructions: instructions.to_string() }}

    pub fn number(instructions: &str) -> Self {
        Input::Number { instructions: instructions.to_string() }
    }

    pub fn enumerator(items: Vec<ValueItem>) -> Self {Input::Enumerator { items }}

    pub fn text(id: Uuid, label: &str, show_label: bool, preset: Option<String>, actions: Option<Vec<ActionItem>>) -> Self {
        let text = label.trim();
        let first_word = text.split_whitespace().next().unwrap_or_default();
        // Keep acronyms and mixed-case names such as AirName in the placeholder.
        let mut chars = text.chars();
        let text = match chars.next() {
            Some(first) if !first_word.chars().skip(1).any(char::is_uppercase) => {
                first.to_lowercase().chain(chars).collect()
            }
            _ => text.to_string(),
        };
        Input::Text {
            id, label: label.to_string(), placeholder: format!("Enter {text}..."), show_label,
            preset, actions,
        }
    }

    pub fn avatar(content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>) -> Self {
        Input::Avatar { content, flair, action }
    }

    pub fn selection(selector: Selector) -> Self {Input::Selection(selector)}

    pub fn qr_code_scanner(instructions: &str, alt: Option<(String, Icons, Action)>, on_find: Box<dyn QrCodeFound>) -> Self {
        Input::QRCodeScanner { instructions: instructions.to_string(), alt, on_find }
    }

    pub fn build(&self, theme: &Theme, is_mobile: bool) -> Vec<Box<dyn Drawable>> {
        let mut children: Vec<Box<dyn Drawable>> = match self {
            Input::Slider { start, label, description } => {
                vec![Box::new(theme.slider_input(*start, label.as_deref(), description.as_deref()))]
            }
            Input::Text { id, label, placeholder, actions, show_label, preset } => {
                let mut children: Vec<Box<dyn Drawable>> = vec![Box::new(theme.text_input(
                    *id, preset.as_deref(), show_label.then_some(label.as_str()), placeholder,
                ))];
                if let Some(actions) = actions.as_ref().filter(|actions| !actions.is_empty()) {
                    children.push(theme.quick_actions(
                        Offset::Start, actions.iter().map(ActionItem::presentation).collect(),
                    ));
                }
                children
            }
            Input::Enumerator { items } => vec![
                Box::new(theme.choice_input(
                    0,
                    items.iter().map(|item| {
                            (
                                item.title.as_str(), item.data.as_str(),
                                Box::new(|_: &mut Context, _: &Theme| {}) as Box<dyn Callback>,
                            )
                        }).collect(),
                )),
            ],
            Input::Currency { instructions } => vec![Box::new(theme.currency_input(instructions))],
            Input::Number { instructions } => vec![Box::new(theme.number_input(instructions))],
            Input::Date { instructions } => vec![Box::new(theme.date_input(instructions))],
            Input::Time { instructions } => vec![Box::new(theme.time_input(instructions))],
            Input::Avatar { content, flair, action } => vec![Box::new(Control::new(
                AvatarInput::new(theme, content.clone(), *flair, action.clone()),
            )) as Box<dyn Drawable>],
            Input::Selection(selector) => vec![Box::new(theme.selection_input(selector.clone()))],
            Input::QRCodeScanner { instructions, alt, on_find } => {
                let mut children: Vec<Box<dyn Drawable>> = vec![
                    Box::new(theme.scanner_input(on_find.clone())),
                    theme.text_view(
                        instructions, TextSize::Md, TextStyle::Secondary, Align::Center,
                    ),
                ];
                if let Some((label, icon, action)) = alt {
                    children.push(theme.text_view(
                        "or", TextSize::Md, TextStyle::Secondary, Align::Center,
                    ));
                    children.push(theme.quick_actions(
                        Offset::Center,
                        vec![ActionItem::new(action.clone(), label, *icon).presentation()],
                    ));
                }
                children
            }
        };
        if is_mobile
            && matches!(
                self,
                Input::Currency { .. }
                    | Input::Number { .. }
                    | Input::Date { .. }
                    | Input::Time { .. }
            )
        {children.push(theme.keypad());}
        children
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
            | Input::Selection(..) => Offset::Start,
            Input::QRCodeScanner { .. } => Offset::Center,
        }
    }

    pub fn state(child: &dyn Drawable) -> Option<State> {
        child.downcast_ref::<Control>().map(Control::state)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormValidState {
    InvalidWithData(String),
    Invalid,
    Valid,
    ValidWithData(String),
}

impl FormValidState {
    pub fn eval(&self) -> Result<String, String> {
        match self {
            FormValidState::InvalidWithData(s) => Err(s.to_string()),
            FormValidState::Invalid => Err(String::new()),
            FormValidState::Valid => Ok(String::new()),
            FormValidState::ValidWithData(s) => Ok(s.to_string()),
        }
    }

    pub fn from(r: Result<String, String>) -> Self {
        match r {
            Ok(e) if e.is_empty() => FormValidState::Valid,
            Ok(e) => FormValidState::ValidWithData(e),
            Err(e) if e.is_empty() => FormValidState::Invalid,
            Err(e) => FormValidState::InvalidWithData(e),
        }
    }
}

#[derive(Debug, Clone)]
pub enum FormAction {
    Paste,
    ScanQRCode(String),
    Custom(ActionItem),
}

impl FormAction {
    pub fn scan_qr_code(instructions: &str) -> Self {
        FormAction::ScanQRCode(instructions.to_string())
    }

    pub fn get(&self, id: Uuid) -> ActionItem {
        match self {
            FormAction::Paste => ActionItem::new_with_active(
                Action::Paste { id }, "Paste clipboard", "Pasted", Icons::Paste,
            ),
            FormAction::ScanQRCode(instructions) => {
                ActionItem::new(Action::scan_qr(instructions, id), "Scan QR code", Icons::QrCode)
            }
            FormAction::Custom(action) => action.clone(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct FormItem {
    pub id: String,
    pub title: String,
    pub input: Input,
    pub validation: Box<dyn ValidationFn>,
}

impl FormItem {
    pub fn contact(invitations: &ContactInvitations, excluded: Vec<Name>, allow_self: bool, input_id: Uuid) -> Self {
        let invitations = invitations.clone();
        let valid = Box::new(move |ctx: &mut Context, value: String| {
            if value.trim().is_empty() {return FormValidState::Invalid;}
            match invitations.validate_contact(ctx, &value, &excluded, allow_self) {
                Ok(_) => FormValidState::Valid,
                Err(error) => FormValidState::InvalidWithData(error.into()),
            }
        });
        let actions = vec![
            FormAction::Paste, FormAction::ScanQRCode("Scan an AirName QR code".into()),
        ].into_iter().map(|action| action.get(input_id)).collect();
        FormItem {
            id: "airname".into(), title: "New contact".into(),
            input: Input::text(input_id, "AirName", false, None, Some(actions)),
            validation: Box::new(InputValidation(valid)),
        }
    }

    pub fn ids(items: &[Self]) -> Vec<String> {
        let mut seen = HashSet::new();
        items.iter().map(|item| {
            assert!(
                !item.id.trim().is_empty() && seen.insert(item.id.clone()),
                "Empty or duplicate form field ID: {}", item.id,
            );
            item.id.clone()
        }).collect()
    }

    pub fn text(id: &str, input_id: Uuid, title: &str, request: &str, actions: Option<Vec<FormAction>>, valid: impl ValidityFn + 'static) -> Self {
        let actions = actions.map(|actions| {
            actions.into_iter().map(|action| action.get(input_id)).collect()
        });
        FormItem {
            id: id.into(), title: title.into(),
            input: Input::text(input_id, request, false, None, actions),
            validation: Box::new(InputValidation(Box::new(valid))),
        }
    }

    pub fn text_with_preset(
        id: &str, input_id: Uuid, title: &str, preset: &str,
        actions: Option<Vec<FormAction>>, valid: impl ValidityFn + 'static,
    ) -> Self {
        let actions = actions.map(|actions| {
            actions.into_iter().map(|action| action.get(input_id)).collect()
        });
        FormItem {
            id: id.into(), title: title.into(),
            input: Input::text(input_id, title, true, Some(preset.into()), actions),
            validation: Box::new(InputValidation(Box::new(valid))),
        }
    }

    pub fn number(id: &str, title: &str, number: NumberVariant, valid: impl ValidityFn + 'static) -> Self {
        let input = match number {
            NumberVariant::Currency => Input::currency("Enter dollar amount"),
            NumberVariant::Date => Input::date("Enter date"),
            NumberVariant::Time => Input::time("Enter time"),
            NumberVariant::Number => Input::number("Enter number"),
        };
        FormItem {
            id: id.into(), title: title.into(), input,
            validation: Box::new(InputValidation(Box::new(valid))),
        }
    }

    pub fn enumerator(id: &str, title: &str, items: Vec<(&str, &str)>) -> Self {
        FormItem {
            id: id.into(), title: title.into(),
            input: Input::enumerator(items.into_iter().map(|(a, b)| ValueItem::new(a, b)).collect()),
            validation: Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        }
    }

    pub fn selection(id: &str, title: &str, selector: Selector) -> Self {
        FormItem {
            id: id.into(), title: title.into(), input: Input::selection(selector),
            validation: Box::new(|_: &mut Context, children: Vec<&mut Box<dyn Drawable>>| {
                children.first().and_then(|child| Input::state(child.as_ref())).is_some_and(
                    |state| matches!(state, State::Selection(keys) if !keys.is_empty()),
                )
            }),
        }
    }

    pub fn avatar(id: &str, title: &str, action: Action, valid: impl ValidityFn + 'static) -> Self {
        FormItem {
            id: id.into(), title: title.into(),
            input: Input::avatar(
                AvatarContent::default(), Some((Icons::Edit, AvatarIconStyle::Secondary)), Some(action),
            ),
            validation: Box::new(InputValidation(Box::new(valid))),
        }
    }

    pub fn avatar_with_preset(id: &str, title: &str, avatar: AvatarContent, action: Action, valid: impl ValidityFn + 'static) -> Self {
        FormItem {
            id: id.into(), title: title.into(),
            input: Input::avatar(avatar, Some((Icons::Edit, AvatarIconStyle::Secondary)), Some(action)),
            validation: Box::new(InputValidation(Box::new(valid))),
        }
    }

    pub fn scan_qr_code(id: &str, title: &str, instructions: &str, alt: Option<(String, Icons, Action)>) -> Self {
        FormItem {
            id: id.into(), title: title.into(),
            input: Input::qr_code_scanner(
                instructions, alt, Box::new(|_: &mut Context, _: String| {}),
            ),
            validation: Box::new(|_: &mut Context, children: Vec<&mut Box<dyn Drawable>>| {
                children.first().and_then(|child| Input::state(child.as_ref())).is_some_and(
                    |state| matches!(state, State::ScanCode(Some(value)) if !value.is_empty()),
                )
            }),
        }
    }
}

#[derive(Clone)]
struct InputValidation(Box<dyn ValidityFn>);
impl Validate<Vec<&mut Box<dyn Drawable>>, bool> for InputValidation {
    fn validate(&mut self, ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>) -> bool {
        let Some(child) = children.first_mut() else {return false;};
        let Some(control) = child.downcast_mut::<crate::Control>() else {return true;};
        let value = match control.state() {
            State::Text(value) | State::Number(value) => value,
            State::Avatar(value) => value.get_image().unwrap_or_default().to_string(),
            _ => panic!("This validator requires text, number, or avatar input"),
        };
        let result = self.0.validate(ctx, value);
        control.0.feedback(&result);

        matches!(result, FormValidState::Valid | FormValidState::ValidWithData(_))
    }
    fn change(&mut self) -> Pin<Box<dyn Future<Output = Option<()>> + '_>> {self.0.change()}
}
