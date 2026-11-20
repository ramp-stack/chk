use prism::Context;
use prism::drawable::{Drawable, Instruction, LayoutTree, Rect};
use prism::events::{ChildEvent, DynEvent, Event, ListenCallback, ListenFuture};
use ptsd::layout::Offset;
use ptsd::navigation::NavigationEvent;
use ptsd::utils::ValidationFn;
use ptsd::Validate;
use uuid::Uuid;

use std::{
    any::Any, cell::RefCell, future::{Future, poll_fn}, ops::Range, pin::Pin, rc::Rc, task::Poll,
};

use crate::closure::{FormSubmit, NavFn};
use crate::design_system::{Callback, Content, Footer, FormEvent, Header, Icons, PageView, Theme};
use crate::form::{FormComplete, FormValues};
use crate::items::{Action, ActionItem, Display, FormItem};
use crate::{Bumper, Flow, Page, PageType, StackPage};

#[derive(Debug, Clone)]
pub struct SubmitRequest(pub Uuid);
impl Event for SubmitRequest {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}

#[derive(Debug, Clone)]
pub struct SubmitFinished(pub Uuid, pub FormComplete);
impl Event for SubmitFinished {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}

#[derive(Debug, Clone)]
pub struct DeferredSubmit(pub crate::PendingSubmit);
impl Event for DeferredSubmit {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}

#[derive(Debug, Clone, Default)]
pub struct Submission {
    pending: bool,
}
impl Submission {
    pub fn pending(&self) -> bool {self.pending}
    pub fn finish(&mut self, page: &mut PageView) {
        self.pending = false;
        page.restore_actions();
    }

    pub fn start(&mut self, page: &mut PageView, result: FormComplete, ctx: &mut Context, theme: &Theme, next: Option<crate::NavFn>) {
        match result {
            FormComplete::Pending(pending) => {
                self.pending = true;
                page.disable_actions(true);
                let id = page.content().id();
                FormComplete::pending(async move {
                    let result = pending.await.resolved().await;
                    FormComplete::Action(Action::custom(move |ctx: &mut Context, _: &Theme| {
                        ctx.emit(SubmitFinished(id, result.clone()));
                    }))
                }).run(ctx, theme);
            }
            result => result.then(next).run(ctx, theme),
        }
    }

    pub fn check_navigation(&self, id: Uuid, event: &dyn DynEvent) -> bool {
        let Some(check) = event.downcast_ref::<CheckEdits>() else {return false;};
        if self.pending() {*check.0.borrow_mut() = Some((id, true));}
        true
    }
}

/// Owns deferred submissions and checks the active editor before destructive navigation.
/// App::build installs one host for the whole interface, including incoming invitations.
#[derive(Debug, Clone)]
pub struct FormHost {
    inner: Box<dyn Drawable>,
    theme: Theme,
    pending: Vec<crate::PendingSubmit>,
}
impl FormHost {
    pub fn new(inner: impl Drawable, theme: Theme) -> Self {
        Self {inner: Box::new(inner), theme, pending: vec![]}
    }

    pub fn confirm(&self, ctx: &mut Context, id: Uuid, navigation: NavigationEvent) {
        let save_navigation = navigation.clone();
        let save = Action::custom(move |ctx: &mut Context, _: &Theme| {
            let navigation = save_navigation.clone();
            ctx.emit(NavigationEvent::reset_with_fn(move |ctx: &mut Context| {
                ctx.emit(EditRequest(id, Some(navigation.clone())));
            }));
        });
        let discard = Action::custom(move |ctx: &mut Context, _: &Theme| {
            let navigation = navigation.clone();
            ctx.emit(NavigationEvent::reset_with_fn(move |ctx: &mut Context| {
                ctx.emit(EditDiscard(id));
                ctx.emit(navigation.clone());
            }));
        });
        let keep = Action::custom(|ctx: &mut Context, _: &Theme| ctx.emit(NavigationEvent::Reset));
        let page = PageType::display("Unsaved changes", vec![
            Display::instructions("Save your changes before leaving?"),
            Display::actions(vec![
                ActionItem::new(save, "Save and leave", Icons::Forward),
                ActionItem::new(discard, "Discard changes", Icons::Block),
                ActionItem::new(keep, "Keep editing", Icons::Edit),
            ], false),
        ], None, Bumper::None, Offset::Center);
        (Flow::new(vec![Page::Static(Box::new(page))]).build(ctx, &self.theme))(ctx, &self.theme);
    }

    pub fn build_form(
        ctx: &mut Context, theme: &Theme, id: String, title: &str, content: Content,
        next: Option<NavFn>, mut on_submit: Option<Box<dyn FormSubmit>>,
    ) -> StackPage<PageView> {
        let fields = vec![(id, 0..content.children().len())];
        let id = content.id();
        let footer = Footer::stack(
            None, Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(SubmitRequest(id))), None,
        );
        let event_theme = theme.clone();
        let mut values = None;
        let mut submission = Submission::default();
        let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
            if submission.check_navigation(id, event) {return true;}
            if let Some(new) = event.downcast_ref::<FormValues>() {
                values = Some(new.clone());
                return true;
            }
            if let Some(SubmitRequest(target)) = event.downcast_ref::<SubmitRequest>() {
                if *target != id {return false;}
                if submission.pending() {return true;}
                let values = values.clone().unwrap_or_else(|| page.values());
                let result = on_submit.as_mut().map(|submit| submit(ctx, &values))
                    .unwrap_or(FormComplete::None);
                submission.start(page, result, ctx, &event_theme, next.clone());
                return true;
            }
            if let Some(SubmitFinished(target, result)) = event.downcast_ref::<SubmitFinished>() {
                if *target != id {return false;}
                submission.finish(page);
                result.clone().then(next.clone()).run(ctx, &event_theme);
                ctx.emit(FormEvent::Refresh(id));
                return true;
            }
            if let Some(FormEvent::Validated {id: target, ..}) = event.downcast_ref::<FormEvent>()
                && *target == id && submission.pending() {
                page.disable_actions(true);
                return true;
            }
            false
        });
        let page = theme.page(
            ctx, Header::stack(title, None), content, Some(footer), fields,
            true, None, Some(on_event), None, None,
        );
        StackPage::new(page)
    }

    pub fn build_edit(page: &PageType, ctx: &mut Context, theme: &Theme, is_root: bool) -> StackPage<PageView> {
        let PageType::Edit {title, header, items, display, mut on_save, ..} = page.clone() else {
            panic!("Expected an edit page");
        };
        let ids = FormItem::ids(&items);
        let mut children = Vec::new();
        let mut validators = Vec::new();
        let mut ranges = Vec::new();
        for item in items {
            let validation = item.validation;
            let start = children.len();
            children.extend(item.input.build(theme, ctx.window.is_mobile));
            let range = start..children.len();
            validators.push((range.clone(), validation));
            ranges.push(range);
        }
        for item in display {children.extend(item.build(ctx, theme));}
        let content = Content::new(Offset::Start, children, Box::new(FormValidation(validators)));
        let id = content.id();
        let on_primary: Box<dyn Callback> =
            Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(EditRequest(id, None)));
        let action = header.map(|(icon, action)| (icon, action.get()));
        let (header, footer) = if is_root {
            (Header::home(&title, action), Footer::home(Some(("Save".into(), on_primary)), None))
        } else {
            (Header::stack(&title, action), Footer::stack(Some("Save"), on_primary, None))
        };
        let fields = ids.into_iter().zip(ranges).collect::<Vec<_>>();
        let event_theme = theme.clone();
        let mut values: Option<FormValues> = None;
        let mut saved_values = FormValues::from_content(&content, &fields);
        let mut saved_content = content.clone();
        let mut valid = false;
        let mut saving = false;
        let mut leave = None;
        let on_event = Box::new(move |ctx: &mut Context, page: &mut PageView, event: &dyn DynEvent| {
            if let Some(check) = event.downcast_ref::<CheckEdits>() {
                if saving || page.values() != saved_values {
                    *check.0.borrow_mut() = Some((id, saving));
                }
                return true;
            }
            if let Some(new) = event.downcast_ref::<FormValues>() {
                values = Some(new.clone());
                page.disable_actions(!valid || page.values() == saved_values || saving);
                return true;
            }
            if let Some(form) = event.downcast_ref::<FormEvent>() {
                match form {
                    FormEvent::Changed(target) if *target == id => {
                        if is_root {values = Some(page.values());}
                        page.disable_actions(!valid || page.values() == saved_values || saving);
                    }
                    FormEvent::Validated {id: target, valid: current} if *target == id => {
                        valid = *current;
                        page.disable_actions(!valid || page.values() == saved_values || saving);
                        return true;
                    }
                    _ => {}
                }
            }
            if let Some(EditDiscard(target)) = event.downcast_ref::<EditDiscard>() {
                if *target != id {return false;}
                *page.content_mut() = saved_content.clone();
                values = Some(page.values());
                page.disable_actions(!valid || page.values() == saved_values || saving);
                ctx.emit(FormEvent::Refresh(id));
                return true;
            }
            let finished = if let Some(EditRequest(target, navigation)) = event.downcast_ref() {
                if *target != id {return false;}
                if saving {return true;}
                if !valid {
                    FormComplete::Error("Please correct the highlighted fields before saving.".into())
                        .run(ctx, &event_theme);
                    return true;
                }
                let saved = page.values();
                if saved == saved_values {return true;}
                saving = true;
                leave = navigation.clone();
                page.disable_actions(true);
                let snapshot = page.content().clone();
                let result = on_save(ctx, values.as_ref().unwrap_or(&saved));
                if let FormComplete::Pending(pending) = result {
                    FormComplete::pending(async move {
                        let result = pending.await.resolved().await;
                        FormComplete::Action(Action::custom(move |ctx: &mut Context, _: &Theme| {
                            ctx.emit(EditFinished(
                                id, saved.clone(), snapshot.clone(), result.clone(),
                            ));
                        }))
                    }).run(ctx, &event_theme);
                    return true;
                }
                Some(EditFinished(id, saved, snapshot, result))
            } else {
                event.downcast_ref::<EditFinished>().filter(|finished| finished.0 == id).cloned()
            };
            if let Some(EditFinished(_, saved, snapshot, result)) = finished {
                saving = false;
                let navigation = leave.take();
                if !matches!(result, FormComplete::Error(_)) {
                    saved_values = saved;
                    saved_content = snapshot;
                }
                page.disable_actions(!valid || page.values() == saved_values);
                match (result, navigation) {
                    (FormComplete::None, Some(navigation)) if page.values() == saved_values => {
                        ctx.emit(navigation);
                    }
                    (mut result, _) => result.run(ctx, &event_theme),
                }
                return true;
            }
            false
        });
        let page = theme.page(
            ctx, header, content, Some(footer), fields, true, None, Some(on_event), None, None,
        );
        StackPage::new(page)
    }

}
impl Drawable for FormHost {
    fn request_width(&mut self) -> (f32, f32) {self.inner.request_width()}
    fn request_height(&mut self, width: f32) -> (f32, f32) {self.inner.request_height(width)}
    fn layout(&mut self, size: (f32, f32)) -> LayoutTree {self.inner.layout(size)}
    fn draw(&self, layout: &LayoutTree, offset: (f32, f32), bound: Rect) -> Vec<Instruction> {
        self.inner.draw(layout, offset, bound)
    }
    fn event(&mut self, ctx: &mut Context, layout: &LayoutTree, event: Box<dyn DynEvent>) {
        if let Some(deferred) = event.downcast_ref::<DeferredSubmit>() {
            self.pending.push(deferred.0.clone());
            return;
        }
        if let Some(navigation) = event.downcast_ref::<NavigationEvent>() {
            // Push preserves the current page (pickers and dialogs must retain drafts).
            if !matches!(navigation, NavigationEvent::Error(_)) {
                let check = CheckEdits::default();
                self.inner.event(ctx, layout, Box::new(check.clone()));
                if let Some((id, saving)) = *check.0.borrow() {
                    if saving {return;}
                    if !matches!(navigation, NavigationEvent::Push(..)) {
                        self.confirm(ctx, id, navigation.clone());
                        return;
                    }
                }
            }
        }
        self.inner.event(ctx, layout, event);
    }
    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            let mut pending = self.pending.iter()
                .map(|pending| Box::pin(pending.clone())).collect::<Vec<_>>();
            let mut inner = self.inner.as_mut().changed();
            poll_fn(|cx| {
                for (index, pending) in pending.iter_mut().enumerate() {
                    if let Poll::Ready(mut result) = pending.as_mut().poll(cx) {
                        return Poll::Ready(Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                            let target = target.downcast_mut::<FormHost>().unwrap();
                            target.pending.remove(index);
                            result.run(ctx, &target.theme);
                        }) as ListenCallback);
                    }
                }
                if let Poll::Ready(callback) = inner.as_mut().poll(cx) {
                    return Poll::Ready(Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                        let target = target.downcast_mut::<FormHost>().unwrap();
                        callback(target.inner.as_mut().as_any_mut(), ctx);
                    }) as ListenCallback);
                }
                Poll::Pending
            }).await
        })
    }
}

#[derive(Debug, Clone)]
struct EditRequest(Uuid, Option<NavigationEvent>);
impl Event for EditRequest {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Debug, Clone)]
struct EditFinished(Uuid, FormValues, Content, FormComplete);
impl Event for EditFinished {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Debug, Clone)]
struct EditDiscard(Uuid);
impl Event for EditDiscard {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Debug, Clone, Default)]
struct CheckEdits(Rc<RefCell<Option<(Uuid, bool)>>>);
impl Event for CheckEdits {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Clone)]
struct FormValidation(Vec<(Range<usize>, Box<dyn ValidationFn>)>);
impl Validate<Vec<&mut Box<dyn Drawable>>, bool> for FormValidation {
    fn validate(&mut self, ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>) -> bool {
        let mut valid = true;
        for (range, validation) in &mut self.0 {
            let inputs = children[range.clone()].iter_mut().map(|child| &mut **child).collect();
            valid &= validation.validate(ctx, inputs);
        }
        valid
    }
    fn change(&mut self) -> Pin<Box<dyn Future<Output = Option<()>> + '_>> {
        Box::pin(async move {
            let mut waits: Vec<_> =
                self.0.iter_mut().map(|(_, validation)| validation.change()).collect();
            poll_fn(|cx| {
                let mut index = 0;
                while index < waits.len() {
                    match waits[index].as_mut().poll(cx) {
                        Poll::Ready(Some(())) => return Poll::Ready(Some(())),
                        Poll::Ready(None) => {drop(waits.swap_remove(index));}
                        Poll::Pending => {index += 1;}
                    }
                }
                if waits.is_empty() {Poll::Ready(None)} else {Poll::Pending}
            }).await
        })
    }
}

