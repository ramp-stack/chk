//! Typed invitation setup supplied by an app and managed by CHK.
use prism::{air::Name, drawable::{Drawable, Instruction, LayoutTree, Offset, Rect},
    events::{DynEvent, ListenCallback, ListenFuture}};
use std::{any::Any, cell::RefCell, collections::{HashSet, VecDeque}, rc::{Rc, Weak},
    future::poll_fn, task::{Poll, Waker}};
use crate::{Action, Context, Theme};

type InvitationParser<T> = Rc<dyn Fn(&str) -> Option<T>>;
type InvitationAction<T> = Rc<dyn Fn(&mut Context, T) -> Action>;

/// URL formatting, validation, and the action to run for an accepted payload.
/// `T` is app-defined: a contact name, document ID, password invitation token, etc.
/// Clones share the same inbox. No action runs inside the native URL callback.
#[derive(Clone)]
pub struct Invitations<T> {
    url: Rc<dyn Fn(T) -> String>,
    parse: InvitationParser<T>,
    on_open: InvitationAction<T>,
    inbox: Rc<RefCell<InvitationInbox<T>>>,
}

impl<T> std::fmt::Debug for Invitations<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invitations").finish_non_exhaustive()
    }
}

impl<T: Clone + std::fmt::Debug + 'static> Invitations<T> {
    pub fn new(
        url: impl Fn(T) -> String + 'static, parse: impl Fn(&str) -> Option<T> + 'static,
        on_open: impl Fn(&mut Context, T) -> Action + 'static,
    ) -> Self {
        Self {
            url: Rc::new(url), parse: Rc::new(parse), on_open: Rc::new(on_open),
            inbox: Rc::new(RefCell::new(InvitationInbox {
                queue: VecDeque::new(), pending: HashSet::new(), waiters: Vec::new(),
            })),
        }
    }

    pub fn url(&self, payload: T) -> String {(self.url)(payload)}
    pub fn parse_invite(&self, input: &str) -> Option<T> {(self.parse)(input.trim())}
    pub fn share_action(&self, payload: T) -> Action {Action::share(&self.url(payload))}

    pub fn receive(&self, url: &str) -> bool {
        let url = url.trim();
        let Some(payload) = self.parse_invite(url) else {return false};
        let mut inbox = self.inbox.borrow_mut();
        if inbox.pending.contains(url) {return true;}
        if inbox.queue.len() == 16 {return false;}
        inbox.queue.push_back((url.to_owned(), payload));
        inbox.pending.insert(url.to_owned());
        let waiters = std::mem::take(&mut inbox.waiters);
        drop(inbox);
        for waiter in waiters.into_iter().filter_map(|waiter| waiter.upgrade()) {
            let waker = waiter.borrow_mut().take();
            if let Some(waker) = waker {waker.wake();}
        }
        true
    }

    /// CHK owns registration. An unavailable platform still supports manual links
    /// and sharing, but cannot deliver native incoming URLs yet.
    pub fn attach(&self, ctx: &Context) -> bool {
        let invitations = self.clone();
        ctx.hardware.listen_app_links(move |url| invitations.receive(url))
    }

}

/// Root drawable that delivers accepted invitations to the application handler.
#[derive(Clone)]
pub struct InvitationLinks<T> {
    inner: Box<dyn Drawable>,
    theme: Theme,
    on_open: InvitationAction<T>,
    inbox: Rc<RefCell<InvitationInbox<T>>>,
}

impl<T> InvitationLinks<T> {
    pub fn new(inner: impl Drawable, theme: Theme, invitations: Invitations<T>) -> Self {
        Self {
            inner: Box::new(inner), theme, on_open: invitations.on_open, inbox: invitations.inbox,
        }
    }
}

impl<T> std::fmt::Debug for InvitationLinks<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InvitationLinks").field("inner", &self.inner).finish_non_exhaustive()
    }
}

impl<T: Clone + std::fmt::Debug + 'static> Drawable for InvitationLinks<T> {
    fn request_width(&mut self) -> (f32, f32) {self.inner.request_width()}
    fn request_height(&mut self, width: f32) -> (f32, f32) {self.inner.request_height(width)}
    fn layout(&mut self, size: (f32, f32)) -> LayoutTree {self.inner.layout(size)}
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        self.inner.draw(layout, offset, bound)
    }
    fn event(&mut self, ctx: &mut Context, layout: &LayoutTree, event: Box<dyn DynEvent>) {
        self.inner.event(ctx, layout, event);
    }
    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            let mut inner = self.inner.as_mut().changed();
            let waiter = Rc::new(RefCell::new(None::<Waker>));
            poll_fn(|cx| {
                let mut inbox = self.inbox.borrow_mut();
                if let Some((url, payload)) = inbox.queue.pop_front() {
                    // Deliver distinct invitations in order. Only duplicate deliveries
                    // still waiting in the queue are coalesced; later taps work again.
                    inbox.pending.remove(&url);
                    return Poll::Ready(Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                        let target = target.downcast_mut::<Self>().unwrap();
                        let action = (target.on_open)(ctx, payload);
                        (action.get())(ctx, &target.theme);
                    }) as ListenCallback);
                }
                *waiter.borrow_mut() = Some(cx.waker().clone());
                inbox.waiters.retain(|waiter| waiter.strong_count() > 0);
                if !inbox.waiters.iter().any(|existing| existing.ptr_eq(&Rc::downgrade(&waiter))) {
                    inbox.waiters.push(Rc::downgrade(&waiter));
                }
                drop(inbox);
                inner.as_mut().poll(cx).map(|callback| {
                    Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                        let target = target.downcast_mut::<Self>().unwrap();
                        callback(target.inner.as_mut().as_any_mut(), ctx);
                    }) as ListenCallback
                })
            }).await
        })
    }
}


#[derive(Clone, Debug, Default)]
pub struct ContactInvitations {
    invitations: Option<Invitations<Name>>,
}

impl ContactInvitations {
    pub fn new(invitations: Invitations<Name>) -> Self {Self {invitations: Some(invitations)}}

    pub fn contact_url(&self, name: Name) -> String {
        self.invitations.as_ref().map_or_else(|| name.to_string(), |invitations| invitations.url(name))
    }

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

    pub fn parse_contact(&self, value: &str) -> Option<Name> {
        match &self.invitations {
            Some(invitations) => invitations.parse_invite(value).or_else(|| value.trim().parse().ok()),
            None => value.trim().parse().ok(),
        }
    }
}

struct InvitationInbox<T> {
    queue: VecDeque<(String, T)>,
    pending: HashSet<String>,
    waiters: Vec<Weak<RefCell<Option<Waker>>>>,
}
