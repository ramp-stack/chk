use crate::messages::{ContactNotificationState, MessageContent, RoomNotificationState};
use super::*;

/// Ready-to-render action data; CHK owns labels and callbacks.
#[derive(Clone, Debug)]
pub struct ActionPresentation {
    pub label: String,
    pub active: Option<String>,
    pub icon: Icons,
    pub on_click: Box<dyn Callback>,
}

#[derive(Debug, Clone)]
pub enum Action {
    Share { data: String },
    SelectImage,
    Custom { action: Box<dyn Callback> },
    None,
    Flow { flow: Flow },
    Paste { id: Uuid },
    SetText { id: Uuid, value: String },
    Copy { data: String },
    PickPhoto,
}

impl PartialEq for Action {
    fn eq(&self, _: &Self) -> bool { true }
}

impl Action {
    pub fn set_text(id: Uuid, value: impl Into<String>) -> Self {
        Action::SetText { id, value: value.into() }
    }

    pub fn share(data: &str) -> Self { Action::Share { data: data.to_string() } }

    pub fn copy(data: &str) -> Self { Action::Copy { data: data.to_string() } }

    pub fn select_image() -> Self { Action::SelectImage }

    pub fn scan_qr(instructions: &str, id: Uuid) -> Self {
        let on_find = Box::new(move |ctx: &mut Context, value: String| {
            ctx.emit(TextInputEvent::Set { id, value });
        });
        Action::Flow {
            flow: Flow::new(vec![Page::Static(Box::new(crate::PageType::scan_qr(
                instructions, None, on_find,
            )))]),
        }
    }

    pub fn custom(action: impl Callback + 'static) -> Self {
        Action::Custom { action: Box::new(action) }
    }

    pub fn flow(flow: Flow) -> Self { Action::Flow { flow } }

    pub fn deferred_flow(mut build: impl FnMut(&mut Context, &Theme) -> Flow + Clone + 'static) -> Self {
        Action::Custom {action: Box::new(move |ctx: &mut Context, theme: &Theme| {
            (build(ctx, theme).build(ctx, theme))(ctx, theme);
        })}
    }

    pub fn block(profile: Instance<Profile>) -> Action {
        let profile = profile.pending();
        let name = profile.name.unwrap();
        Action::Flow {flow: Flow::action_target(
            ("block", Action::Custom {action: Box::new(move |ctx: &mut Context, _: &Theme| {
                Block::create(ctx, name).send(BlockAction::Block);
            })}),
            "blocked", "user", &profile.username, profile.avatar.clone(),
            AvatarPurpose::new(Icons::Block, AvatarIconStyle::Danger)
        )}
    }

    pub fn unblock(profile: Instance<Profile>) -> Action {
        let profile = profile.pending();
        let name = profile.name.unwrap();
        Action::Flow {flow: Flow::action_target(
            ("unblock", Action::Custom {action: Box::new(move |ctx: &mut Context, _: &Theme| {
                Block::create(ctx, name).send(BlockAction::Unblock);
            })}),
            "unblocked", "user", &profile.username, profile.avatar.clone(),
            AvatarPurpose::new(Icons::Unblock, AvatarIconStyle::Success)
        )}
    }

    pub fn mute(ctx: &mut Context, room: &Instance<ChatRoom>, muted: bool) -> Action {
        let data = room.pending();
        let (kind, target, avatar) = if data.is_group() {
            let details = data.group_details();
            ("group", details.name, details.photo)
        } else {
            let me = ctx.air.me();
            let name = data.members.iter().copied().find(|name| *name != me).unwrap_or(me);
            let profile = Profile::from_name(ctx, name);
            let profile = profile.pending();
            let target = if name == me {format!("{} (You)", profile.username)} else {profile.username.clone()};
            (if name == me {"chat"} else {"user"}, target, profile.avatar.clone())
        };
        let room = room.clone();
        Action::mute_target(kind, &target, avatar, muted, move |ctx: &mut Context, _: &Theme| {
            RoomNotificationState::set_muted(ctx, &room, muted);
        })
    }

    pub fn mute_profile(ctx: &Context, profile: &Instance<Profile>, muted: bool) -> Action {
        let profile = profile.pending();
        let name = profile.name.unwrap();
        let is_me = name == ctx.air.me();
        let target = if is_me {format!("{} (You)", profile.username)} else {profile.username.clone()};
        Action::mute_target(if is_me {"chat"} else {"user"}, &target, profile.avatar.clone(), muted,
            move |ctx: &mut Context, _: &Theme| {
                ContactNotificationState::set_muted(ctx, name, muted);
            })
    }

    fn mute_target(kind: &str, target: &str, avatar: AvatarContent, muted: bool,
        on_confirm: impl Callback + 'static) -> Action {
        let (label, past_action, icon, style) = if muted {
            ("mute", "muted", Icons::Mute, AvatarIconStyle::Danger)
        } else {
            ("unmute", "unmuted", Icons::Unmute, AvatarIconStyle::Success)
        };
        Action::Flow {flow: Flow::action_target(
            (label, Action::custom(on_confirm)),
            past_action, kind, target, avatar, AvatarPurpose::new(icon, style)
        )}
    }

    pub fn get(&self) -> Box<dyn Callback> {
        match self {
            Action::Share { data } => {
                let data = data.clone();
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    ctx.hardware.share(&Data::Text(data.clone()))
                })
            }

            Action::SelectImage => {
                Box::new(move |_ctx: &mut Context, _: &Theme| println!("Selecting image"))
            }

            Action::Custom { action } => {
                let mut action = action.clone();
                Box::new(move |ctx: &mut Context, theme: &Theme| (action)(ctx, theme))
            }

            Action::Flow { flow } => {
                let mut flow = flow.clone();
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    flow.build(ctx, theme)(ctx, theme);
                })
            }

            Action::SetText { id, value } => {
                let (id, value) = (*id, value.clone());
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    ctx.emit(TextInputEvent::Set { id, value: value.clone() });
                })
            }

            Action::Paste { id } => {
                let id = *id;
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    let text = match ctx.hardware.get_clipboard() {
                        Some(Data::Text(text)) => text, _ => String::new(),
                    };
                    ctx.emit(TextInputEvent::Set { id, value: text });
                })
            }

            Action::Copy { data } => {
                let data = data.to_string();
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    ctx.hardware.set_clipboard(&Data::Text(data.clone()))
                })
            }

            _ => Box::new(move |_ctx: &mut Context, _: &Theme| {}),
        }
    }

    pub fn message(name: Name) -> Action {
        Action::Custom {action: Box::new(move |ctx: &mut Context, theme: &Theme| {
            let mut room = ChatRoom::create(ctx, vec![name]);
            let mut flow = Flow::new(vec![Page::messaging(ctx, &mut room)]);
            (flow.build(ctx, theme))(ctx, theme);
        })}
    }

    pub fn share_contact(invitations: &ContactInvitations, name: Name) -> Action {
        Action::Share {data: invitations.contact_url(name)}
    }

}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionItem(Action, String, Option<String>, Icons);
impl ActionItem {
    pub fn format_label(label: &str) -> String {
        let mut output = String::with_capacity(label.len());
        let mut first_word = true;
        for part in label.split_inclusive(char::is_whitespace) {
            let letters = part.chars().filter(|character| character.is_alphabetic()).count();
            let capitalize = first_word || letters > 3;
            let mut first_letter = true;
            for character in part.chars() {
                if first_letter && character.is_alphabetic() {
                    first_letter = false;
                    first_word = false;
                    if capitalize {
                        output.extend(character.to_uppercase());
                        continue;
                    }
                }
                output.push(character);
            }
        }
        output
    }

    pub fn new(action: Action, label: &str, icon: Icons) -> Self {
        ActionItem(action, label.to_string(), None, icon)
    }

    pub fn new_with_active(action: Action, label: &str, active: &str, icon: Icons) -> Self {
        ActionItem(action, label.to_string(), Some(active.to_string()), icon)
    }

    pub fn mute(ctx: &mut Context, room: Instance<ChatRoom>) -> Self {
        let muted = RoomNotificationState::is_muted_for(&ctx.air, &room);
        let action = Action::mute(ctx, &room, !muted);
        let (label, icon) = if muted {("Unmute", Icons::Unmute)} else {("Mute", Icons::Mute)};
        Self(action, label.into(), None, icon)
    }

    pub fn mute_profile(ctx: &Context, profile: &Instance<Profile>) -> Self {
        let name = profile.pending().name.unwrap();
        let muted = ContactNotificationState::is_muted_for(&ctx.air, name);
        let action = Action::mute_profile(ctx, profile, !muted);
        let (label, icon) = if muted {("Unmute", Icons::Unmute)} else {("Mute", Icons::Mute)};
        Self(action, label.into(), None, icon)
    }

    pub fn copy_message(ctx: &mut Context, content: &MessageContent) -> Option<Self> {
        let action = match content {
            MessageContent::Text(text) => Action::copy(text),
            MessageContent::Gif(gif) => Action::copy(&gif.url),
            MessageContent::Video(_) => return None,
            MessageContent::Photo(photo) => {
                let photo = photo.clone();
                Action::custom(move |ctx: &mut Context, _: &Theme| {
                    ctx.hardware.set_clipboard(&Data::Image(photo.image()));
                })
            }
            MessageContent::Sticker(sticker) => {
                let sticker = sticker.clone();
                Action::custom(move |ctx: &mut Context, _: &Theme| sticker.copy(ctx))
            }
            MessageContent::System(notice) => {
                let profile = Profile::from_name(ctx, notice.member());
                let text = notice.text_for(ctx.air.me(), &profile.pending().username);
                Action::copy(&text)
            }
        };
        Some(Self(action, "Copy".into(), Some("copied".into()), Icons::Copy))
    }

    pub fn contact_actions(invitations: &ContactInvitations, name: Name) -> Vec<ActionItem> {
        vec![
            ActionItem(Action::copy(&name.to_string()), "Copy".into(), Some("Copied".into()), Icons::Copy),
            ActionItem(Action::flow(Flow::new(vec![Page::Static(Box::new(crate::PageType::qr_page(invitations, name)))])),
                "Display QR Code".into(), None, Icons::QrCode),
            ActionItem(Action::share_contact(invitations, name), "Share".into(), None, Icons::Send),
        ]
    }
    pub fn icon_action(&self) -> (Icons, Box<dyn Callback>) { (self.3, self.0.get()) }

    pub fn presentation(&self) -> ActionPresentation {
        ActionPresentation {label: ActionItem::format_label(&self.1), active: self.2.as_deref().map(ActionItem::format_label),
            icon: self.3, on_click: self.0.get()}
    }
}
