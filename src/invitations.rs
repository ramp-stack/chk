//! Typed invitation setup supplied by an app and managed by CHK.
use std::{any::Any, cell::RefCell, collections::HashSet, rc::Rc};
use async_channel::{Receiver, Sender};
use prism::{drawable::{Drawable, Instruction, LayoutTree, Offset, Rect},
    events::{DynEvent, ListenCallback, ListenFuture}};
use crate::{Action, Context, Theme};
use prism::air::Name;

/// URL formatting, validation, and the action to run for an accepted payload.
/// `T` is app-defined: a contact name, document ID, password invitation token, etc.
/// Clones share the same inbox. No action runs inside the native URL callback.
#[derive(Clone)]
pub struct Invitations<T> {
    url: Rc<dyn Fn(T) -> String>,
    parse: Rc<dyn Fn(&str) -> Option<T>>,
    on_open: Rc<dyn Fn(&mut Context, T) -> Action>,
    inbox: (Sender<(String, T)>, Receiver<(String, T)>),
    pending: Rc<RefCell<HashSet<String>>>,
}

impl<T> std::fmt::Debug for Invitations<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invitations").finish_non_exhaustive()
    }
}

impl<T: Clone + std::fmt::Debug + 'static> Invitations<T> {
    pub fn new(url: impl Fn(T) -> String + 'static,
        parse: impl Fn(&str) -> Option<T> + 'static,
        on_open: impl Fn(&mut Context, T) -> Action + 'static) -> Self {
        Self {url: Rc::new(url), parse: Rc::new(parse), on_open: Rc::new(on_open),
            inbox: async_channel::bounded(16), pending: Rc::new(RefCell::new(HashSet::new()))}
    }

    pub fn url(&self, payload: T) -> String {(self.url)(payload)}
    pub fn parse_invite(&self, input: &str) -> Option<T> {(self.parse)(input.trim())}
    pub fn share_action(&self, payload: T) -> Action {Action::share(&self.url(payload))}

    pub fn receive(&self, url: &str) -> bool {
        let url = url.trim();
        let Some(payload) = self.parse_invite(url) else {return false};
        let mut pending = self.pending.borrow_mut();
        if pending.contains(url) {return true;}
        if self.inbox.0.try_send((url.to_owned(), payload)).is_err() {return false;}
        pending.insert(url.to_owned());
        true
    }

    /// CHK owns registration. An unavailable platform still supports manual links
    /// and sharing, but cannot deliver native incoming URLs yet.
    pub(crate) fn attach(&self, ctx: &Context) -> bool {
        let invitations = self.clone();
        ctx.hardware.listen_app_links(move |url| invitations.receive(url))
    }

    pub(crate) fn wrap(&self, inner: impl Drawable, theme: Theme) -> InvitationLinks<T> {
        let (sender, receiver) = &self.inbox;
        InvitationLinks {inner: Box::new(inner), theme, on_open: self.on_open.clone(),
            receiver: receiver.clone(), _sender: sender.clone(), pending: self.pending.clone()}
    }
}

/// Root drawable that delivers accepted invitations to the application handler.
#[derive(Clone)]
pub struct InvitationLinks<T> {
    inner: Box<dyn Drawable>,
    theme: Theme,
    on_open: Rc<dyn Fn(&mut Context, T) -> Action>,
    receiver: Receiver<(String, T)>,
    _sender: Sender<(String, T)>,
    pending: Rc<RefCell<HashSet<String>>>,
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
            let receive = Box::pin(self.receiver.recv());
            match futures::future::select(receive, self.inner.as_mut().changed()).await {
                futures::future::Either::Left((Ok((url, payload)), _)) => {
                    // Deliver distinct invitations in order. Only duplicate deliveries
                    // still waiting in the queue are coalesced; later taps work again.
                    self.pending.borrow_mut().remove(&url);
                    Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                        let target = target.downcast_mut::<Self>().unwrap();
                        let action = (target.on_open)(ctx, payload);
                        (action.get())(ctx, &target.theme);
                    }) as ListenCallback
                }
                futures::future::Either::Right((callback, _)) => {
                    Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                        let target = target.downcast_mut::<Self>().unwrap();
                        callback(target.inner.as_mut().as_any_mut(), ctx);
                    }) as ListenCallback
                }
                _ => unreachable!("the invitation view retains its sender"),
            }
        })
    }
}



#[derive(Clone, Debug, Default)]
pub struct ContactInvitations {
    pub(crate) invitations: Option<Invitations<Name>>,
}

impl ContactInvitations {
    pub fn new(invitations: Invitations<Name>) -> Self {Self {invitations: Some(invitations)}}

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

    pub(crate) fn parse_contact(&self, value: &str) -> Option<Name> {
        match &self.invitations {
            Some(invitations) => invitations.parse_invite(value).or_else(|| value.trim().parse().ok()),
            None => value.trim().parse().ok(),
        }
    }
}
