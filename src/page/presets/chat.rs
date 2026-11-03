//! Reusable chat, contacts, and self-profile screens with optional invitation links.
use crate::{Action, ActionItem, Bumper, Context, Display, Flow, Form, FormAction,
    FormComplete, FormItem, FormSubmit, FormValidState, Icons, Invitations, ListItem,
    Offset, Page, PageType, Root, Selector, FormValues, Theme, Watch};
use crate::air::{messages::{ChatRoom, RoomReadState}, profiles::Profile};
use prism::air::{Instance, Name};

type MessageLists = (Vec<Instance<ChatRoom>>, Vec<Instance<Profile>>, Vec<Instance<RoomReadState>>);

/// Standard chat screens. Without invitation configuration, contact entry accepts raw AirNames.
#[derive(Clone, Debug, Default)]
pub struct ChatPresets {
    invitations: Option<Invitations<Name>>,
}

impl ChatPresets {
    pub fn message(name: Name) -> Action {
        Action::deferred_flow(move |ctx, _| {
            let mut room = ChatRoom::create(ctx, vec![name]);
            Flow::new(vec![Page::messaging(ctx, &mut room)])
        })
    }

    pub fn block(profile: Instance<Profile>) -> Action {Self::blocking(profile, true)}
    pub fn unblock(profile: Instance<Profile>) -> Action {Self::blocking(profile, false)}

    fn blocking(profile: Instance<Profile>, blocked: bool) -> Action {
        use crate::{AvatarPurpose, AvatarIconStyle, blocking::{Block, BlockAction}};
        let profile = profile.pending();
        let name = profile.name.unwrap();
        let action = Action::custom(move |ctx: &mut Context, _: &Theme| {
            Block::create(ctx, name).send(if blocked {BlockAction::Block} else {BlockAction::Unblock});
        });
        let (verb, past, icon, style) = if blocked {
            ("block", "blocked", Icons::Block, AvatarIconStyle::Danger)
        } else {("unblock", "unblocked", Icons::Unblock, AvatarIconStyle::Success)};
        Action::flow(Flow::action_target((verb, action), past, &profile.username,
            profile.avatar.clone(), AvatarPurpose::new(icon, style)))
    }

    pub fn new(invitations: Invitations<Name>) -> Self {Self {invitations: Some(invitations)}}

    pub fn profile(&self, ctx: &mut Context) -> Root {
        Root::custom(Page::Static(Box::new(crate::ProfilePage::editing_with_contact_actions(
            true, Profile::me(ctx), ctx.window.is_mobile, Some(self.contact_actions(ctx.air.me())),
        ))))
    }

    /// QR payloads retain the raw AirName; sharing uses the app's invitation URL when configured.
    pub fn qr_page(&self, name: Name) -> PageType {
        PageType::display("Share AirName",
            vec![Display::qr_code(&name.to_string(), "Scan to share this AirName.")], None,
            Bumper::custom("Share", self.share_contact(name)), Offset::Center)
    }

    pub fn contact_actions(&self, name: Name) -> Vec<ActionItem> {
        vec![
            ActionItem::new_with_active(Action::copy(&name.to_string()), "Copy", "Copied", Icons::Copy),
            ActionItem::new(Action::flow(Flow::new(vec![Page::Static(Box::new(self.qr_page(name)))])),
                "Display QR Code", Icons::QrCode),
            ActionItem::new(self.share_contact(name), "Share", Icons::Send),
        ]
    }

    fn share_contact(&self, name: Name) -> Action {
        match &self.invitations {
            Some(invitations) => invitations.share_action(name),
            None => Action::share(&name.to_string()),
        }
    }

    pub fn messages(&self, ctx: &mut Context) -> Root {
        ctx.air.start::<crate::air::notifications::MessageNotifications>(());
        let presets = self.clone();
        Root::custom(Page::watching((Watch::list::<ChatRoom>(), Watch::list::<Profile>(), Watch::list::<RoomReadState>()),
            move |ctx: &mut Context, _theme: &Theme, (rooms, _, reads): MessageLists| {
                let me = ctx.air.me();
                let reads = reads.into_iter().filter_map(|instance| {
                    let read = instance.pending();
                    (read.owner == me).then(|| (read.room, read.clone()))
                }).collect::<std::collections::HashMap<_, _>>();
                let mut items = rooms.into_iter().filter(|room| room.pending().members.contains(&me)).map(|mut instance| {
                    let chat = Flow::new(vec![Page::messaging(ctx, &mut instance)]);
                    let room = instance.pending();
                    let members = room.members.iter().copied().filter(|name| *name != me).collect::<Vec<_>>();
                    let name = room.name(ctx);
                    let avatar = if room.is_group() {
                        room.group_details().photo
                    } else {
                        let profile = Profile::from_name(ctx, members.first().copied().unwrap_or(me));
                        profile.pending().avatar.clone()
                    };
                    let (timestamp, last) = match room.messages(me).last() {
                        Some(last) => (last.timestamp, last.preview(ctx, room.is_group())),
                        None => (0, "No messages yet.".into()),
                    };
                    let unread = room.messages(me).any(|message| message.author != me
                        && reads.get(&instance.id()).is_none_or(|read| !read.is_read(message)));
                    (timestamp, ListItem::avatar(avatar, &name, &last, None, Some(Action::flow(chat))).with_badge(unread))
                }).collect::<Vec<_>>();
                items.sort_by_key(|item| std::cmp::Reverse(item.0));
                let items = items.into_iter().map(|(_, item)| item).collect();
                let presets = presets.clone();
                let new_message = Action::deferred_flow(move |ctx: &mut Context, theme: &Theme| {
                    Flow::from_form(presets.new_message(ctx, theme))
                });
                PageType::root("Messages", vec![], vec![Display::list(None, items,
                    Some("No messages yet.\nGet started by messaging a friend."))], None,
                    Some(("New Message".into(), new_message)), None)
            }))
    }

    pub fn contacts(&self) -> Root {
        let presets = self.clone();
        Root::custom(Page::watching(Watch::list::<Profile>(),
            move |ctx: &mut Context, _theme: &Theme, profiles: Vec<Instance<Profile>>| {
                let me = ctx.air.me();
                let mut items = profiles.into_iter().filter_map(|mut instance| {
                    if instance.pending().name == Some(me) {return None;}
                    let view = Flow::new(vec![Page::profile(ctx, &mut instance)]);
                    let profile = instance.pending();
                    Some(ListItem::avatar(profile.avatar.clone(), &profile.username,
                        &profile.name(), None, Some(Action::flow(view))))
                }).collect::<Vec<_>>();
                items.sort_by(|a, b| a.title.cmp(&b.title));
                let presets = presets.clone();
                let new_contact = Action::deferred_flow(move |_ctx: &mut Context, theme: &Theme| {
                    Flow::from_form(presets.new_contact(theme))
                });
                PageType::root("Contacts", vec![], vec![Display::list(None, items,
                    Some("No contacts yet.\nGet started by adding a friend."))], None,
                    Some(("New Contact".into(), new_contact)), None)
            }))
    }

    pub fn new_message(&self, ctx: &mut Context, theme: &Theme) -> Form {
        let submit = Box::new(|ctx: &mut Context, state: &FormValues| {
            let Some(keys) = state.selection("recipients") else {
                return FormComplete::None;
            };
            let recipients = match Self::selected_recipients(keys) {
                Ok(recipients) => recipients,
                Err(error) => {
                    return FormComplete::Next(Page::Static(Box::new(PageType::display(
                        "New message",
                        vec![Display::instructions(error)],
                        None,
                        Bumper::Done,
                        Offset::Center,
                    ))));
                }
            };
            let mut room = ChatRoom::create(ctx, recipients);
            room.notify(true);
            FormComplete::Next(Page::messaging(ctx, &mut room))
        }) as Box<dyn FormSubmit>;
        let me = ctx.air.me();
        let items = ctx
            .air
            .instances::<Profile>()
            .list()
            .into_values()
            .filter_map(|profile| Self::profile_row(&profile.pending(), me))
            .collect();
        let selector = Selector::new("contacts", items);
        let add_contact = self.select_contact(theme, selector.clone(), vec![]);
        let selector = selector.with_actions(vec![FormAction::Custom(ActionItem::new(Action::flow(Flow::from_form(add_contact)), "New contact", Icons::Add))]);
        Form::flow(
            theme,
            vec![FormItem::selection("New message", selector).named("recipients")],
            None,
            None,
            submit,
        )
    }

    /// Keys belong to the generic UI; parse the entire selection before creating a room.
    /// Invalid keys must never silently turn an intended group into a smaller audience.
    pub fn selected_recipients(keys: &[String]) -> Result<Vec<Name>, &'static str> {
        if keys.is_empty() {
            return Err("Choose at least one contact.");
        }
        let mut recipients = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for key in keys {
            let name = key
                .parse::<Name>()
                .map_err(|_| "One of the selected contacts is invalid. Choose your contacts again.")?;
            if seen.insert(name) {
                recipients.push(name);
            }
        }
        Ok(recipients)
    }

    fn profile_row(profile: &Profile, me: Name) -> Option<(ListItem, String)> {
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

    fn parse_contact(&self, value: &str) -> Option<Name> {
        match &self.invitations {
            Some(invitations) => invitations.parse_invite(value).or_else(|| value.trim().parse().ok()),
            None => value.trim().parse().ok(),
        }
    }

    /// Add a contact and open their profile. The self-contact shortcut belongs in messages.
    pub fn new_contact(&self, theme: &Theme) -> Form {
        let presets = self.clone();
        self.contact_form(theme, vec![], false, Box::new(move |ctx: &mut Context, values: &FormValues| {
            let Some(value) = values.text("airname") else {return FormComplete::None;};
            let Some(name) = presets.parse_contact(value) else {return FormComplete::None;};
            if name == ctx.air.me() {return FormComplete::None;}
            let mut profile = Profile::from_name(ctx, name);
            FormComplete::Next(Page::profile(ctx, &mut profile))
        }))
    }

    /// Add a contact and return it to an existing selector, retaining the current selection.
    /// Excluded members cannot be added again (for example, current group members).
    pub fn select_contact(&self, theme: &Theme, selector: Selector, excluded: Vec<Name>) -> Form {
        let presets = self.clone();
        let excluded_on_submit = excluded.clone();
        self.contact_form(theme, excluded, true, Box::new(move |ctx: &mut Context, values: &FormValues| {
            let Some(value) = values.text("airname") else {return FormComplete::None;};
            let Some(name) = presets.parse_contact(value) else {return FormComplete::None;};
            if excluded_on_submit.contains(&name) {return FormComplete::None;}
            let profile = Profile::from_name(ctx, name);
            let Some((row, key)) = Self::profile_row(&profile.pending(), ctx.air.me()) else {return FormComplete::None;};
            FormComplete::Return(selector.select(row, key).get())
        }))
    }

    fn contact_form(&self, theme: &Theme, excluded: Vec<Name>, allow_self: bool, submit: Box<dyn FormSubmit>) -> Form {
        let presets = self.clone();
        Form::flow(theme, vec![FormItem::text("New contact", "AirName", Some(vec![
            FormAction::Paste, FormAction::scan_qr_code("Scan an AirName QR code"),
        ]), move |ctx: &mut Context, value: String| {
            if value.trim().is_empty() {return FormValidState::Invalid;}
            match presets.parse_contact(&value) {
                Some(name) if !allow_self && name == ctx.air.me() => FormValidState::InvalidWithData("You cannot add yourself as a contact.".into()),
                Some(name) if excluded.contains(&name) => FormValidState::InvalidWithData("Already in this group.".into()),
                Some(_) => FormValidState::Valid,
                None => FormValidState::InvalidWithData("Enter a valid AirName or contact link.".into()),
            }
        }).named("airname")], None, None, submit)
    }
}
