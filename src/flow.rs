use prism::drawable::Component;
use prism::events::{on_event, OnEvent, DynEvent, on_change, OnChange};
use ptsd::layout::Stack;
use ptsd::navigation::{NavigationEvent, NavigationChanged, Flow as NavigationFlow, FlowContainer, AppPage};
use std::{rc::Rc, cell::RefCell, fmt::Debug};
use crate::ContentSurface;
use crate::design_system::{Callback, Context, FormEvent, Theme};
use crate::form::{Form, FormValues};
use crate::page::{Page, EditPage, PageType, FormPage, ReviewPage, SuccessPage};
use crate::closure::{NavFn};
use crate::{Listener, PageChanged, Action, Bumper, Offset, Display, AvatarContent, AvatarPurpose};

#[derive(Debug, Clone, Default)]
pub struct Flow(pub Vec<Page>);
impl Flow {
    pub fn new(builders: Vec<Page>) -> Self {Flow(builders)}

    pub fn action_target(
        action: (&str, Action), past_action: &str, target_kind: &str,
        target: &str, avatar: AvatarContent, purpose: AvatarPurpose,
    ) -> Self {
        let (action_str, action) = action;
        let action_caps = action_str.chars().next().map(|c| c.to_uppercase().collect::<String>() + &action_str[c.len_utf8()..]).unwrap_or_default();
        let target_caps = target_kind.chars().next().map(|c| c.to_uppercase().collect::<String>() + &target_kind[c.len_utf8()..]).unwrap_or_default();

        let prompt = PageType::display(&format!("{} {}", action_caps, target_kind),
            vec![
                Display::avatar(avatar.clone(), purpose.clone()),
                Display::confirmation_message(&format!("Are you sure you want to {} {}", action_str, target)),
            ],
            None,
            Bumper::double(&action_caps, action, "Cancel", Action::None),
            Offset::Center
        );

        let complete = PageType::display(&format!("{} {}", target_caps, past_action),
            vec![
                Display::avatar(avatar, purpose),
                Display::confirmation_message(&format!("{} has been {}", target, past_action)),
            ],
            None,
            Bumper::Done,
            Offset::Center
        );

        Flow(vec![Page::Static(Box::new(prompt)), Page::Static(Box::new(complete))])
    }

    pub fn from_form(form: Form) -> Self {
        match form {
            Form::Flow {theme: _, inputs, on_submit, review, success} => {
                let mut pages: Vec<Page> = vec![];

                let mut submit = review.is_none().then(|| on_submit.clone());

                inputs.into_iter().rev().map(|input| {
                    let submit = submit.take();
                    Page::Static(Box::new(PageType::form(
                        &input.id(), &input.title(), input.build(), input.validation(), submit.clone(),
                    )))
                }).collect::<Vec<Page>>().into_iter().rev().for_each(|s| pages.push(s));

                if let Some(review) = review {
                    let review = Page::Static(Box::new(PageType::review(
                        &review.title, review.getter, on_submit.clone(),
                    )));
                    pages.push(review);
                }

                if let Some(success) = success {
                    let success = Page::Static(Box::new(PageType::success(
                        &success.title, success.getter, submit.clone(),
                    )));
                    pages.push(success);
                }

                Flow(pages)
            },
            Form::Page {title, inputs, display, on_save} => {
                let page = Page::Static(Box::new(PageType::edit(
                    &title, None, inputs, display, on_save,
                )));
                Flow(vec![page])
            }
        }
    }

    pub fn build(&mut self, ctx: &mut Context, theme: &Theme) -> Box<dyn Callback> {
        let flow = self.build_as_flow(ctx, theme);

        Box::new(move |ctx: &mut Context, _: &Theme| {
            ctx.emit(NavigationEvent::push(flow.clone()));
        })
    }

    pub fn build_as_flow(&mut self, ctx: &mut Context, theme: &Theme) -> FlowWrapper {
        let mut new: Vec<Box<dyn AppPage>> = vec![];
        let length = self.0.len();
        if self.0.is_empty() { return FlowWrapper::new(NavigationFlow::new(vec![])) }

        let mut pages = self.0.clone();
        let first = pages.remove(0);
        let mut next_fn: Option<NavFn> = None;

        pages.into_iter().rev().for_each(|page| {
            new.push(page.build(ctx, theme, next_fn.take(), length));
            next_fn = Some(NavFn(Rc::new(RefCell::new(move |ctx: &mut Context, _: &Theme| {
                ctx.emit(NavigationEvent::Next);
            }))));
        });

        new.push(first.build(ctx, theme, next_fn.clone(), length));
        new.reverse();

        FlowWrapper::new(NavigationFlow::new(new.clone()))
    }
}

#[derive(Debug, Component, Clone)]
pub struct FlowWrapper(Stack, NavigationFlow, #[skip] bool);
impl FlowWrapper {
    pub fn new(flow: NavigationFlow) -> Self {Self(Stack::default(), flow, true)}

    pub fn refresh(&mut self, ctx: &mut Context) {
        let mut inputs = FormValues::default();
        for page in self.1.stored.iter().take(self.1.index).chain(self.1.current.iter()) {
            let mut page = page.as_ref();
            while let Some(listener) = page.downcast_ref::<Listener>() {page = listener.page.as_ref();}
            if let Some(page) = page.downcast_ref::<FormPage>() {inputs.extend(page.values());}
            else if let Some(page) = page.downcast_ref::<EditPage>() {inputs.extend(page.values());}
        }
        if let Some(current) = &mut self.1.current {
            let mut page = current.as_mut();
            while page.downcast_ref::<Listener>().is_some() {
                page = page.downcast_mut::<Listener>().unwrap().page.as_mut();
            }
            if let Some(page) = page.downcast_mut::<ReviewPage>() {page.on_change(inputs);}
            else if let Some(page) = page.downcast_mut::<SuccessPage>() {page.on_change(ctx, inputs);}
            else if let Some(page) = page.downcast_mut::<FormPage>() {page.on_change(inputs);}
            else if let Some(page) = page.downcast_mut::<EditPage>() {page.on_change(inputs);}
        }
    }
}

#[on_change(async {
    if !self.2 {std::future::pending::<()>().await;}
})]
impl OnChange<()> for FlowWrapper {
    fn on_change(&mut self, ctx: &mut Context, _: ()) {
        self.2 = false;
        self.refresh(ctx);
    }
}

#[on_event]
impl OnEvent<FormEvent> for FlowWrapper {
    fn on(&mut self, ctx: &mut Context, event: FormEvent) -> Vec<Box<dyn DynEvent>> {
        if let FormEvent::Changed(id) = event && let Some(page) = &self.1.current {
            let mut page = page.as_ref();
            while let Some(listener) = page.downcast_ref::<Listener>() {page = listener.page.as_ref();}
            let content = if let Some(page) = page.downcast_ref::<FormPage>() {Some(page.1.content())}
                else if let Some(page) = page.downcast_ref::<EditPage>() {Some(page.1.content())}
                else if let Some(page) = page.downcast_ref::<ReviewPage>() {Some(page.1.content())}
                else {page.downcast_ref::<SuccessPage>().map(|page| page.1.content())};
            if content.is_some_and(|content| content.id() == id) {self.refresh(ctx);}
        }
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<NavigationChanged> for FlowWrapper {
    fn on(&mut self, ctx: &mut Context, event: NavigationChanged) -> Vec<Box<dyn DynEvent>> {
        self.refresh(ctx);
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<PageChanged> for FlowWrapper {
    fn on(&mut self, ctx: &mut Context, event: PageChanged) -> Vec<Box<dyn DynEvent>> {
        if let Some(page) = &self.1.current
            && let Some(listener) = page.downcast_ref::<Listener>() && listener.id() == event.0 {
            self.refresh(ctx);
        }
        prism::events![event]
    }
}

impl FlowContainer for FlowWrapper {
    fn flow(&mut self) -> &mut NavigationFlow {&mut self.1}
}
