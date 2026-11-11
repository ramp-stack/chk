#![doc(html_logo_url = "https://raw.githubusercontent.com/ramp-stack/chk/main/logo.png")]

pub use pelican_ui::{Context, utils::{Timestamp, Assets},
    theme::{Theme, Color, Icons}, image};
pub use prism::events::{DynEvent, Event, OnEvent};
pub use ptsd::layout::Offset;
pub use ptsd::{Validate, Validator, Watched};

use pelican_ui::interface::navigation::RootInfo as PelicanRootInfo;

pub use pelican_ui::components::Selector;

pub mod invitations;
pub use invitations::{Invitations, InvitationLinks, ContactInvitations};

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
#[path = "typing.rs"]
mod typing_ui;

pub struct RootInfo(pub PelicanRootInfo);
impl RootInfo {
    pub fn messages(ctx: &mut Context, theme: &Theme, invitations: &Invitations<prism::air::Name>) -> Self {
        let page = Root::messages(ctx, &ContactInvitations::new(invitations.clone()));
        Self::icon(ctx, theme, Icons::Messages, "Messages", page)
    }

    pub fn contacts(ctx: &mut Context, theme: &Theme, invitations: &Invitations<prism::air::Name>) -> Self {
        Self::icon(ctx, theme, Icons::Group, "Contacts", Root::contacts(&ContactInvitations::new(invitations.clone())))
    }

    pub fn profile(ctx: &mut Context, theme: &Theme, invitations: &Invitations<prism::air::Name>) -> Self {
        let avatar = pelican_ui::components::avatar::AvatarValue::watch(
            profiles::Profile::me(ctx),
            async |profile| {profile.next().await;},
            |profile| profile.pending().avatar.clone(),
        );
        let page = Root::profile(ctx, &ContactInvitations::new(invitations.clone()));
        Self::avatar(ctx, theme, avatar, "Profile", page)
    }

    pub fn icon(ctx: &mut Context, theme: &Theme, icon: Icons, label: &str, page: Root) -> RootInfo {
        RootInfo(PelicanRootInfo::icon(icon, label, page.0.build_root(ctx, theme)))
    }

    pub fn avatar(ctx: &mut Context, theme: &Theme, avatar: impl Into<pelican_ui::components::avatar::AvatarValue>, label: &str, page: Root) -> RootInfo {
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
    type Invitation: Clone + std::fmt::Debug + 'static;

    fn name() -> String;
    fn new(ctx: &mut Context) -> Self;
    fn roots(&self, ctx: &mut Context, theme: &Theme,
        invitations: Option<&Invitations<Self::Invitation>>) -> Vec<RootInfo>;
    fn theme(&self) -> ChkTheme {ChkTheme::default()}
    fn assets(&self) -> Assets {Assets::new(Vec::new())}
    fn keyboard(&self, _ctx: &Context, _theme: &Theme) -> Option<Box<dyn prism::drawable::Drawable>> {None}

    /// Called once during setup. The app supplies its format, parser, and action;
    /// CHK owns native registration, queued delivery, and UI dispatch.
    fn invitations(&self, _ctx: &mut Context) -> Option<Invitations<Self::Invitation>> {None}

    fn build(self, ctx: &mut Context) -> Box<dyn prism::drawable::Drawable> {
        let theme = self.theme().to_pelican(self.assets());
        if let Some(key) = option_env!("GIPHY_API_KEY").filter(|key| !key.trim().is_empty()) {
            theme.gifs().set_api_key(key);
        }
        let invitations = self.invitations(ctx);
        if let Some(invitations) = &invitations {invitations.attach(ctx);}
        let keyboard = self.keyboard(ctx, &theme);
        let roots = self.roots(ctx, &theme, invitations.as_ref()).into_iter().map(|root| root.0).collect();
        let interface = pelican_ui::interface::general::Interface::new(ctx, &theme, roots, keyboard);
        let interface = FormHost::new(interface, theme.clone());
        match invitations {
            Some(invitations) => Box::new(invitations.wrap(interface, theme)),
            None => Box::new(interface),
        }
    }
}

/// Runs a CHK App through Prism without platform-specific setup in the app.
pub struct Application<A>(std::marker::PhantomData<fn() -> A>);
impl<A: App> prism::Application for Application<A> {
    fn name() -> String {A::name()}
    fn new(ctx: &mut Context) -> impl prism::drawable::Drawable {A::new(ctx).build(ctx)}
}

extern crate self as chk;
