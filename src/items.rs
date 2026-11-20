use prism::Context;
use prism::air::Name;
use prism::drawable::Drawable;
use ptsd::selection::SelectorItem;

use crate::blocking::Block;
use crate::design_system::{Color, Icons, ListItemView, Theme};
use crate::profiles::Profile;
pub use crate::design_system::{AvatarContent, AvatarIconStyle, Number};

mod action;
pub use action::*;
mod avatar;
pub use avatar::*;
mod display;
pub use display::*;
mod input;
pub use input::*;

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
    pub fn build(&self, ctx: &mut Context, theme: &Theme) -> Vec<Box<dyn Drawable>> {
        match self {
            Item::Input(input) => input.build(theme, ctx.window.is_mobile),
            Item::Display(display) => display.build(ctx, theme),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ListItem {
    pub avatar: Option<AvatarContent>,
    pub title: String,
    pub subtitle: String,
    pub secondary: Option<String>,
    pub action: Option<Action>,
    pub badge: bool,
    pub flair: Option<(Icons, Color)>,
}

impl PartialEq for ListItem {
    fn eq(&self, other: &Self) -> bool {
        self.avatar == other.avatar
            && self.title == other.title
            && self.subtitle == other.subtitle
            && self.secondary == other.secondary
            && self.badge == other.badge
            && self.flair == other.flair
    }
}

impl ListItem {
    pub fn plain(title: &str, subtitle: &str, secondary: Option<&str>, action: Option<Action>, badge: bool, flair: Option<(Icons, Color)>) -> Self {
        ListItem {
            avatar: None, title: title.to_string(), subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()), action, badge, flair,
        }
    }

    pub fn avatar(
        avatar: AvatarContent, title: &str, subtitle: &str, secondary: Option<&str>,
        action: Option<Action>, badge: bool, flair: Option<(Icons, Color)>,
    ) -> Self {
        ListItem {
            avatar: Some(avatar), title: title.to_string(), subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()), action, badge, flair,
        }
    }

    pub fn build(&self, theme: &Theme) -> ListItemView {theme.list_item(self)}

    pub fn recipient(ctx: &mut Context, theme: &Theme, profile: &Profile) -> Option<Self> {
        let name = profile.name?;
        if Block::am_i_blocked(ctx, name) {return None;}
        let blocked = Block::did_i_block(ctx, name);
        let subtitle = if blocked { "You blocked this user".to_string() } else { profile.name() };
        let title = if name == ctx.air.me() {
            format!("{} (You)", profile.username)
        } else {profile.username.clone()};
        Some(ListItem {
            avatar: Some(profile.avatar.clone()), title, subtitle, secondary: None, action: None,
            badge: false,
            flair: blocked.then(|| (Icons::Block, theme.colors().get(ptsd::Status::Danger))),
        })
    }

    pub fn profile_row(profile: &Profile, me: Name) -> Option<(ListItem, String)> {
        let name = profile.name?;
        let username = if name == me {
            format!("{} (You)", profile.username)
        } else {profile.username.clone()};
        Some((
            ListItem {
                avatar: Some(profile.avatar.clone()), title: username, subtitle: profile.name(),
                secondary: None, action: None, badge: false, flair: None,
            },
            name.to_string(),
        ))
    }
}

impl From<ListItem> for SelectorItem {
    fn from(item: ListItem) -> SelectorItem {
        SelectorItem {
            title: item.title, subtitle: item.subtitle, avatar: item.avatar,
            secondary: item.secondary, badge: item.badge, flair: item.flair,
        }
    }
}
