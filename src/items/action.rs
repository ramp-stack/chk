use super::*;
use crate::messages::RoomNotificationState;


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
        Self::Custom {action: Box::new(move |ctx: &mut Context, theme: &Theme| {
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
        let (label, past_action, icon, style) = if muted {
            ("mute", "muted", Icons::Mute, AvatarIconStyle::Danger)
        } else {
            ("unmute", "unmuted", Icons::Unmute, AvatarIconStyle::Success)
        };
        let room = room.clone();
        Action::Flow {flow: Flow::action_target(
            (label, Action::Custom {action: Box::new(move |ctx: &mut Context, _: &Theme| {
                RoomNotificationState::set_muted(ctx, &room, muted);
            })}),
            past_action, kind, &target, avatar, AvatarPurpose::new(icon, style)
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
        Action::Share {data: match &invitations.invitations {
            Some(invitations) => invitations.url(name),
            None => name.to_string(),
        }}
    }

}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionItem(pub(crate) Action, pub(crate) String, pub(crate) Option<String>, pub(crate) Icons);
impl ActionItem {
    pub(crate) fn format_label(label: &str) -> String {
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

    pub fn contact_actions(invitations: &ContactInvitations, name: Name) -> Vec<ActionItem> {
        vec![
            ActionItem(Action::copy(&name.to_string()), "Copy".into(), Some("Copied".into()), Icons::Copy),
            ActionItem(Action::flow(Flow::new(vec![Page::Static(Box::new(crate::PageType::qr_page(invitations, name)))])),
                "Display QR Code".into(), None, Icons::QrCode),
            ActionItem(Action::share_contact(invitations, name), "Share".into(), None, Icons::Send),
        ]
    }
}

/// Ready-to-render action data; CHK owns labels and callbacks.
#[derive(Clone, Debug)]
pub struct ActionPresentation {
    pub label: String,
    pub active: Option<String>,
    pub icon: Icons,
    pub on_click: Box<dyn Callback>,
}
impl ActionItem {
    pub fn presentation(&self) -> ActionPresentation {
        ActionPresentation {label: Self::format_label(&self.1), active: self.2.as_deref().map(Self::format_label),
            icon: self.3, on_click: self.0.get()}
    }
}
