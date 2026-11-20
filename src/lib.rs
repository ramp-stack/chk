#![doc(html_logo_url = "https://raw.githubusercontent.com/ramp-stack/chk/main/logo.png")]

pub use prism::{Context, events::{DynEvent, Event, OnEvent}};
use prism::air::Instance;
use prism::drawable::Drawable;
use ptsd::navigation::AppPage;
pub use ptsd::{Validate, Validator, Watched, layout::Offset, utils::{Timestamp, Assets}};

extern crate self as chk;

pub mod design_system;
pub use design_system::{DesignSystem, Theme, Color, Icons, image};
use design_system::AvatarValue;

#[cfg(feature = "pelican_ui")]
pub mod pelican_ui;
#[cfg(feature = "pelican_ui")]
pub use pelican_ui::PelicanUi;

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
pub mod page_type;
pub mod page;
pub use page::*;
pub mod air;
pub use air::*;
use messages::{ChatRoom, RoomReadState};
use profiles::Profile;

pub mod design;
pub use design::{
    InputControl, Control, PageSurface, PageView, AvatarSurface, AvatarView, CameraSurface,
    CameraView, PlaybackSurface, VideoView,
};

#[path = "typing.rs"]
mod typing_ui;

mod pending;

pub enum RootIcon {Icon(Icons), Avatar(AvatarValue)}

pub trait App: Sized + 'static {
    type DesignSystem: crate::DesignSystem;
    type Invitation: Clone + std::fmt::Debug + 'static;

    fn name() -> String;
    fn new(ctx: &mut Context) -> Self;
    fn roots(&self, ctx: &mut Context, theme: &Theme, invitations: Option<&Invitations<Self::Invitation>>) -> Vec<RootInfo>;
    fn theme(&self) -> ChkTheme {ChkTheme::default()}
    fn assets(&self) -> Assets {Assets::new(Vec::new())}
    fn keyboard(&self, _ctx: &Context, _theme: &Theme) -> Option<Box<dyn Drawable>> {None}

    /// Called once during setup. The app supplies its format, parser, and action;
    /// CHK owns native registration, queued delivery, and UI dispatch.
    fn invitations(&self, _ctx: &mut Context) -> Option<Invitations<Self::Invitation>> {None}

    fn build(self, ctx: &mut Context) -> Box<dyn Drawable> {
        let theme = self.theme().build::<<Self as App>::DesignSystem>(self.assets());
        if let Some(key) = option_env!("GIPHY_API_KEY").filter(|key| !key.trim().is_empty()) {
            theme.configure_gifs(key);
        }
        let invitations = self.invitations(ctx);
        if let Some(invitations) = &invitations {invitations.attach(ctx);}
        let keyboard = self.keyboard(ctx, &theme);
        let roots = self.roots(ctx, &theme, invitations.as_ref());
        let interface = theme.interface(ctx, roots, keyboard);
        let interface = FormHost::new(interface, theme.clone());
        match invitations {
            Some(invitations) => Box::new(InvitationLinks::new(interface, theme, invitations)),
            None => Box::new(interface),
        }
    }
}

pub struct RootInfo {
    pub icon: RootIcon,
    pub label: String,
    pub page: Box<dyn AppPage>,
}

impl RootInfo {
    pub fn messages(ctx: &mut Context, theme: &Theme, invitations: &Invitations<prism::air::Name>) -> Self {
        let page = Root::messages(ctx, &ContactInvitations::new(invitations.clone()));
        RootInfo {
            icon: RootIcon::Icon(Icons::Messages), label: "Messages".into(),
            page: page.0.build_root(ctx, theme),
        }
    }

    pub fn contacts(ctx: &mut Context, theme: &Theme, invitations: &Invitations<prism::air::Name>) -> Self {
        let page = Root::contacts(&ContactInvitations::new(invitations.clone()));
        RootInfo {
            icon: RootIcon::Icon(Icons::Group), label: "Contacts".into(),
            page: page.0.build_root(ctx, theme),
        }
    }

    pub fn profile(ctx: &mut Context, theme: &Theme, invitations: &Invitations<prism::air::Name>) -> Self {
        let avatar = AvatarValue::watch(
            Profile::me(ctx),
            async |profile| {profile.next().await;},
            |profile| profile.pending().avatar.clone(),
        );
        let page = Root::profile(ctx, &ContactInvitations::new(invitations.clone()));
        RootInfo {
            icon: RootIcon::Avatar(avatar), label: "Profile".into(), page: page.0.build_root(ctx, theme),
        }
    }

    pub fn icon(ctx: &mut Context, theme: &Theme, icon: Icons, label: &str, page: Root) -> RootInfo {
        RootInfo {icon: RootIcon::Icon(icon), label: label.into(), page: page.0.build_root(ctx, theme)}
    }

    pub fn avatar(ctx: &mut Context, theme: &Theme, avatar: impl Into<AvatarValue>, label: &str, page: Root) -> RootInfo {
        RootInfo {
            icon: RootIcon::Avatar(avatar.into()), label: label.into(),
            page: page.0.build_root(ctx, theme),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ChkTheme {
    Dark(Color),
    Light(Color),
    Auto(Color),
}

impl ChkTheme {
    pub fn build<D: crate::DesignSystem>(self, assets: Assets) -> Theme {
        Theme::new(D::new(self, assets))
    }
}

impl Default for ChkTheme {
    fn default() -> Self {ChkTheme::Dark(Color::from_hex("#ffdd00", 255))}
}

/// Runs a CHK App through Prism without platform-specific setup in the app.
pub struct Application<A>(std::marker::PhantomData<fn() -> A>);
impl<A: App> prism::Application for Application<A> {
    fn name() -> String {A::name()}
    fn new(ctx: &mut Context) -> impl Drawable {A::new(ctx).build(ctx)}
}

pub struct Root(pub Page);
impl Root {
    pub fn new(
        title: &str, items: Vec<Display>, header: Option<(Icons, Action)>,
        bumper_a: Option<(String, Action)>, bumper_b: Option<(String, Action)>,
    ) -> Self {
        Root(Page::Static(Box::new(PageType::root(
            title, vec![], items, header, bumper_a, bumper_b,
        ))))
    }

    pub fn custom(page: Page) -> Self { Root(page) }

    pub fn contacts(invitations: &ContactInvitations) -> Root {
        let invitations = invitations.clone();
        Root(Page::watching(ListWatch::<Profile>::default(),
            move |ctx: &mut Context, _theme: &Theme, profiles: Vec<Instance<Profile>>| {
                let items = SearchSource::contact_items(&invitations, ctx, profiles);
                let search = Action::flow(Flow::new(vec![Page::Static(Box::new(SearchSource::Contacts(ListWatch::<Profile>::default()).page(&invitations)))]));
                let invitations = invitations.clone();
                let new_contact = Action::deferred_flow(move |_ctx: &mut Context, _: &Theme| {
                    Flow::from_form(Form::new_contact(&invitations))
                });
                PageType::root("Contacts", vec![], vec![Display::list(
                    None, items, Some("No contacts yet.\nGet started by adding a friend."),
                )], Some((Icons::Search, search)), Some(("New Contact".into(), new_contact)), None)
            }))
    }

    pub fn profile(ctx: &mut Context, invitations: &ContactInvitations) -> Root {
        let name = ctx.air.me();
        Root(Page::Static(Box::new(Profile::editing(
            true, Profile::me(ctx), ctx.window.is_mobile, Some(vec![
                ActionItem::new_with_active(
                    Action::copy(&name.to_string()), "Copy", "Copied", Icons::Copy,
                ),
                ActionItem::new(Action::flow(Flow::new(vec![Page::Static(Box::new(
                    PageType::qr_page(invitations, name),
                ))])), "Display QR Code", Icons::QrCode),
                ActionItem::new(Action::share_contact(invitations, name), "Share", Icons::Send),
            ]),
        ))))
    }

    pub fn messages(ctx: &mut Context, invitations: &ContactInvitations) -> Root {
        ChatRoom::start_notifications(ctx);
        let invitations = invitations.clone();
        Root(Page::watching((
            ListWatch::<ChatRoom>::default(), ListWatch::<Profile>::default(),
            ListWatch::<RoomReadState>::default(),
        ),
            move |ctx: &mut Context, _theme: &Theme, (rooms, profiles, reads): (Vec<Instance<ChatRoom>>, _, _)| {
                let items = SearchSource::message_items(&invitations, ctx, rooms, profiles, reads);
                let search = Action::flow(Flow::new(vec![Page::Static(Box::new(SearchSource::Messages((
                    ListWatch::<ChatRoom>::default(), ListWatch::<Profile>::default(),
                    ListWatch::<RoomReadState>::default(),
                )).page(&invitations)))]));
                let invitations = invitations.clone();
                let new_message = Action::deferred_flow(move |ctx: &mut Context, theme: &Theme| {
                    Flow::from_form(Form::new_message(&invitations, ctx, theme))
                });
                PageType::root("Messages", vec![], vec![Display::list(
                    None, items, Some("No messages yet.\nGet started by messaging a friend."),
                )], Some((Icons::Search, search)), Some(("New Message".into(), new_message)), None)
            }))
    }
}
