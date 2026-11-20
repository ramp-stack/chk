use prism::Context;
use prism::air::{Instance, Name};
use prism::drawable::text::Align;
use prism::events::{ChildEvent, DynEvent, Event};
use ptsd::layout::Offset;
use ptsd::navigation::NavigationEvent;
use uuid::Uuid;

use std::collections::BTreeMap;
use std::fmt::{self, Debug};
use std::future::Future;
use std::ops::Range;
use std::pin::Pin;

use crate::blocking::Block;
use crate::closure::{FormSubmit, NavFn, ReviewItemGetter, SuccessGetter};
use crate::design_system::{
    AvatarContent, Content, Footer, FormEvent, Header, Icons, PageCallbacks, PageView, Selector,
    TextSize, TextStyle, Theme,
};
use crate::items::{Action, ActionItem, Display, Input, ListItem};
pub use crate::items::{FormAction, FormItem, FormValidState, NumberVariant, State};
use crate::messages::ChatRoom;
use crate::profiles::Profile;
use crate::{Bumper, ContactInvitations, Flow, Page, PageType};

mod runtime;
pub use runtime::{DeferredSubmit, FormHost, Submission, SubmitFinished, SubmitRequest};

#[derive(Debug, Clone)]
pub struct Success {
    pub title: String,
    pub getter: Box<dyn SuccessGetter>,
}

impl Success {
    pub fn new(title: &str, getter: impl SuccessGetter + 'static) -> Self {
        Success { title: title.to_string(), getter: Box::new(getter) }
    }

    pub fn page(title: &str, getter: Box<dyn SuccessGetter>, on_submit: Option<Box<dyn FormSubmit>>) -> PageType {
        let lifecycle = Box::new(move |
            _: &mut Context, theme: &Theme, _: Option<NavFn>, _: usize,
        | -> PageCallbacks {
            let event_theme = theme.clone();
            let mut getter = getter.clone();
            let mut on_submit = on_submit.clone();
            let mut values = None;
            let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
                let Some(new) = event.downcast_ref::<FormValues>() else {return false;};
                if values.as_ref() == Some(new) {return true;}
                values = Some(new.clone());
                if let Some(mut submit) = on_submit.take() {submit(ctx, new).run(ctx, &event_theme);}
                let (icon, description) = getter(new.clone());
                let items = vec![Display::icon(icon), Display::Text {
                    text: description, size: TextSize::H4,
                    style: TextStyle::Heading, align: Align::Center,
                }];
                *page.content_mut().children_mut() = items.into_iter()
                    .flat_map(|item| item.build(ctx, &event_theme)).collect();
                true
            });
            (Some(on_event), None, None)
        });
        PageType::Content {
            header: Header::stack(title, None), items: vec![], bumper: Bumper::Done,
            offset: Offset::Center, flow_len: 1, next: None, lifecycle: Some(lifecycle),
            identity: None, reveal_child: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Review {
    pub title: String,
    pub getter: Box<dyn ReviewItemGetter>,
}

impl Review {
    pub fn new(title: &str, getter: impl ReviewItemGetter + 'static) -> Self {
        Review { title: title.to_string(), getter: Box::new(getter) }
    }

    pub fn page(title: &str, getter: Box<dyn ReviewItemGetter>, on_submit: Box<dyn FormSubmit>) -> PageType {
        let request_id = Uuid::new_v4();
        let footer = Footer::stack(
            None, Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(SubmitRequest(request_id))),
            None,
        );
        let lifecycle = Box::new(move |
            _: &mut Context, theme: &Theme, next: Option<NavFn>, _: usize,
        | -> PageCallbacks {
            let event_theme = theme.clone();
            let mut getter = getter.clone();
            let mut on_submit = on_submit.clone();
            let mut values = None;
            let mut submission = Submission::default();
            let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
                let id = page.content().id();
                if submission.check_navigation(id, event) {return true;}
                if let Some(new) = event.downcast_ref::<FormValues>() {
                    if values.as_ref() == Some(new) {return true;}
                    values = Some(new.clone());
                    let children = getter(new).into_iter()
                        .flat_map(|item| item.build(ctx, &event_theme)).collect();
                    *page.content_mut().children_mut() = children;
                    if submission.pending() {page.disable_actions(true);}
                    return true;
                }
                if let Some(SubmitRequest(target)) = event.downcast_ref::<SubmitRequest>() {
                    if *target != request_id {return false;}
                    if submission.pending() {return true;}
                    let Some(values) = values.as_ref() else {return true;};
                    let result = on_submit(ctx, values);
                    submission.start(page, result, ctx, &event_theme, next.clone());
                    return true;
                }
                if let Some(SubmitFinished(target, result)) = event.downcast_ref::<SubmitFinished>() {
                    if *target != id {return false;}
                    submission.finish(page);
                    result.clone().then(next.clone()).run(ctx, &event_theme);
                    return true;
                }
                if let Some(FormEvent::Validated {id: target, ..}) = event.downcast_ref::<FormEvent>()
                    && *target == id && submission.pending() {
                    page.disable_actions(true);
                    return true;
                }
                false
            });
            (Some(on_event), None, None)
        });
        PageType::Content {
            header: Header::stack(title, None), items: vec![], bumper: Bumper::Footer(footer),
            offset: Offset::Start, flow_len: 1, next: None, lifecycle: Some(lifecycle),
            identity: None, reveal_child: None,
        }
    }
}

/// Form values are addressed by field ID, independently of visual order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormValues(BTreeMap<String, State>);

impl Event for FormValues {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}

impl FormValues {
    pub fn new(values: impl IntoIterator<Item = (String, State)>) -> Self {
        let mut result = FormValues(BTreeMap::new());
        for (id, value) in values {
            assert!(!id.trim().is_empty(), "Form field IDs must not be empty");
            assert!(result.0.insert(id.clone(), value).is_none(), "Duplicate form field ID: {id}");
        }
        result
    }
    pub fn from_content(content: &Content, fields: &[(String, Range<usize>)]) -> Self {
        let children = content.children();
        let mut values = BTreeMap::new();
        for (id, range) in fields {
            let mut states =
                children[range.clone()].iter().filter_map(|child| Input::state(&**child));
            let state = states.next().expect("Each form field must produce one value");
            assert!(states.next().is_none(), "A field must not produce multiple values");
            assert!(!id.trim().is_empty(), "Form field IDs must not be empty");
            assert!(values.insert(id.clone(), state).is_none(), "Duplicate form field ID: {id}");
        }
        FormValues(values)
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

#[derive(Debug, Clone)]
pub enum Form {
    Flow {
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
    },
}

impl Form {
    pub fn flow(inputs: Vec<FormItem>, review: Option<Review>, success: Option<Success>, on_submit: Box<dyn FormSubmit>) -> Self {
        FormItem::ids(&inputs);
        Form::Flow { inputs, review, success, on_submit }
    }

    pub fn page(title: &str, inputs: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        FormItem::ids(&inputs);
        Form::Page { title: title.to_string(), inputs, display, on_save }
    }

    pub fn new_message(invitations: &ContactInvitations, ctx: &mut Context, theme: &Theme) -> Form {
        let messaging_invitations = invitations.clone();
        let submit = Box::new(move |ctx: &mut Context, state: &FormValues| {
            let Some(keys) = state.selection("recipients") else {return FormComplete::None;};
            let recipients = match ContactInvitations::selected_recipients(keys) {
                Ok(recipients) => recipients,
                Err(error) => {
                    return FormComplete::Next(Page::Static(Box::new(PageType::display(
                        "New message", vec![Display::instructions(error)], None, Bumper::Done,
                        Offset::Center,
                    ))));
                }
            };
            if recipients.iter().any(|name| Block::am_i_blocked(ctx, *name)) {
                return FormComplete::Error("A selected user has blocked you.".into());
            }
            let room = ChatRoom::create(ctx, recipients);
            crate::messages::RoomNotificationState::apply_to(ctx, &room);
            FormComplete::Next(Page::messaging(&messaging_invitations, room, None))
        }) as Box<dyn FormSubmit>;

        let items = ctx.air.instances::<Profile>().list().into_values().filter_map(|profile| {
                let profile = profile.pending();
                let row = ListItem::recipient(ctx, theme, &profile)?;
                Some((row, profile.name?))
            }).map(|(item, name)| (item, name.to_string())).collect();

        let selector_id = Uuid::new_v4();
        let input_id = Uuid::new_v4();
        let contact_inputs = vec![FormItem::contact(invitations, vec![], true, input_id)];
        FormItem::ids(&contact_inputs);
        let add_contact = Form::Flow {
            inputs: contact_inputs, review: None, success: None,
            on_submit: Selector::contact_submit(selector_id, invitations, theme, vec![], input_id),
        };
        let actions = vec![FormAction::Custom(ActionItem::new(
            Action::flow(Flow::from_form(add_contact)), "New contact", Icons::Add,
        ))];
        let selector = Selector::new(
            selector_id, "contacts", items, usize::MAX, actions, false, true,
        );
        let inputs = vec![FormItem::selection("recipients", "New message", selector)];
        FormItem::ids(&inputs);
        Form::Flow {
            inputs, review: None, success: None, on_submit: submit,
        }
    }

    pub fn select_contact(invitations: &ContactInvitations, theme: &Theme, selector_id: Uuid, excluded: Vec<Name>) -> Form {
        let input_id = Uuid::new_v4();
        let inputs = vec![FormItem::contact(invitations, excluded.clone(), true, input_id)];
        FormItem::ids(&inputs);
        let on_submit = Selector::contact_submit(selector_id, invitations, theme, excluded, input_id);
        Form::Flow {inputs, review: None, success: None, on_submit}
    }

    pub fn new_contact(invitations: &ContactInvitations) -> Form {
        let input_id = Uuid::new_v4();
        let inputs = vec![FormItem::contact(invitations, vec![], false, input_id)];
        FormItem::ids(&inputs);
        let parse = invitations.clone();
        let on_submit = Box::new(move |ctx: &mut Context, values: &FormValues| {
            let value = values.text("airname").unwrap_or_default();
            let name = match parse.validate_contact(ctx, value, &[], false) {
                Ok(name) => name,
                Err(_) => {
                    ctx.emit(crate::design_system::TextInputEvent::Changed {
                        id: input_id, value: value.to_string(),
                    });
                    return FormComplete::Action(Action::None);
                }
            };
            let mut profile = Profile::from_name(ctx, name);
            FormComplete::Next(Page::profile(&parse, ctx, &mut profile))
        });
        Form::Flow {inputs, review: None, success: None, on_submit}
    }

    pub fn add_members(invitations: &ContactInvitations, ctx: &mut Context, theme: &Theme, mut room: Instance<ChatRoom>) -> Form {
        let members = room.pending().members.clone();
        let mut items: Vec<_> = ctx.air.instances::<Profile>().list().into_values()
            .filter_map(|profile| {
                let profile = profile.pending();
                let name = profile.name?;
                if members.contains(&name) {return None;}
                ListItem::recipient(ctx, theme, &profile).map(|row| (row, name))
            }).collect();
        items.sort_by_key(|(_, name)| *name);
        items.dedup_by_key(|(_, name)| *name);
        let items = items.into_iter().map(|(item, name)| (item, name.to_string())).collect();
        let selector_id = Uuid::new_v4();
        let input_id = Uuid::new_v4();
        let contact_inputs = vec![FormItem::contact(invitations, members.clone(), true, input_id)];
        FormItem::ids(&contact_inputs);
        let new_contact = Form::Flow {
            inputs: contact_inputs, review: None, success: None,
            on_submit: Selector::contact_submit(selector_id, invitations, theme, members, input_id),
        };
        let submit = Box::new(move |ctx: &mut Context, values: &FormValues| {
            let Some(keys) = values.selection("members") else {return FormComplete::None;};
            let Ok(names) =
                keys.iter().map(|key| key.parse::<Name>()).collect::<Result<Vec<_>, _>>()
            else {return FormComplete::None;};
            let names = names.into_iter().filter(|name| !Block::am_i_blocked(ctx, *name)).collect();
            ChatRoom::add_members(ctx, &mut room, names);
            ctx.emit(NavigationEvent::Pop);
            FormComplete::None
        }) as Box<dyn FormSubmit>;
        let actions = vec![FormAction::Custom(ActionItem::new(
            Action::flow(Flow::from_form(new_contact)), "New contact", Icons::Add,
        ))];
        let selector = Selector::new(
            selector_id, "contacts", items, usize::MAX, actions, false, true,
        );
        let inputs = vec![FormItem::selection("members", "Add members", selector)];
        FormItem::ids(&inputs);
        Form::Flow {
            inputs, review: None, success: None, on_submit: submit,
        }
    }
}

/// Cloneable completion future; CHK polls it on the UI thread.
#[derive(Clone)]
pub struct PendingSubmit(crate::pending::Pending<FormComplete>);
impl fmt::Debug for PendingSubmit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {f.write_str("PendingSubmit")}
}

impl Future for PendingSubmit {
    type Output = FormComplete;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<FormComplete> {
        Pin::new(&mut self.get_mut().0).poll(cx)
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

impl FormComplete {
    pub fn pending(future: impl Future<Output = FormComplete> + 'static) -> Self {
        FormComplete::Pending(PendingSubmit(crate::pending::Pending::new(future)))
    }

    pub async fn resolved(mut self) -> Self {
        while let FormComplete::Pending(pending) = self {self = pending.await;}
        self
    }

    pub fn then(self, next: Option<crate::NavFn>) -> Self {
        match self {
            FormComplete::None => next.map(|next| {
                    FormComplete::Action(Action::custom(move |ctx: &mut Context, theme: &Theme| {
                        (next.borrow_mut())(ctx, theme);
                    }))
                }).unwrap_or(FormComplete::None),
            FormComplete::Pending(pending) => {
                FormComplete::Pending(PendingSubmit(crate::pending::Pending::new(async move {
                    pending.await.resolved().await.then(next)
                })))
            }
            other => other,
        }
    }

    pub fn run(&mut self, ctx: &mut Context, theme: &Theme) {
        match self {
            FormComplete::Return(function) => {
                let theme = theme.clone();
                let function = function.clone();
                ctx.emit(NavigationEvent::reset_with_fn(move |ctx: &mut Context| {
                    (function.clone())(ctx, &theme.clone())
                }));
            }
            FormComplete::Next(page) => {
                let flow = Flow::new(vec![page.clone()]).build_as_flow(ctx, theme);
                ctx.emit(NavigationEvent::restart(flow));
            }
            FormComplete::Action(action) => (action.get())(ctx, theme),
            FormComplete::Error(message) => {
                let page = crate::PageType::display(
                    "Unable to complete", vec![Display::instructions(message)], None,
                    crate::Bumper::Done, crate::Offset::Center,
                );
                (Flow::new(vec![Page::Static(Box::new(page))]).build(ctx, theme))(ctx, theme);
            }
            FormComplete::Pending(pending) => {
                ctx.emit(DeferredSubmit(pending.clone()))
            }
            FormComplete::None => {}
        }
    }
}
