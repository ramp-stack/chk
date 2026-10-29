#![doc(html_logo_url = "https://raw.githubusercontent.com/ramp-stack/chk/main/logo.png")]

pub use pelican_ui::{Context, utils::{Timestamp, Assets},
    theme::{Theme, Color, Icons}, image};
pub use prism::events::{DynEvent, Event, OnEvent};
pub use ptsd::layout::Offset;
pub use ptsd::{Validate, Validator, Watched};

use pelican_ui::interface::navigation::RootInfo as PelicanRootInfo;

pub mod listener;
pub use listener::*;
pub mod closure;
pub use closure::*;
pub mod flow;
pub use flow::*;
pub mod form;
pub use form::*;
pub mod items;
pub use items::*;
pub mod page;
pub use page::*;
pub mod air;
pub use air::*;

pub struct RootInfo(pub PelicanRootInfo);
impl RootInfo {
    pub fn icon(ctx: &mut Context, theme: &Theme, icon: Icons, label: &str, page: Root) -> RootInfo {
        RootInfo(PelicanRootInfo::icon(icon, label, page.0.build_root(ctx, theme)))
    }

    pub fn avatar(ctx: &mut Context, theme: &Theme, avatar: AvatarContent, label: &str, page: Root) -> RootInfo {
        RootInfo(PelicanRootInfo::avatar(avatar, label, page.0.build_root(ctx, theme)))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ChkTheme {
    Dark(Color),
    Light(Color),
    Auto(Color),
}

impl ChkTheme {
    pub fn to_pelican(self, assets: Assets) -> Theme {
        match self {
            ChkTheme::Dark(c) => Theme::dark(&assets.inner, c),
            ChkTheme::Light(c) => Theme::light(&assets.inner, c),
            ChkTheme::Auto(c) => Theme::from(&assets.inner, c)
        }
    }
}

impl Default for ChkTheme { fn default() -> Self {ChkTheme::Dark(Color::from_hex("#ffdd00", 255))} }

pub trait App: Sized + 'static {
    fn roots(&self, ctx: &mut Context, theme: &Theme) -> Vec<RootInfo>;
    fn theme(&self) -> ChkTheme {ChkTheme::default()}
    /// App-owned configuration, supplied before any GIF picker is created.
    fn giphy_api_key(&self) -> Option<&str> {None}
    fn keyboard(&self, _ctx: &Context, _theme: &Theme) -> Option<Box<dyn prism::drawable::Drawable>> {None}

    fn build(self, ctx: &mut Context, assets: Assets) -> pelican_ui::interface::general::Interface {
        let theme = self.theme().to_pelican(assets);
        if let Some(key) = self.giphy_api_key().filter(|key| !key.trim().is_empty()) {
            theme.gifs().set_api_key(key);
        }
        let keyboard = self.keyboard(ctx, &theme);
        let roots = self.roots(ctx, &theme).into_iter().map(|root| root.0).collect();
        pelican_ui::interface::general::Interface::new(ctx, &theme, roots, keyboard)
    }
}

extern crate self as chk;
