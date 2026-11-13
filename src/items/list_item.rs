use super::*;
use crate::design_system::Color;

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
    pub fn plain(title: &str, subtitle: &str, secondary: Option<&str>, action: Option<Action>) -> Self {
        ListItem {
            avatar: None,
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()),
            action, badge: false, flair: None,
        }
    }

    pub fn avatar(avatar: AvatarContent, title: &str, subtitle: &str, secondary: Option<&str>, action: Option<Action>) -> Self {
        ListItem {
            avatar: Some(avatar),
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()),
            action, badge: false, flair: None,
        }
    }

    pub fn with_badge(mut self, badge: bool) -> Self {self.badge = badge; self}
    pub fn with_flair(mut self, flair: Option<(Icons, Color)>) -> Self {self.flair = flair; self}

    pub fn build(&self, theme: &Theme) -> ListItemView {theme.list_item(self)}

    pub fn recipient(ctx: &mut Context, theme: &Theme, profile: &Profile) -> Option<Self> {
        let name = profile.name?;
        if Block::am_i_blocked(ctx, name) {return None;}
        let blocked = Block::did_i_block(ctx, name);
        let subtitle = if blocked {"You blocked this user".to_string()} else {profile.name()};
        let title = if name == ctx.air.me() {format!("{} (You)", profile.username)}
            else {profile.username.clone()};
        Some(Self {
            avatar: Some(profile.avatar.clone()), title, subtitle, secondary: None, action: None,
            badge: false, flair: blocked.then(|| (Icons::Block, theme.colors().get(ptsd::Status::Danger))),
        })
    }

    pub fn profile_row(profile: &Profile, me: Name) -> Option<(ListItem, String)> {
        let name = profile.name?;
        let username = if name == me {
            format!("{} (You)", profile.username)
        } else {
            profile.username.clone()
        };
        Some((
            ListItem {
                avatar: Some(profile.avatar.clone()),
                title: username,
                subtitle: profile.name(),
                secondary: None,
                action: None,
                badge: false,
                flair: None,
            },
            name.to_string(),
        ))
    }
}
