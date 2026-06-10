use pelican_ui::Context;
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::drawable::Drawable;
use pelican_ui::components::avatar::{AvatarContent, AvatarIconStyle};
use pelican_ui::components::SearchBar;
use pelican_ui::utils::ValidationFn;

use crate::{Review, Success};
use crate::items::{EnumItem, Input, ListItem, Action, Display};
use crate::closure::{FormSubmit, FormClosure, ValidityFn};

use air::names::Name;

use std::collections::HashMap;
use std::fmt::Debug;


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
}

#[derive(Debug, Clone)]
pub enum Form {
    Flow {
        theme: Theme,
        inputs: Vec<FormItem>,
        review: Option<Review>,
        success: Option<Success>,
        on_submit: Box<dyn FormSubmit>, 
    }, 
    Page {
        theme: Theme,
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
            theme: theme.clone(),
            review,
            success,
            on_submit,
        }
    }

    pub fn page(theme: &Theme, title: &str, inputs: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        Form::Page {
            title: title.to_string(),
            inputs,
            display,
            on_save,
            theme: theme.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormValidState {
    InvalidWithData(String), // Disables continue button, sends out error message
    Invalid, // Disables continue button, with no error message,
    Valid, // Enables continue button
    ValidWithData(String), // Enables continue button, sends out help messsage
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
pub enum FormItem {
    Text(String, Box<dyn FormClosure>, Option<Vec<(String, Icons, Action)>>, Box<dyn ValidityFn>),
    TextWithPreset(String, String, Box<dyn FormClosure>, Option<Vec<(String, Icons, Action)>>, Box<dyn ValidityFn>),
    Number(String, NumberVariant, Box<dyn ValidityFn>),
    Enum(String, Vec<EnumItem>),
    Search(String, Vec<(ListItem, Name)>, Option<Vec<(String, Icons, Action)>>),
    ScanQR(String, String, Option<(String, Icons, Action)>),
    Avatar(String, AvatarContent, Box<dyn ValidityFn>),
    AvatarWithPreset(String, AvatarContent, Box<dyn ValidityFn>),
}

pub struct FormStorage(pub HashMap<String, String>);

impl FormItem {
    pub fn text(text: &str, actions: Option<Vec<(String, Icons, Action)>>, valid: impl ValidityFn + 'static) -> Self {
        let text = text.to_string();
        FormItem::Text(text.to_string(), Box::new(move |storage: &mut FormStorage, value: String| {storage.0.insert(text.to_string(), value);}), actions, Box::new(valid))
    }

    pub fn text_with_preset(text: &str, preset: &str, actions: Option<Vec<(String, Icons, Action)>>, valid: impl ValidityFn + 'static) -> Self {
        let text = text.to_string();
        FormItem::TextWithPreset(text.to_string(), preset.to_string(), Box::new(move |storage: &mut FormStorage, value: String| {storage.0.insert(text.to_string(), value);}), actions, Box::new(valid))
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

    pub fn search(title: &str, items: Vec<(ListItem, Name)>, actions: Option<Vec<(String, Icons, Action)>>) -> Self {
        FormItem::Search(title.to_string(), items, actions)
    }

    pub fn avatar(title: &str, valid: impl ValidityFn + 'static) -> Self {
        FormItem::Avatar(title.to_string(), AvatarContent::default(), Box::new(valid))
    }

    pub fn avatar_with_preset(title: &str, avatar: AvatarContent, valid: impl ValidityFn + 'static) -> Self {
        FormItem::AvatarWithPreset(title.to_string(), avatar, Box::new(valid))
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
            FormItem::Text(_, _, _, validation) | FormItem::TextWithPreset(_, _, _, _, validation) => {
                use pelican_ui::components::TextInput;

                let validation = validation.clone();
                Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
                    if let Some(input) = children[0].as_any_mut().downcast_mut::<TextInput>() {
                        let result = (validation.clone())(ctx, input.value());
                        input.error(result.eval().map(|_| {}));
                        result == FormValidState::Valid
                    } else {
                        true
                    }
                })
            },
            FormItem::Number(_, _, validation) => {
                use pelican_ui::components::NumericalInput;
                
                let validation = validation.clone();
                Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
                    if let Some(input) = children[0].as_any_mut().downcast_mut::<NumericalInput>() {
                        let result = (validation.clone())(ctx, input.value());
                        input.error(result.eval());
                        result == FormValidState::Valid
                    } else {
                        true
                    }
                })
            },
            FormItem::Search(_, _, _) => Box::new(|_ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
                if let Some(searchbar) = children[0].as_any_mut().downcast_mut::<SearchBar>() {!searchbar.results().is_empty()} else {true}
            }),
            FormItem::Avatar(_, _, validation) | FormItem::AvatarWithPreset(_, _, validation) => {
                use pelican_ui::components::avatar::Avatar;
                let validation = validation.clone();
                Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
                    if let Some(avatar) = children[0].as_any_mut().downcast_mut::<Avatar>() {
                        let result = (validation.clone())(ctx, avatar.content.get_image().unwrap_or_default().to_string());
                        result == FormValidState::Valid
                    } else {
                        true
                    }
                })
            }
            _ => Box::new(|_ctx: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        }
    }

    pub fn build(&self) -> Input {
        match self {
            FormItem::Text(title, _, actions, _) => {
                let title = title.clone();
                Input::text(&title, false, None, actions.clone())
            },
            FormItem::TextWithPreset(title, preset, _, actions, _) => {
                let title = title.clone();
                Input::text(&title, true, Some(preset.to_string()), actions.clone())
            },
            FormItem::Number(_, variant, _) => {
                match variant {
                    NumberVariant::Currency => Input::currency("Enter dollar amount"),
                    NumberVariant::Date => Input::date("Enter date"),
                    NumberVariant::Time => Input::time("Enter time"),
                }
            },
            FormItem::Enum(_, items) => {
                Input::enumerator(items.clone())
            },
            FormItem::Search(_, items, actions) => {
                Input::search(items.clone(), actions.clone())
            },
            FormItem::ScanQR(_, instructions, alt) => Input::qr_code_scanner(instructions, alt.clone()),
            FormItem::Avatar(_, _, _) => Input::avatar(AvatarContent::default(), Some((Icons::Edit, AvatarIconStyle::Secondary)), Some(Action::None)),
            FormItem::AvatarWithPreset(_, avatar, _) => Input::avatar(avatar.clone(), Some((Icons::Edit, AvatarIconStyle::Secondary)), Some(Action::None))
        }
    }
}

