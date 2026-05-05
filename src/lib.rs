#![doc(html_logo_url = "https://raw.githubusercontent.com/ramp-stack/chk/main/logo.png")]

pub use pelican_ui::{Context, Assets, utils::Timestamp, event::{OnEvent, Event}, layout::Offset, theme::{Theme, Color, Icons}, image};
pub use ramp::prism::IS_MOBILE;

use pelican_ui::interface::navigation::RootInfo as PelicanRootInfo;

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
            ChkTheme::Dark(c) => Theme::dark(&assets.0, c),
            ChkTheme::Light(c) => Theme::light(&assets.0, c),
            ChkTheme::Auto(c) => Theme::from(&assets.0, c)
        }
    }
}

impl Default for ChkTheme { fn default() -> Self {ChkTheme::Dark(Color::from_hex("#ffdd00", 255))} }

pub trait App {
    fn roots(&self, ctx: &mut Context, theme: &Theme) -> Vec<RootInfo>;
    fn theme(&self) -> ChkTheme { ChkTheme::default() }
    fn on_event(&mut self, _ctx: &mut Context, event: Box<dyn Event>) -> Vec<Box<dyn Event>> { vec![event] }
}

#[doc(hidden)]
pub mod __private {
    pub use ramp;
    pub use ramp::prism;
    pub use prism::Assets;
    pub use pelican_ui::theme::Theme;
    pub use pelican_ui::Context;
    pub use pelican_ui::event::Event;
    pub use pelican_ui::interface::{general::Interface, navigation::RootInfo as PelicanRootInfo};
    pub use chk::App;
    pub use chk::ChkTheme;
    pub use chk::RootInfo;
    pub use std::rc::Rc;
    pub use std::cell::RefCell;
}

// example
// chk::run! {[ChatRoom]; |_ctx: &mut Context| Orange }

#[macro_export]
macro_rules! run {
    ([$($c:ty),* $(,)?]; $app:expr) => {
        use $crate::__private::*;

        ramp::run!([$($c),*]; move |ctx: &mut Context, assets: Assets| {
            let app: Rc<RefCell<dyn App>> = Rc::new(RefCell::new(($app)(ctx)));
            let theme: Theme = app.borrow().theme().to_pelican(assets);
            let roots: Vec<RootInfo> = app.borrow().roots(ctx, &theme);
            let roots = roots.into_iter().map(|root| root.0).collect::<Vec<PelicanRootInfo>>();
            let app = Rc::clone(&app);
            let on_event = Box::new(move |d: &mut Box<dyn Drawable>, ctx: &mut Context, event: Box<dyn Event>| {
                app.borrow_mut().on_event(ctx, event)
            });
            Interface::new(ctx, &theme, roots, on_event)
        });
    }
}

extern crate self as chk;