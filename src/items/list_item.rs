use super::*;

#[derive(Debug, Clone)]
pub struct ListItem {
    avatar: Option<AvatarContent>,
    pub title: String,
    subtitle: String,
    secondary: Option<String>,
    action: Option<Action>,
    badge: bool,
}

impl PartialEq for ListItem {
    fn eq(&self, other: &Self) -> bool {
        self.avatar == other.avatar
            && self.title == other.title
            && self.subtitle == other.subtitle
            && self.secondary == other.secondary
            && self.badge == other.badge
    }
}

impl ListItem {
    pub fn plain(title: &str, subtitle: &str, secondary: Option<&str>, action: Option<Action>) -> Self {
        ListItem {
            avatar: None,
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()),
            action, badge: false,
        }
    }

    pub fn avatar(avatar: AvatarContent, title: &str, subtitle: &str, secondary: Option<&str>, action: Option<Action>) -> Self {
        ListItem {
            avatar: Some(avatar),
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()),
            action, badge: false,
        }
    }

    pub fn with_badge(mut self, badge: bool) -> Self {self.badge = badge; self}

    pub fn build(&self, theme: &Theme) -> PelicanListItem {
        let ListItem { avatar, title, subtitle, secondary, action, badge } = self.clone();
        let has_action = action.is_some();
        let closure = action.unwrap_or(Action::None).get();

        let avatar = avatar.map(|content| {
            Avatar::new(theme, content, None, AvatarSize::Md, None)
        });
        PelicanListItem::new(
            theme, avatar, ListItemInfoLeft::new(&title, Some(&subtitle), None, None).with_badge(badge),
            secondary.as_ref().map(|s| TitleSubtitle::new(s, Some("Details"))), None,
            has_action.then_some(Icons::Forward), closure,
        )
    }

    pub fn profile_row(profile: &Profile, me: Name) -> Option<(ListItem, String)> {
        let name = profile.name?;
        let username = if name == me {
            format!("{} (You)", profile.username)
        } else {
            profile.username.clone()
        };
        Some((
            ListItem::avatar(
                profile.avatar.clone(),
                &username,
                &profile.name(),
                None,
                None,
            ),
            name.to_string(),
        ))
    }
}
impl From<ListItem> for pelican_ui::components::SelectorItem {
    fn from(item: ListItem) -> Self {
        Self {title: item.title, subtitle: item.subtitle, avatar: item.avatar, secondary: item.secondary, badge: item.badge}
    }
}
