use prism::Context;
use prism::drawable::Drawable;

use crate::design_system::Theme;
pub use crate::design_system::{AvatarContent, AvatarIconStyle, Number};

mod action;
pub use action::*;
mod avatar;
pub use avatar::*;
mod display;
pub use display::*;
mod input;
pub use input::*;
mod list_item;
pub use list_item::*;

/// A labeled value shared by tables and enumerated choices.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueItem {
    pub title: String,
    pub data: String,
}
impl ValueItem {
    pub fn new(title: &str, data: &str) -> Self {
        ValueItem { title: title.to_string(), data: data.to_string() }
    }
}

/// Page content keeps editable controls and presentation explicitly distinct.
#[derive(Debug, Clone)]
pub enum Item {
    Input(Input),
    Display(Display),
}

impl Item {
    pub fn build(&mut self, ctx: &mut Context, theme: &Theme) -> Option<Vec<Box<dyn Drawable>>> {
        match self {
            Item::Input(input) => input.build(theme, ctx.window.is_mobile),
            Item::Display(display) => display.build(ctx, theme),
        }
    }
}
