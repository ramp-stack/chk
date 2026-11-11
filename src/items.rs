use pelican_ui::{colors, Context, Callback};
use pelican_ui::drawable::Drawable;
use prism::drawable::{Component, text::Align};
use prism::events::{on_change, on_event, ChildEvent, DynEvent, Event, OnEvent, OnChange};
use prism::hardware::{Image, Picked};
use ptsd::layout::Stack;
use uuid::Uuid;
use pelican_ui::components::QrCodeFound;
use prism::hardware::Data;
use pelican_ui::theme::{Theme, Icons};
use ptsd::layout::Offset;
use pelican_ui::utils::{TitleSubtitle};
use pelican_ui::components::list_item::{
    ListItemSection, ListItemInfoLeft, ListItem as PelicanListItem,
};
use pelican_ui::components::{
    QRCodeScanner, TextInput, RadioSelector, Icon, DataItem, QRCode, NumericalInput,
};
pub use pelican_ui::components::Number;
use pelican_ui::components::text::{Text, TextStyle, TextSize};
use pelican_ui::components::avatar::{Avatar, AvatarSize};
pub use pelican_ui::components::avatar::{AvatarContent, AvatarIconStyle};
use pelican_ui::components::button::{QuickActions, IconButtonGroup, ActionData};
use pelican_ui::components::{Keypad, TextInputEvent};
use pelican_ui::components::{Selector, SelectionInput};
use pelican_ui::interface::general::FormEvent;
use pelican_ui::components::{MessageGroups, Message as PelicanMessage};
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
    title: String,
    data: String,
}
impl ValueItem {
    pub fn new(title: &str, data: &str) -> Self {
        Self {title: title.to_string(), data: data.to_string()}
    }
    fn get(&self) -> (&str, &str, Box<dyn Callback>) {
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
