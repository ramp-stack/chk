//! App-defined invitation payloads and behavior, queued until the UI is ready.
use std::any::Any;
use async_channel::{Receiver, Sender};
use prism::{drawable::{Drawable, Instruction, LayoutTree, Offset, Rect},
    events::{DynEvent, ListenCallback, ListenFuture}};
use crate::{Action, Context, Theme};

/// An application's link format, invitation inbox, and open handler.
/// `T` can represent a contact, document, group, or any app-defined destination.
/// The parser must validate incoming URLs before returning a payload.
/// CHK delivers that payload to `on_open`; it does not create contacts or DMs.
#[derive(Clone, Debug)]
pub struct Invitations<T> {
    format: fn(T) -> String,
    parse: fn(&str) -> Option<T>,
    on_open: fn(&mut Context, &Theme, T),
    inbox: (Sender<T>, Receiver<T>),
}

impl<T: Clone + std::fmt::Debug + 'static> Invitations<T> {
    /// Clones share one inbox between the platform receiver and root interface.
    /// `on_open` runs with the UI context after startup, not in the native callback.
    pub fn new(format: fn(T) -> String, parse: fn(&str) -> Option<T>,
        on_open: fn(&mut Context, &Theme, T)) -> Self {
        Self {format, parse, on_open, inbox: async_channel::bounded(16)}
    }

    pub fn url(&self, payload: T) -> String {(self.format)(payload)}

    pub fn parse_invite(&self, input: &str) -> Option<T> {(self.parse)(input.trim())}

    /// Validate and queue a platform URL, including before the UI exists.
    /// Returns false for an invalid URL or a full queue. Does not call `on_open`.
    pub fn receive(&self, url: &str) -> bool {
        let Some(payload) = self.parse_invite(url) else {return false};
        self.inbox.0.try_send(payload).is_ok()
    }

    /// Attach one consumer to the root interface. When several links are waiting,
    /// the latest pending destination wins, without suppressing future taps.
    pub fn wrap(&self, inner: impl Drawable, theme: Theme) -> InvitationLinks<T> {
        let (sender, receiver) = &self.inbox;
        InvitationLinks {inner: Box::new(inner), theme, on_open: self.on_open,
            receiver: receiver.clone(), _sender: sender.clone()}
    }

    pub fn share_action(&self, payload: T) -> Action {Action::share(&self.url(payload))}
}

/// Root drawable that delivers accepted invitations to the application handler.
#[derive(Clone, Debug)]
pub struct InvitationLinks<T> {
    inner: Box<dyn Drawable>,
    theme: Theme,
    on_open: fn(&mut Context, &Theme, T),
    receiver: Receiver<T>,
    _sender: Sender<T>,
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
                futures::future::Either::Left((Ok(mut name), _)) => {
                    // UIKit may report a cold launch through both launch options and
                    // openURL. Collapse pending deliveries without suppressing future taps.
                    while let Ok(next) = self.receiver.try_recv() {name = next;}
                    Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                        let target = target.downcast_mut::<Self>().unwrap();
                        (target.on_open)(ctx, &target.theme, name);
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
