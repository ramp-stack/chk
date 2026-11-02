//! Optional contact presentation helpers for invitations carrying a public name.
use prism::air::Name;
use crate::{Action, ActionItem, Bumper, Context, Display, Flow, Icons, Invitations,
    Offset, Page, PageType, ProfilePage};
use crate::profiles::Profile;

impl Invitations<Name> {
    pub fn contact_url(&self, name: Name) -> String {self.url(name)}

    /// Contact entry and QR scanners also accept a raw public identity.
    pub fn parse_contact(&self, input: &str) -> Option<Name> {
        self.parse_invite(input).or_else(|| input.trim().parse().ok())
    }

    /// The QR still contains the raw public identity for existing scanners.
    /// Its Share action sends the same invitation URL as the profile's Share action.
    pub fn qr_page(&self, name: Name) -> PageType {
        PageType::display(
            "Share name", vec![Display::qr_code(&name.to_string(), "Scan to share this contact.")], None,
            Bumper::custom("Share", self.share_action(name)), Offset::Center,
        )
    }

    pub fn contact_actions(&self, name: Name) -> Vec<ActionItem> {
        vec![
            ActionItem::new_with_active(Action::copy(&name.to_string()), "Copy", "Copied", Icons::Copy),
            ActionItem::new(Action::flow(Flow::new(vec![Page::Static(Box::new(self.qr_page(name)))])),
                "Display QR Code", Icons::QrCode),
            ActionItem::new(self.share_action(name), "Share", Icons::Send),
        ]
    }

    /// The standard editable self profile with invitation actions under its name.
    pub fn my_profile(&self, ctx: &mut Context) -> Page {
        let profile = Profile::me(ctx);
        Page::Static(Box::new(ProfilePage::editing_with_contact_actions(
            true, profile, ctx.window.is_mobile, Some(self.contact_actions(ctx.air.me())),
        )))
    }
}

