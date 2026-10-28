use pelican_ui::Context;
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::drawable::Drawable;
use pelican_ui::components::avatar::{AvatarContent, AvatarIconStyle};
use pelican_ui::navigation::NavigationEvent;

use pelican_ui::components::{SearchBar, TextInput, NumericalInput};
use ptsd::Validate;
use pelican_ui::utils::ValidationFn;

use prism::air::Name;
use uuid::Uuid;

use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

use crate::{Flow, Page, Review, Success};
use crate::items::{EnumItem, Input, ListItem, Action, Display};
use crate::closure::{FormSubmit, FormClosure, ValidityFn};

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Text(String),
    Enumerator(usize),
    Number(String),
    Avatar(AvatarContent),
    Search(Vec<Name>),
    ScanCode(Option<String>),
}

#[derive(Clone, Debug)]
pub enum NumberVariant {
    Currency,
    Date,
    Time,
    Number,
}

#[derive(Debug, Clone)]
pub enum Form {
    Flow {
        theme: Box<Theme>,
        inputs: Vec<FormItem>,
        review: Option<Review>,
        success: Option<Success>,
        on_submit: Box<dyn FormSubmit>,
    },
    Page {
        title: String,
        inputs: Vec<FormItem>,
        display: Vec<Display>,
        on_save: Box<dyn FormSubmit>,
    }
}

impl Form {
    pub fn flow(theme: &Theme, inputs: Vec<FormItem>, review: Option<Review>, success: Option<Success>, on_submit: Box<dyn FormSubmit>) -> Self {
        Form::Flow {
            inputs,
            theme: Box::new(theme.clone()),
            review,
            success,
            on_submit,
        }
    }

    pub fn page(title: &str, inputs: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        Form::Page {
            title: title.to_string(),
            inputs,
            display,
            on_save,
        }
    }
}



#[derive(Debug, Clone)]
pub enum FormComplete {
    Next(Page),
    Return(Box<dyn pelican_ui::Callback>),
    None,
}

impl FormComplete {
    pub fn run(&mut self, ctx: &mut Context, theme: &Theme) {
        match self {
            FormComplete::Return(function) => {
                let theme = theme.clone();
                let function = function.clone();
                ctx.emit(NavigationEvent::reset_with_fn(move |ctx: &mut Context| (function.clone())(ctx, &theme.clone())));
            },
            FormComplete::Next(page) => {
                let flow = Flow::new(vec![page.clone()]).build_as_flow(ctx, theme);
                ctx.emit(NavigationEvent::restart(flow));
            },
            FormComplete::None => {}
        }
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
    Custom(String, Option<String>, Icons, Action)
}

impl FormAction {
    pub fn scan_qr_code(instructions: &str) -> Self {
        FormAction::ScanQRCode(instructions.to_string())
    }

    pub fn get(&self, id: Uuid) -> (String, Option<String>, Icons, Action) {
        match self {
            FormAction::Paste => (
                "Paste clipboard".to_string(), Some("Pasted".to_string()), Icons::Paste, Action::Paste {id},
            ),
            FormAction::ScanQRCode(instructions) => (
                "Scan QR code".to_string(), None, Icons::QrCode, Action::scan_qr(instructions, id),
            ),
            FormAction::Custom(l, al, i, a) => (l.to_string(), al.clone(), *i, a.clone())
        }
    }
}
#[derive(Debug, Clone)]
pub enum FormItem {
    Text(String, String, Box<dyn FormClosure>, Option<Vec<FormAction>>, Box<dyn ValidityFn>),
    TextWithPreset(String, String, Box<dyn FormClosure>, Option<Vec<FormAction>>, Box<dyn ValidityFn>),
    Number(String, NumberVariant, Box<dyn ValidityFn>),
    Enum(String, Vec<EnumItem>),
    Search(String, String, Vec<(ListItem, Name)>, Option<Vec<FormAction>>),
    ScanQR(String, String, Option<(String, Icons, Action)>),
    Avatar(String, AvatarContent, Action, Box<dyn ValidityFn>),
    AvatarWithPreset(String, AvatarContent, Action, Box<dyn ValidityFn>),
}

pub struct FormStorage(pub HashMap<String, String>);

impl FormItem {
    pub fn text(text: &str, request: &str, actions: Option<Vec<FormAction>>, valid: impl ValidityFn + 'static) -> Self {
        let text = text.to_string();
        FormItem::Text(text.to_string(), request.to_string(), Box::new(
            move |storage: &mut FormStorage, value: String| {storage.0.insert(text.to_string(), value);}
        ), actions, Box::new(valid))
    }

    pub fn text_with_preset(text: &str, preset: &str, actions: Option<Vec<FormAction>>, valid: impl ValidityFn + 'static) -> Self {
        let text = text.to_string();
        FormItem::TextWithPreset(text.to_string(), preset.to_string(), Box::new(
            move |storage: &mut FormStorage, value: String| {storage.0.insert(text.to_string(), value);}
        ), actions, Box::new(valid))
    }

    pub fn number(title: &str, number: NumberVariant, valid: impl ValidityFn + 'static) -> Self {
        FormItem::Number(title.to_string(), number, Box::new(valid))
    }

    pub fn enumerator(label: &str, items: Vec<(&str, &str)>) -> Self {
        let items = items.into_iter().map(|(a, b)| {
            EnumItem::new(a, b)
        }).collect::<Vec<EnumItem>>();
        FormItem::Enum(label.to_string(), items)
    }

    pub fn search(title: &str, search: &str, items: Vec<(ListItem, Name)>, actions: Option<Vec<FormAction>>) -> Self {
        FormItem::Search(title.to_string(), search.to_string(), items, actions)
    }

    pub fn avatar(title: &str, action: Action, valid: impl ValidityFn + 'static) -> Self {
        FormItem::Avatar(title.to_string(), AvatarContent::default(), action, Box::new(valid))
    }

    pub fn avatar_with_preset(title: &str, avatar: AvatarContent, action: Action, valid: impl ValidityFn + 'static) -> Self {
        FormItem::AvatarWithPreset(title.to_string(), avatar, action, Box::new(valid))
    }

    pub fn scan_qr_code(title: &str, instructions: &str, alt: Option<(String, Icons, Action)>) -> Self {
        FormItem::ScanQR(title.to_string(), instructions.to_string(), alt)
    }

    pub fn title(&self) -> String {
        match self {
            FormItem::Search(title, ..) |
            FormItem::Text(title, ..) |
            FormItem::TextWithPreset(title, ..) |
            FormItem::Number(title, ..) |
            FormItem::Avatar(title, ..) |
            FormItem::AvatarWithPreset(title, ..) |
            FormItem::ScanQR(title, ..) |
            FormItem::Enum(title, ..) => title.to_string()
        }
    }

    pub fn validation(&self) -> Box<dyn ValidationFn> {
        match self {
            FormItem::Text(_, _, _, _, validation) | FormItem::TextWithPreset(_, _, _, _, validation) |
            FormItem::Number(_, _, validation) | FormItem::Avatar(_, _, _, validation) |
            FormItem::AvatarWithPreset(_, _, _, validation) => Box::new(InputValidation(validation.clone())),
            FormItem::Search(_, _, _, _) => Box::new(|_ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
                if let Some(searchbar) = children[0].downcast_mut::<SearchBar>() {!searchbar.results().is_empty()} else {true}
            }),
            _ => Box::new(|_ctx: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        }
    }

    pub fn build(&self) -> Input {
        match self {
            FormItem::Text(_, request, _, actions, _) => {
                let id = Uuid::new_v4();
                let request = request.clone();
                let actions = actions.clone().map(|act| act.into_iter().map(|a| a.get(id)).collect::<Vec<_>>());
                Input::text(id, &request, false, None, actions)
            },
            FormItem::TextWithPreset(title, preset, _, actions, _) => {
                let id = Uuid::new_v4();
                let title = title.clone();
                let actions = actions.clone().map(|act| act.into_iter().map(|a| a.get(id)).collect::<Vec<_>>());
                Input::text(id, &title, true, Some(preset.to_string()), actions)
            },
            FormItem::Number(_, variant, _) => {
                match variant {
                    NumberVariant::Currency => Input::currency("Enter dollar amount"),
                    NumberVariant::Date => Input::date("Enter date"),
                    NumberVariant::Time => Input::time("Enter time"),
                    NumberVariant::Number => Input::number("Enter number"),
                }
            },
            FormItem::Enum(_, items) => {
                Input::enumerator(items.clone())
            },
            FormItem::Search(_, search, items, actions) => {
                let id = Uuid::new_v4();
                let actions = actions.clone().map(|act| act.into_iter().map(|a| a.get(id)).collect::<Vec<_>>());
                Input::search(id, search, items.clone(), actions)
            },
            FormItem::ScanQR(_, instructions, alt) => Input::qr_code_scanner(
                instructions, alt.clone(), Box::new(|_ctx: &mut Context, _value: String| {}),
            ),
            FormItem::Avatar(_, _, action, _) => Input::avatar(
                AvatarContent::default(), Some((Icons::Edit, AvatarIconStyle::Secondary)), Some(action.clone()),
            ),
            FormItem::AvatarWithPreset(_, avatar, action, _) => Input::avatar(
                avatar.clone(), Some((Icons::Edit, AvatarIconStyle::Secondary)), Some(action.clone()),
            ),
        }
    }
}

#[derive(Clone)]
struct InputValidation(Box<dyn ValidityFn>);
impl Validate<Vec<&mut Box<dyn Drawable>>, bool> for InputValidation {
    fn validate(&mut self, ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>) -> bool {
        let result = if let Some(input) = children[0].downcast_mut::<TextInput>() {
            let result = self.0.validate(ctx, input.value());
            input.error(result.eval().map(|_| ()));
            result
        } else if let Some(input) = children[0].downcast_mut::<NumericalInput>() {
            let result = self.0.validate(ctx, input.value());
            input.error(result.eval());
            result
        } else if let Some(avatar) = children[0].downcast_ref::<crate::items::AvatarInput>() {
            self.0.validate(ctx, avatar.content().get_image().unwrap_or_default().to_string())
        } else {return true};
        matches!(result, FormValidState::Valid | FormValidState::ValidWithData(_))
    }
    fn change(&mut self) -> Pin<Box<dyn Future<Output = Option<()>> + '_>> {self.0.change()}
}
