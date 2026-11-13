use crate::design_system::{Context, Callback};
use prism::drawable::Drawable;
use prism::drawable::{Component, text::Align};
use prism::events::{on_change, on_event, ChildEvent, DynEvent, Event, OnEvent, OnChange};
use prism::hardware::{Image, Picked};
use ptsd::layout::Stack;
use uuid::Uuid;
use crate::design_system::QrCodeFound;
use prism::hardware::Data;
use crate::design_system::{Theme, Icons};
use ptsd::layout::Offset;
use crate::design_system::ListItemView;
pub use crate::design_system::Number;
use crate::design_system::{TextStyle, TextSize};
pub use crate::design_system::{AvatarContent, AvatarIconStyle};
use crate::design_system::TextInputEvent;
use crate::design_system::FormEvent;
use crate::design_system::{MessageGroups, MessageView};
use prism::air::{Id, Name, Instance};
use crate::profiles::Profile;
use crate::blocking::{Block, BlockAction};
use crate::Page;
use crate::form::State;
use crate::flow::Flow;
use crate::ContactInvitations;
use crate::messages::ChatRoom;

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

/// Page content keeps editable controls and presentation explicitly distinct.
#[derive(Debug, Clone)]
pub enum Item {
    Input(Input),
    Display(Display),
}

impl Item {
    pub fn build(&mut self, theme: &Theme, is_mobile: bool) -> Option<Vec<Box<dyn Drawable>>> {
        match self {
            Self::Input(input) => input.build(theme, is_mobile),
            Self::Display(display) => display.build(theme),
        }
    }
}

/// A labeled value shared by tables and enumerated choices.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueItem {
    pub(crate) title: String,
    pub(crate) data: String,
}
impl ValueItem {
    pub fn new(title: &str, data: &str) -> Self {
        Self {title: title.to_string(), data: data.to_string()}
    }
    pub(crate) fn get(&self) -> (&str, &str, Box<dyn Callback>) {
        (&self.title, &self.data, Box::new(|_, _| {}))
    }
}

#[derive(Debug, Clone)]
struct PickPhoto(Id);
impl Event for PickPhoto {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children { *child.event = Some(self.clone()); }
    }
}
