use prism::air::{Id, Instance, Name};
use prism::drawable::{Component, Drawable, text::Align};
use prism::events::{ChildEvent, DynEvent, Event, OnChange, OnEvent, on_change, on_event};
use prism::hardware::{Data, Image, Picked};
use ptsd::layout::{Offset, Stack};
use uuid::Uuid;

use crate::blocking::{Block, BlockAction};
use crate::design_system::{
    Callback, Context, FormEvent, Icons, ListItemView, MessageGroups, MessageView, QrCodeFound,
    TextInputEvent, TextSize, TextStyle, Theme,
};
pub use crate::design_system::{AvatarContent, AvatarIconStyle, Number};
use crate::flow::Flow;
use crate::form::State;
use crate::messages::ChatRoom;
use crate::profiles::Profile;
use crate::{ContactInvitations, Page};

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
        Self {title: title.to_string(), data: data.to_string()}
    }
}

/// Page content keeps editable controls and presentation explicitly distinct.
#[derive(Debug, Clone)]
pub enum Item {
    Input(Input),
    Display(Display),
}

impl Item {
    pub fn build(&mut self, theme: &Theme, is_mobile: bool) -> Option<Vec<Box<dyn Drawable>>> {
        match self {
            Item::Input(input) => input.build(theme, is_mobile),
            Item::Display(display) => display.build(theme),
        }
    }
}

#[derive(Debug, Clone)]
struct PickPhoto(Id);
impl Event for PickPhoto {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children { *child.event = Some(self.clone()); }
    }
}
