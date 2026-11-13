use crate::*;
use crate::design_system::AvatarValue;
use ptsd::navigation::AppPage;

pub enum RootIcon {Icon(Icons), Avatar(AvatarValue)}

pub struct RootInfo {
    pub icon: RootIcon,
    pub label: String,
    pub page: Box<dyn AppPage>,
}

impl RootInfo {
    pub fn messages(ctx: &mut Context, theme: &Theme, invitations: &Invitations<prism::air::Name>) -> Self {
        let page = Root::messages(ctx, &ContactInvitations::new(invitations.clone()));
        Self {icon: RootIcon::Icon(Icons::Messages), label: "Messages".into(), page: page.0.build_root(ctx, theme)}
    }

    pub fn contacts(ctx: &mut Context, theme: &Theme, invitations: &Invitations<prism::air::Name>) -> Self {
        let page = Root::contacts(&ContactInvitations::new(invitations.clone()));
        Self {icon: RootIcon::Icon(Icons::Group), label: "Contacts".into(), page: page.0.build_root(ctx, theme)}
    }

    pub fn profile(ctx: &mut Context, theme: &Theme, invitations: &Invitations<prism::air::Name>) -> Self {
        let avatar = AvatarValue::watch(
            profiles::Profile::me(ctx),
            async |profile| {profile.next().await;},
            |profile| profile.pending().avatar.clone(),
        );
        let page = Root::profile(ctx, &ContactInvitations::new(invitations.clone()));
        Self {icon: RootIcon::Avatar(avatar), label: "Profile".into(), page: page.0.build_root(ctx, theme)}
    }

    pub fn icon(ctx: &mut Context, theme: &Theme, icon: Icons, label: &str, page: Root) -> RootInfo {
        RootInfo {icon: RootIcon::Icon(icon), label: label.into(), page: page.0.build_root(ctx, theme)}
    }

    pub fn avatar(ctx: &mut Context, theme: &Theme, avatar: impl Into<AvatarValue>, label: &str, page: Root) -> RootInfo {
        RootInfo {icon: RootIcon::Avatar(avatar.into()), label: label.into(), page: page.0.build_root(ctx, theme)}
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ChkTheme {
    Dark(Color),
    Light(Color),
    Auto(Color),
}

impl ChkTheme {
    pub fn build<D: crate::DesignSystem>(self, assets: Assets) -> Theme {Theme::new(D::new(self, assets))}
}

impl Default for ChkTheme { fn default() -> Self {ChkTheme::Dark(Color::from_hex("#ffdd00", 255))} }

pub trait App: Sized + 'static {
    type DesignSystem: crate::DesignSystem;
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
        let theme = Theme::new(Self::DesignSystem::new(self.theme(), self.assets()));
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
