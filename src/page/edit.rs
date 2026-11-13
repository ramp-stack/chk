use prism::events::{ChildEvent, DynEvent, Event, OnEvent, on_event};
use prism::drawable::{Component, Drawable};
use crate::design_system::{Context};
use ptsd::layout::{Stack, Offset};
use ptsd::Validate;
use crate::design_system::{Callback, Content, FormEvent, PageView};
use ptsd::navigation::AppPage;
use crate::design_system::Theme;
use ptsd::utils::ValidationFn;

use std::ops::Range;
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::Poll;

use crate::{FormItem, FormValues, FormComplete, Action, Icons};
use crate::items::{Input, Display};
use crate::closure::FormSubmit;

#[derive(Debug, Component, Clone)]
pub struct EditPage(
    Stack, pub PageView,
    #[skip] Box<dyn FormSubmit>,
    #[skip] Option<FormValues>,
    #[skip] bool,
    #[skip] FormValues,
    #[skip] bool,
    #[skip] Vec<(String, Range<usize>)>,
    #[skip] Content,
    #[skip] Theme,
    #[skip] bool,
    #[skip] Option<ptsd::navigation::NavigationEvent>,
);
#[on_event]
impl OnEvent<FormEvent> for EditPage {
    fn on(&mut self, _ctx: &mut Context, event: FormEvent) -> Vec<Box<dyn DynEvent>> {
        match event {
            FormEvent::Changed(id) if id == self.1.content().id() => {
                if self.4 {self.on_change(self.values());}
                self.update_save_button();
            }
            FormEvent::Validated {id, valid} if id == self.1.content().id() => {
                self.6 = valid;
                // Page handles validation too; include the edit requirement so
                // a valid, unchanged form cannot re-enable Save downstream.
                return prism::events![FormEvent::Validated {
                    id, valid: valid && self.values() != self.5 && !self.10,
                }];
            }
            _ => {}
        }

        prism::events![event]
    }
}

#[derive(Debug, Clone)]
struct EditRequest(uuid::Uuid, Option<ptsd::navigation::NavigationEvent>);
impl Event for EditRequest {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Debug, Clone)]
struct EditFinished(uuid::Uuid, FormValues, Content, FormComplete);
impl Event for EditFinished {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[derive(Debug, Clone)]
struct EditDiscard(uuid::Uuid);
impl Event for EditDiscard {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[on_event]
impl OnEvent<EditRequest> for EditPage {
    fn on(&mut self, ctx: &mut Context, event: EditRequest) -> Vec<Box<dyn DynEvent>> {
        if event.0 != self.1.content().id() {return prism::events![event];}
        if self.10 {return vec![];}
        if !self.6 {
            FormComplete::Error("Please correct the highlighted fields before saving.".into()).run(ctx, &self.9);
            return vec![];
        }
        if self.values() == self.5 {return vec![];}
        self.10 = true;
        self.11 = event.1;
        self.update_save_button();
        let saved = self.values();
        let snapshot = self.1.content().clone();
        let values = self.3.clone().unwrap_or_else(|| saved.clone());
        let result = (self.2)(ctx, &values);
        if let FormComplete::Pending(pending) = result {
            let id = self.1.content().id();
            FormComplete::pending(async move {
                let result = pending.0.await.resolved().await;
                FormComplete::Action(Action::custom(move |ctx: &mut Context, _: &Theme| {
                    ctx.emit(EditFinished(id, saved.clone(), snapshot.clone(), result.clone()));
                }))
            }).run(ctx, &self.9);
        } else {self.finish(ctx, saved, snapshot, result);}
        vec![]
    }
}
#[on_event]
impl OnEvent<EditFinished> for EditPage {
    fn on(&mut self, ctx: &mut Context, event: EditFinished) -> Vec<Box<dyn DynEvent>> {
        if event.0 != self.1.content().id() {return prism::events![event];}
        self.finish(ctx, event.1, event.2, event.3);
        vec![]
    }
}
#[on_event]
impl OnEvent<EditDiscard> for EditPage {
    fn on(&mut self, ctx: &mut Context, event: EditDiscard) -> Vec<Box<dyn DynEvent>> {
        if event.0 != self.1.content().id() {return prism::events![event];}
        *self.1.content_mut() = self.8.clone();
        self.3 = None;
        self.on_change(self.values());
        ctx.emit(FormEvent::Refresh(self.1.content().id()));
        vec![]
    }
}
impl AppPage for EditPage {}
impl EditPage {
    fn view(theme: &Theme, title: &str, action: Option<(Icons, Box<dyn Callback>)>,
        content: Content, root: bool) -> PageView {
        let (header, footer) = if root {
            (crate::design_system::Header::home(title, action),
                crate::design_system::Footer::home(Some(("Save".into(), Box::new(|_: &mut Context, _: &Theme| {}))), None))
        } else {
            (crate::design_system::Header::stack(title, action),
                crate::design_system::Footer::stack(Some("Save"), Box::new(|_: &mut Context, _: &Theme| {}), None))
        };
        theme.page(header, content, Some(footer))
    }

    pub fn new(
        theme: &Theme, is_mobile: bool, title: String, input: Vec<Input>, display: Vec<Display>,
        validations: Vec<Box<dyn ValidationFn>>, ids: Vec<String>, on_save: Box<dyn FormSubmit>,
    ) -> Self {
        assert_eq!(ids.len(), input.len(), "Each form input needs one field ID");
        let header = None;
        let (content, ranges) = Self::content(theme, is_mobile, input, display, validations);
        let ids = ids.into_iter().zip(ranges).collect::<Vec<_>>();

        let mut page = Self::view(theme, &title, header, content, false);
        let snapshot = page.content().clone();
        let initial = Self::read_values(page.content(), &ids);
        page.disable_actions(true);
        Self(Stack::default(), page, on_save, None, false, initial, false, ids, snapshot,
            theme.clone(), false, None)
    }

    pub fn edit_and_display(
        theme: &Theme, is_mobile: bool, title: String,
        mut header: Option<(Icons, Action)>, items: Vec<FormItem>,
        display: Vec<Display>, on_save: Box<dyn FormSubmit>,
    ) -> Self {
        let header = header.as_mut().map(|(icon, action)| (*icon, action.get()));

        let ids = FormItem::ids(&items);
        let validations = items.iter().map(|i| i.validation()).collect::<Vec<_>>();
        let inputs = items.into_iter().map(|i| i.build()).collect::<Vec<Input>>();
        let (content, ranges) = Self::content(theme, is_mobile, inputs, display, validations);
        let ids = ids.into_iter().zip(ranges).collect::<Vec<_>>();

        let mut page = Self::view(theme, &title, header, content, false);
        let snapshot = page.content().clone();
        let initial = Self::read_values(page.content(), &ids);
        page.disable_actions(true);
        Self(Stack::default(), page, on_save, None, false, initial, false, ids, snapshot,
            theme.clone(), false, None)
    }

    pub fn root(
        theme: &Theme, is_mobile: bool, title: String,
        mut header: Option<(Icons, Action)>, items: Vec<FormItem>,
        display: Vec<Display>, on_save: Box<dyn FormSubmit>,
    ) -> Self {
        let header = header.as_mut().map(|(icon, action)| (*icon, action.get()));

        let ids = FormItem::ids(&items);
        let validations = items.iter().map(|i| i.validation()).collect::<Vec<_>>();
        let inputs = items.into_iter().map(|i| i.build()).collect::<Vec<Input>>();
        let (content, ranges) = Self::content(theme, is_mobile, inputs, display, validations);
        let ids = ids.into_iter().zip(ranges).collect::<Vec<_>>();

        let mut page = Self::view(theme, &title, header, content, true);
        let snapshot = page.content().clone();
        let initial = Self::read_values(page.content(), &ids);
        page.disable_actions(true);
        Self(Stack::default(), page, on_save, None, true, initial, false, ids, snapshot,
            theme.clone(), false, None)
    }

    fn read_values(content: &Content, fields: &[(String, Range<usize>)]) -> FormValues {
        let children = content.children();
        FormValues::new(fields.iter().map(|(id, range)| {
            let mut states = children[range.clone()].iter().filter_map(|child| Input::state(&**child));
            let state = states.next().expect("Each form field must produce one value");
            assert!(states.next().is_none(), "A field must not produce multiple values");
            (id.clone(), state)
        }))
    }

    pub(crate) fn values(&self) -> FormValues {Self::read_values(self.1.content(), &self.7)}

    fn finish(&mut self, ctx: &mut Context, saved: FormValues, snapshot: Content, mut result: FormComplete) {
        self.10 = false;
        let leave = self.11.take();
        if !matches!(result, FormComplete::Error(_)) {
            self.5 = saved;
            self.8 = snapshot;
        }
        self.update_save_button();
        if let Some(leave) = leave {
            if matches!(result, FormComplete::Error(_)) {result.run(ctx, &self.9);}
            else if self.values() == self.5 {ctx.emit(leave);}
            // Edits typed during an asynchronous save remain dirty and visible.
        } else {result.run(ctx, &self.9);}
    }

    fn update_save_button(&mut self) {
        let enabled = self.6 && self.values() != self.5 && !self.10;
        self.1.disable_actions(!enabled);
    }

    fn content(theme: &Theme, is_mobile: bool, inputs: Vec<Input>, display: Vec<Display>, validations: Vec<Box<dyn ValidationFn>>) -> (Content, Vec<Range<usize>>) {
        let mut children = Vec::new();
        let mut validators = Vec::new();
        let mut ranges = Vec::new();
        let mut validations = validations.into_iter();
        for input in inputs {
            let start = children.len();
            children.extend(input.build(theme, is_mobile).unwrap_or_default());
            ranges.push(start..children.len());
            if let Some(validation) = validations.next() {
                validators.push((start..children.len(), validation));
            }
        }
        for mut item in display { children.extend(item.build(theme).unwrap_or_default()); }
        (crate::design_system::Content::new(Offset::Start, children, Box::new(FormValidation(validators))), ranges)
    }

    pub fn on_change(&mut self, new: FormValues) {
        self.update_save_button();
        self.3 = Some(new);
        let id = self.1.content().id();
        self.1.on_primary(Box::new(move |ctx: &mut Context| {
            ctx.emit(EditRequest(id, None));
        }));
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

#[derive(Debug, Clone)]
pub(crate) struct DeferredSubmit(pub crate::PendingSubmit);
impl Event for DeferredSubmit {
    fn pass(&self, _: &mut Context, _: &mut [ChildEvent<'_, Self>]) {}
}

#[derive(Debug, Clone, Default)]
struct CheckEdits(std::rc::Rc<std::cell::RefCell<Option<(uuid::Uuid, bool)>>>);
impl Event for CheckEdits {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
#[on_event]
impl OnEvent<CheckEdits> for EditPage {
    fn on(&mut self, _: &mut Context, event: CheckEdits) -> Vec<Box<dyn DynEvent>> {
        if self.10 || self.values() != self.5 {*event.0.borrow_mut() = Some((self.1.content().id(), self.10));}
        vec![]
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

    fn confirm(&self, ctx: &mut Context, id: uuid::Uuid, navigation: ptsd::navigation::NavigationEvent) {
        use crate::{ActionItem, Bumper, Flow, Page, PageType};
        use ptsd::navigation::NavigationEvent;
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
}
impl Drawable for FormHost {
    fn request_width(&mut self) -> (f32, f32) {self.inner.request_width()}
    fn request_height(&mut self, width: f32) -> (f32, f32) {self.inner.request_height(width)}
    fn layout(&mut self, size: (f32, f32)) -> prism::drawable::LayoutTree {self.inner.layout(size)}
    fn draw(&self, layout: &prism::drawable::LayoutTree, offset: prism::drawable::Offset,
        bound: prism::drawable::Rect) -> Vec<prism::drawable::Instruction> {self.inner.draw(layout, offset, bound)}
    fn event(&mut self, ctx: &mut Context, layout: &prism::drawable::LayoutTree, event: Box<dyn DynEvent>) {
        use ptsd::navigation::NavigationEvent;
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
    fn changed(&mut self) -> prism::events::ListenFuture<'_> {
        use prism::events::ListenCallback;
        Box::pin(async move {
            let mut pending = self.pending.iter().map(|pending| Box::pin(pending.0.clone())).collect::<Vec<_>>();
            let mut inner = self.inner.as_mut().changed();
            poll_fn(|cx| {
                for (index, pending) in pending.iter_mut().enumerate() {
                    if let Poll::Ready(mut result) = pending.as_mut().poll(cx) {
                        return Poll::Ready(Box::new(move |target: &mut dyn std::any::Any, ctx: &mut Context| {
                            let target = target.downcast_mut::<Self>().unwrap();
                            target.pending.remove(index);
                            result.run(ctx, &target.theme);
                        }) as ListenCallback);
                    }
                }
                if let Poll::Ready(callback) = inner.as_mut().poll(cx) {
                    return Poll::Ready(Box::new(move |target: &mut dyn std::any::Any, ctx: &mut Context| {
                        let target = target.downcast_mut::<Self>().unwrap();
                        callback(target.inner.as_mut().as_any_mut(), ctx);
                    }) as ListenCallback);
                }
                Poll::Pending
            }).await
        })
    }
}

#[on_event]
impl OnEvent<CheckEdits> for crate::FormPage {
    fn on(&mut self, _: &mut Context, event: CheckEdits) -> Vec<Box<dyn DynEvent>> {
        if self.submitting() {*event.0.borrow_mut() = Some((self.1.content().id(), true));}
        vec![]
    }
}
#[on_event]
impl OnEvent<CheckEdits> for crate::ReviewPage {
    fn on(&mut self, _: &mut Context, event: CheckEdits) -> Vec<Box<dyn DynEvent>> {
        if self.submitting() {*event.0.borrow_mut() = Some((self.1.content().id(), true));}
        vec![]
    }
}
