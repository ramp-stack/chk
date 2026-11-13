use crate::{ContactInvitations, PageType, Bumper};
use crate::messages::ChatRoom;
use crate::profiles::Profile;
use crate::blocking::Block;
use prism::air::Instance;
use ptsd::layout::Offset;
use prism::Context;
use crate::design_system::{Theme, Icons};
use prism::drawable::Drawable;
use crate::design_system::{AvatarContent, AvatarIconStyle};
use ptsd::navigation::NavigationEvent;

use crate::design_system::Selector;
use ptsd::Validate;
use ptsd::utils::ValidationFn;

use prism::air::Name;
use uuid::Uuid;

use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

use crate::{Flow, Page, Review, Success};
use crate::items::{ValueItem, ActionItem, Input, ListItem, Action, Display};
use crate::closure::{FormSubmit, ValidityFn};

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Slider(f32),
    Text(String),
    Enumerator(usize),
    Number(String),
    Avatar(AvatarContent),
    Search(Vec<Name>),
    Selection(Vec<String>),
    ScanCode(Option<String>),
}

/// Form values are addressed by field ID, independently of visual order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormValues(std::collections::BTreeMap<String, State>);
impl FormValues {
    pub fn new(values: impl IntoIterator<Item = (String, State)>) -> Self {
        let mut result = Self(std::collections::BTreeMap::new());
        for (id, value) in values {
            assert!(!id.trim().is_empty(), "Form field IDs must not be empty");
            assert!(result.0.insert(id.clone(), value).is_none(), "Duplicate form field ID: {id}");
        }
        result
    }
    pub fn get(&self, id: &str) -> Option<&State> {self.0.get(id)}
    pub fn slider(&self, id: &str) -> Option<f32> {
        match self.get(id)? {State::Slider(value) => Some(*value), _ => None}
    }
    pub fn text(&self, id: &str) -> Option<&str> {
        match self.get(id)? {State::Text(value) => Some(value), _ => None}
    }
    pub fn number(&self, id: &str) -> Option<&str> {
        match self.get(id)? {State::Number(value) => Some(value), _ => None}
    }
    pub fn avatar(&self, id: &str) -> Option<&AvatarContent> {
        match self.get(id)? {State::Avatar(value) => Some(value), _ => None}
    }
    pub fn selection(&self, id: &str) -> Option<&[String]> {
        match self.get(id)? {State::Selection(value) => Some(value), _ => None}
    }
    pub fn search(&self, id: &str) -> Option<&[Name]> {
        match self.get(id)? {State::Search(value) => Some(value), _ => None}
    }
    pub fn enumerator(&self, id: &str) -> Option<usize> {
        match self.get(id)? {State::Enumerator(value) => Some(*value), _ => None}
    }
    pub fn scan_code(&self, id: &str) -> Option<&Option<String>> {
        match self.get(id)? {State::ScanCode(value) => Some(value), _ => None}
    }
    pub fn extend(&mut self, other: Self) {
        for (id, value) in other.0 {
            assert!(self.0.insert(id.clone(), value).is_none(), "Duplicate form field ID: {id}");
        }
    }
}

#[derive(Clone, Debug)]
pub enum NumberVariant {
    Currency,
    Date,
    Time,
    Number,
}

#[derive(Debug, Clone)]
pub enum Form {
    Flow {
        theme: Box<Theme>,
        inputs: Vec<FormItem>,
        review: Option<Review>,
        success: Option<Success>,
        on_submit: Box<dyn FormSubmit>,
    },
    Page {
        title: String,
        inputs: Vec<FormItem>,
        display: Vec<Display>,
        on_save: Box<dyn FormSubmit>,
    }
}

impl Form {
    pub fn flow(theme: &Theme, inputs: Vec<FormItem>, review: Option<Review>, success: Option<Success>, on_submit: Box<dyn FormSubmit>) -> Self {
        FormItem::ids(&inputs);
        Form::Flow {
            inputs,
            theme: Box::new(theme.clone()),
            review,
            success,
            on_submit,
        }
    }

    pub fn page(title: &str, inputs: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        FormItem::ids(&inputs);
        Form::Page {
            title: title.to_string(),
            inputs,
            display,
            on_save,
        }
    }

    pub fn new_message(invitations: &ContactInvitations, ctx: &mut Context, theme: &Theme) -> Form {
        let submit = Box::new(|ctx: &mut Context, state: &FormValues| {
            let Some(keys) = state.selection("recipients") else {
                return FormComplete::None;
            };
            let recipients = match ContactInvitations::selected_recipients(keys) {
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
            if recipients.iter().any(|name| Block::am_i_blocked(ctx, *name)) {
                return FormComplete::Error("A selected user has blocked you.".into());
            }
            let mut room = ChatRoom::create(ctx, recipients);
            crate::messages::RoomNotificationState::apply_to(ctx, &room);
            FormComplete::Next(Page::messaging(ctx, &mut room))
        }) as Box<dyn FormSubmit>;
        let items = ctx
            .air
            .instances::<Profile>()
            .list()
            .into_values()
            .filter_map(|profile| Self::recipient_item(ctx, theme, &profile.pending()))
            .map(|(item, name)| (item, name.to_string()))
            .collect();
        let selector = Selector::new("contacts", items).with_alphabetical_order();
        let (contact_inputs, on_submit) = Self::contact_selection_fields(invitations, theme, selector.clone(), vec![]);
        let add_contact = Form::Flow {
            theme: Box::new(theme.clone()), inputs: contact_inputs, review: None, success: None, on_submit,
        };
        let selector = selector.with_actions(vec![FormAction::Custom(ActionItem::new(Action::flow(Flow::from_form(add_contact)), "New contact", Icons::Add))]);
        let inputs = vec![FormItem::selection("New message", selector).named("recipients")];
        FormItem::ids(&inputs);
        Form::Flow {
            theme: Box::new(theme.clone()), inputs, review: None, success: None, on_submit: submit,
        }
    }

    pub fn select_contact(invitations: &ContactInvitations, theme: &Theme, selector: Selector, excluded: Vec<Name>) -> Form {
        let (inputs, on_submit) = Self::contact_selection_fields(invitations, theme, selector, excluded);
        Form::Flow {theme: Box::new(theme.clone()), inputs, review: None, success: None, on_submit}
    }

    fn contact_selection_fields(invitations: &ContactInvitations, theme: &Theme, selector: Selector, excluded: Vec<Name>) -> (Vec<FormItem>, Box<dyn FormSubmit>) {
        let excluded_on_submit = excluded.clone();
        let parse = invitations.clone();
        let row_theme = theme.clone();
        Self::contact_fields(invitations, excluded, true, Box::new(move |ctx: &mut Context, values: &FormValues| {
            let Some(value) = values.text("airname") else {return FormComplete::None;};
            let Some(name) = parse.parse_contact(value) else {return FormComplete::None;};
            if excluded_on_submit.contains(&name) {return FormComplete::None;}
            let profile = Profile::from_name(ctx, name);
            let Some((row, name)) = Self::recipient_item(ctx, &row_theme, &profile.pending()) else {return FormComplete::None;};
            FormComplete::Return(selector.select(row, name.to_string()))
        }))
    }

    fn contact_fields(invitations: &ContactInvitations, excluded: Vec<Name>, allow_self: bool, mut submit: Box<dyn FormSubmit>) -> (Vec<FormItem>, Box<dyn FormSubmit>) {
        let invitations = invitations.clone();
        let parse = invitations.clone();
        let input_id = Uuid::new_v4();
        let submit = Box::new(move |ctx: &mut Context, values: &FormValues| {
            let value = values.text("airname").unwrap_or_default();
            let name = parse.parse_contact(value);
            if name.is_some_and(|name| Block::am_i_blocked(ctx, name)) {
                // Revalidate in place if a block arrived after the field was validated.
                ctx.emit(crate::design_system::TextInputEvent::Changed {id: input_id, value: value.to_string()});
                return FormComplete::Action(Action::None);
            }
            submit(ctx, values)
        });
        let inputs = vec![FormItem::text("New contact", "AirName", Some(vec![
            FormAction::Paste, FormAction::scan_qr_code("Scan an AirName QR code"),
        ]), move |ctx: &mut Context, value: String| {
            if value.trim().is_empty() {return FormValidState::Invalid;}
            match invitations.parse_contact(&value) {
                Some(name) if !allow_self && name == ctx.air.me() => FormValidState::InvalidWithData("You cannot add yourself as a contact.".into()),
                Some(name) if excluded.contains(&name) => FormValidState::InvalidWithData("Already in this group.".into()),
                Some(name) if Block::am_i_blocked(ctx, name) => FormValidState::InvalidWithData("This user blocked you. You cannot add them as a contact.".into()),
                Some(_) => FormValidState::Valid,
                None => FormValidState::InvalidWithData("Enter a valid AirName or contact link.".into()),
            }
        }).with_input_id(input_id).named("airname")];
        FormItem::ids(&inputs);
        (inputs, submit)
    }

    pub fn new_contact(invitations: &ContactInvitations, theme: &Theme) -> Form {
        let parse = invitations.clone();
        let (inputs, on_submit) = Self::contact_fields(invitations, vec![], false, Box::new(move |ctx: &mut Context, values: &FormValues| {
            let Some(value) = values.text("airname") else {return FormComplete::None;};
            let Some(name) = parse.parse_contact(value) else {return FormComplete::None;};
            if name == ctx.air.me() {return FormComplete::None;}
            let mut profile = Profile::from_name(ctx, name);
            FormComplete::Next(Page::profile(ctx, &mut profile))
        }));
        Form::Flow {theme: Box::new(theme.clone()), inputs, review: None, success: None, on_submit}
    }

    pub fn add_members(ctx: &mut Context, theme: &Theme, mut room: Instance<ChatRoom>) -> Form {
        let members = room.pending().members.clone();
        let mut items: Vec<_> = ctx.air.instances::<Profile>().list().into_values().filter_map(|profile| {
            let profile = profile.pending();
            let name = profile.name?;
            if members.contains(&name) {return None;}
            Self::recipient_item(ctx, theme, &profile)
        }).collect();
        items.sort_by_key(|(_, name)| *name);
        items.dedup_by_key(|(_, name)| *name);
        let selector = Selector::new("contacts", items.into_iter()
            .map(|(item, name)| (item, name.to_string())).collect()).with_alphabetical_order();
        let invitations = ContactInvitations::default();
        let (contact_inputs, on_submit) = Self::contact_selection_fields(&invitations, theme, selector.clone(), members);
        let new_contact = Form::Flow {
            theme: Box::new(theme.clone()), inputs: contact_inputs, review: None, success: None, on_submit,
        };
        let submit = Box::new(move |ctx: &mut Context, values: &FormValues| {
            let Some(keys) = values.selection("members") else {return FormComplete::None;};
            let Ok(names) = keys.iter().map(|key| key.parse::<Name>()).collect::<Result<Vec<_>, _>>()
                else {return FormComplete::None;};
            let names = names.into_iter().filter(|name| !Block::am_i_blocked(ctx, *name)).collect();
            ChatRoom::add_members(ctx, &mut room, names);
            ctx.emit(NavigationEvent::Pop);
            FormComplete::None
        }) as Box<dyn FormSubmit>;
        let inputs = vec![FormItem::selection("Add members", selector.with_actions(vec![
            FormAction::Custom(ActionItem::new(Action::flow(Flow::from_form(new_contact)), "New contact", Icons::Add)),
        ])).named("members")];
        FormItem::ids(&inputs);
        Form::Flow {
            theme: Box::new(theme.clone()), inputs, review: None, success: None, on_submit: submit,
        }
    }

    fn recipient_item(ctx: &mut Context, theme: &Theme, profile: &Profile) -> Option<(ListItem, Name)> {
        let name = profile.name?;
        if Block::am_i_blocked(ctx, name) {return None;}
        let blocked = Block::did_i_block(ctx, name);
        let subtitle = if blocked {
            "You blocked this user".to_string()
        } else {profile.name()};
        let username = if name == ctx.air.me() {format!("{} (You)", profile.username)}
            else {profile.username.clone()};
        let item = ListItem::avatar(profile.avatar.clone(), &username, &subtitle, None, None)
            .with_flair(blocked.then(|| (Icons::Block, theme.colors().get(ptsd::Status::Danger))));
        Some((item, name))
    }
}



#[derive(Debug, Clone)]
pub enum FormComplete {
    Next(Page),
    Action(Action),
    Error(String),
    Pending(PendingSubmit),
    Return(Box<dyn crate::design_system::Callback>),
    None,
}

/// Cloneable completion future; CHK polls it on the UI thread.
#[derive(Clone)]
pub struct PendingSubmit(pub(crate) crate::pending::Pending<FormComplete>);
impl std::fmt::Debug for PendingSubmit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {f.write_str("PendingSubmit")}
}

impl FormComplete {
    pub fn pending(future: impl Future<Output = FormComplete> + 'static) -> Self {
        Self::Pending(PendingSubmit(crate::pending::Pending::new(future)))
    }

    pub(crate) async fn resolved(mut self) -> Self {
        while let Self::Pending(pending) = self {self = pending.0.await;}
        self
    }

    pub(crate) fn then(self, next: Option<crate::NavFn>) -> Self {
        match self {
            Self::None => next.map(|next| Self::Action(Action::custom(move |ctx: &mut Context, theme: &Theme| {
                (next.borrow_mut())(ctx, theme);
            }))).unwrap_or(Self::None),
            Self::Pending(pending) => {
                Self::Pending(PendingSubmit(crate::pending::Pending::new(async move {pending.0.await.resolved().await.then(next)})))
            },
            other => other,
        }
    }

    pub fn run(&mut self, ctx: &mut Context, theme: &Theme) {
        match self {
            FormComplete::Return(function) => {
                let theme = theme.clone();
                let function = function.clone();
                ctx.emit(NavigationEvent::reset_with_fn(move |ctx: &mut Context| (function.clone())(ctx, &theme.clone())));
            },
            FormComplete::Next(page) => {
                let flow = Flow::new(vec![page.clone()]).build_as_flow(ctx, theme);
                ctx.emit(NavigationEvent::restart(flow));
            },
            FormComplete::Action(action) => (action.get())(ctx, theme),
            FormComplete::Error(message) => {
                let page = crate::PageType::display("Unable to complete", vec![Display::instructions(message)],
                    None, crate::Bumper::Done, crate::Offset::Center);
                (Flow::new(vec![Page::Static(Box::new(page))]).build(ctx, theme))(ctx, theme);
            },
            FormComplete::Pending(pending) => ctx.emit(crate::page::DeferredSubmit(pending.clone())),
            FormComplete::None => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormValidState {
    InvalidWithData(String),
    Invalid,
    Valid,
    ValidWithData(String),
}

impl FormValidState {
    pub fn eval(&self) -> Result<String, String> {
        match self {
            FormValidState::InvalidWithData(s) => Err(s.to_string()),
            FormValidState::Invalid => Err(String::new()),
            FormValidState::Valid => Ok(String::new()),
            FormValidState::ValidWithData(s) => Ok(s.to_string()),
        }
    }

    pub fn from(r: Result<String, String>) -> Self {
        match r {
            Ok(e) if e.is_empty() => FormValidState::Valid,
            Ok(e) => FormValidState::ValidWithData(e),
            Err(e) if e.is_empty() => FormValidState::Invalid,
            Err(e) => FormValidState::InvalidWithData(e),
        }
    }
}

#[derive(Debug, Clone)]
pub enum FormAction {
    Paste,
    ScanQRCode(String),
    Custom(ActionItem)
}

impl FormAction {
    pub fn scan_qr_code(instructions: &str) -> Self {
        FormAction::ScanQRCode(instructions.to_string())
    }

    pub fn get(&self, id: Uuid) -> ActionItem {
        match self {
            Self::Paste => ActionItem::new_with_active(Action::Paste {id}, "Paste clipboard", "Pasted", Icons::Paste),
            Self::ScanQRCode(instructions) => ActionItem::new(Action::scan_qr(instructions, id), "Scan QR code", Icons::QrCode),
            Self::Custom(action) => action.clone(),
        }
    }
}
#[derive(Debug, Clone)]
pub enum FormItem {
    Named {id: String, item: Box<FormItem>},
    InputId {id: Uuid, item: Box<FormItem>},
    Text(String, String, Option<Vec<FormAction>>, Box<dyn ValidityFn>),
    TextWithPreset(String, String, Option<Vec<FormAction>>, Box<dyn ValidityFn>),
    Number(String, NumberVariant, Box<dyn ValidityFn>),
    Enum(String, Vec<ValueItem>),
    Search(String, String, Vec<(ListItem, Name)>, Option<Vec<FormAction>>),
    Selection(String, Selector),
    ScanQR(String, String, Option<(String, Icons, Action)>),
    Avatar(String, AvatarContent, Action, Box<dyn ValidityFn>),
    AvatarWithPreset(String, AvatarContent, Action, Box<dyn ValidityFn>),
}

impl FormItem {
    /// Use a stable ID when the visible label may change or repeat.
    pub fn named(self, id: &str) -> Self {
        assert!(!id.trim().is_empty(), "Form field IDs must not be empty");
        let item = match self {Self::Named {item, ..} => item, item => Box::new(item)};
        Self::Named {id: id.into(), item}
    }

    /// Bind a text or legacy search input to a UUID used by `Action::set_text`.
    /// This is independent of the field ID used by `FormValues`; built-in Paste
    /// and Scan actions use the same UUID. Other input kinds are not supported.
    pub fn with_input_id(self, id: Uuid) -> Self {
        match self {
            Self::Named {id: name, item} => Self::Named {
                id: name, item: Box::new(item.with_input_id(id)),
            },
            Self::InputId {item, ..} => Self::InputId {id, item},
            item @ (Self::Text(..) | Self::TextWithPreset(..) | Self::Search(..)) => {
                Self::InputId {id, item: Box::new(item)}
            },
            _ => panic!("Only text and search fields support an input UUID"),
        }
    }

    pub fn id(&self) -> String {
        match self {
            Self::Named {id, ..} => id.clone(),
            Self::InputId {item, ..} => item.id(),
            _ => self.title(),
        }
    }

    pub(crate) fn ids(items: &[Self]) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        items.iter().map(|item| {
            let id = item.id();
            assert!(!id.trim().is_empty() && seen.insert(id.clone()), "Empty or duplicate form field ID: {id}");
            id
        }).collect()
    }

    pub fn text(text: &str, request: &str, actions: Option<Vec<FormAction>>, valid: impl ValidityFn + 'static) -> Self {
        let text = text.to_string();
        FormItem::Text(text, request.to_string(), actions, Box::new(valid))
    }

    pub fn text_with_preset(text: &str, preset: &str, actions: Option<Vec<FormAction>>, valid: impl ValidityFn + 'static) -> Self {
        FormItem::TextWithPreset(text.to_string(), preset.to_string(), actions, Box::new(valid))
    }

    pub fn number(title: &str, number: NumberVariant, valid: impl ValidityFn + 'static) -> Self {
        FormItem::Number(title.to_string(), number, Box::new(valid))
    }

    pub fn enumerator(label: &str, items: Vec<(&str, &str)>) -> Self {
        let items = items.into_iter().map(|(a, b)| {
            ValueItem::new(a, b)
        }).collect::<Vec<ValueItem>>();
        FormItem::Enum(label.to_string(), items)
    }

    pub fn search(title: &str, search: &str, items: Vec<(ListItem, Name)>, actions: Option<Vec<FormAction>>) -> Self {
        FormItem::Search(title.to_string(), search.to_string(), items, actions)
    }

    /// Search and select application-owned keys with the standard list selector.
    pub fn selection(title: &str, selector: Selector) -> Self {
        FormItem::Selection(title.to_string(), selector)
    }

    pub fn avatar(title: &str, action: Action, valid: impl ValidityFn + 'static) -> Self {
        FormItem::Avatar(title.to_string(), AvatarContent::default(), action, Box::new(valid))
    }

    pub fn avatar_with_preset(title: &str, avatar: AvatarContent, action: Action, valid: impl ValidityFn + 'static) -> Self {
        FormItem::AvatarWithPreset(title.to_string(), avatar, action, Box::new(valid))
    }

    pub fn scan_qr_code(title: &str, instructions: &str, alt: Option<(String, Icons, Action)>) -> Self {
        FormItem::ScanQR(title.to_string(), instructions.to_string(), alt)
    }

    pub fn title(&self) -> String {
        match self {
            Self::Named {item, ..} | Self::InputId {item, ..} => item.title(),
            FormItem::Search(title, ..) |
            FormItem::Selection(title, ..) |
            FormItem::Text(title, ..) |
            FormItem::TextWithPreset(title, ..) |
            FormItem::Number(title, ..) |
            FormItem::Avatar(title, ..) |
            FormItem::AvatarWithPreset(title, ..) |
            FormItem::ScanQR(title, ..) |
            FormItem::Enum(title, ..) => title.to_string()
        }
    }

    pub fn validation(&self) -> Box<dyn ValidationFn> {
        match self {
            Self::Named {item, ..} | Self::InputId {item, ..} => item.validation(),
            FormItem::Text(_, _, _, validation) | FormItem::TextWithPreset(_, _, _, validation) |
            FormItem::Number(_, _, validation) | FormItem::Avatar(_, _, _, validation) |
            FormItem::AvatarWithPreset(_, _, _, validation) => Box::new(InputValidation(validation.clone())),
            FormItem::Search(..) | FormItem::Selection(..) => Box::new(|_ctx: &mut Context, children: Vec<&mut Box<dyn Drawable>>| {
                children.first().and_then(|child| Input::state(child.as_ref()))
                    .is_some_and(|state| match state {State::Search(keys) => !keys.is_empty(), State::Selection(keys) => !keys.is_empty(), _ => false})
            }),
            _ => Box::new(|_ctx: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        }
    }

    pub fn build(&self) -> Input {self.build_with_input_id(None)}

    fn build_with_input_id(&self, id: Option<Uuid>) -> Input {
        match self {
            Self::Named {item, ..} => item.build_with_input_id(id),
            Self::InputId {id, item} => item.build_with_input_id(Some(*id)),
            FormItem::Text(_, request, actions, _) => {
                let id = id.unwrap_or_else(Uuid::new_v4);
                let request = request.clone();
                let actions = actions.clone().map(|act| act.into_iter().map(|a| a.get(id)).collect::<Vec<_>>());
                Input::text(id, &request, false, None, actions)
            },
            FormItem::TextWithPreset(title, preset, actions, _) => {
                let id = id.unwrap_or_else(Uuid::new_v4);
                let title = title.clone();
                let actions = actions.clone().map(|act| act.into_iter().map(|a| a.get(id)).collect::<Vec<_>>());
                Input::text(id, &title, true, Some(preset.to_string()), actions)
            },
            FormItem::Number(_, variant, _) => {
                match variant {
                    NumberVariant::Currency => Input::currency("Enter dollar amount"),
                    NumberVariant::Date => Input::date("Enter date"),
                    NumberVariant::Time => Input::time("Enter time"),
                    NumberVariant::Number => Input::number("Enter number"),
                }
            },
            FormItem::Enum(_, items) => {
                Input::enumerator(items.clone())
            },
            FormItem::Search(_, search, items, actions) => {
                let id = id.unwrap_or_else(Uuid::new_v4);
                let actions = actions.clone().map(|act| act.into_iter().map(|a| a.get(id)).collect::<Vec<_>>());
                Input::search(id, search, items.clone(), actions)
            },
            FormItem::Selection(_, selector) => Input::selection(selector.clone()),
            FormItem::ScanQR(_, instructions, alt) => Input::qr_code_scanner(
                instructions, alt.clone(), Box::new(|_ctx: &mut Context, _value: String| {}),
            ),
            FormItem::Avatar(_, _, action, _) => Input::avatar(
                AvatarContent::default(), Some((Icons::Edit, AvatarIconStyle::Secondary)), Some(action.clone()),
            ),
            FormItem::AvatarWithPreset(_, avatar, action, _) => Input::avatar(
                avatar.clone(), Some((Icons::Edit, AvatarIconStyle::Secondary)), Some(action.clone()),
            ),
        }
    }
}

#[derive(Clone)]
struct InputValidation(Box<dyn ValidityFn>);
impl Validate<Vec<&mut Box<dyn Drawable>>, bool> for InputValidation {
    fn validate(&mut self, ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>) -> bool {
        let Some(child) = children.first_mut() else {return false;};
        let Some(control) = child.downcast_mut::<crate::Control>() else {return true;};
        let Some(value) = control.0.validation_value() else {return true;};
        let result = self.0.validate(ctx, value);
        control.0.feedback(&result);

        matches!(result, FormValidState::Valid | FormValidState::ValidWithData(_))
    }
    fn change(&mut self) -> Pin<Box<dyn Future<Output = Option<()>> + '_>> {self.0.change()}
}


#[derive(Debug, Clone)]
pub(crate) struct SubmitRequest(pub Uuid);
impl prism::events::Event for SubmitRequest {
    fn pass(&self, _: &mut Context, children: &mut [prism::events::ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Debug, Clone)]
pub(crate) struct SubmitFinished(pub Uuid, pub FormComplete);
impl prism::events::Event for SubmitFinished {
    fn pass(&self, _: &mut Context, children: &mut [prism::events::ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Debug, Clone, Default)]
pub(crate) struct Submission {pub pending: bool}
impl Submission {
    pub fn start(&mut self, id: Uuid, result: FormComplete, ctx: &mut Context, theme: &Theme, next: Option<crate::NavFn>) {
        match result {
            FormComplete::Pending(pending) => {
                self.pending = true;
                FormComplete::pending(async move {
                    let result = pending.0.await.resolved().await;
                    FormComplete::Action(Action::custom(move |ctx: &mut Context, _: &Theme| {
                        ctx.emit(SubmitFinished(id, result.clone()));
                    }))
                }).run(ctx, theme);
            }
            result => result.then(next).run(ctx, theme),
        }
    }
}
