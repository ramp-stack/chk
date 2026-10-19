use pelican_ui::{Callback, Context};
use pelican_ui::navigation::{NavigationEvent, Flow as PelicanFlow, FlowContainer, AppPage};
use pelican_ui::theme::Theme;
use prism::drawable::Component;
use ptsd::layout::Stack;
use prism::events::{on_event, OnEvent};
use prism::events::{DynEvent, TickEvent};

use crate::form::{Form, State, FormComplete};
use crate::items::Input;
use crate::page::{Page, EditPage, PageType, FormPage, ReviewPage, SuccessPage};
use crate::closure::{FormSubmit, NavFn};
use crate::{Listener, Action, Bumper, Offset, Display, AvatarContent, AvatarPurpose};

use std::rc::Rc;
use std::cell::RefCell;
use std::fmt::Debug;

#[derive(Debug, Clone, Default)]
pub struct Flow(pub Vec<Page>);
impl Flow{
    pub fn new(builders: Vec<Page>) -> Self {Flow(builders)}

    pub fn action_target(action: (&str, Action), past_action: &str, target: &str, avatar: AvatarContent, purpose: AvatarPurpose) -> Self {
        let (action_str, action) = action;
        let action_caps = action_str.chars().next().map(|c| c.to_uppercase().collect::<String>() + &action_str[c.len_utf8()..]).unwrap_or_default();

        let prompt = PageType::display(&format!("{} user", action_caps),
            vec![
                Display::avatar(avatar.clone(), purpose.clone()),
                Display::confirmation_message(&format!("Are you sure you want to {} {}", action_str, target)),
            ],
            None,
            Bumper::double(&action_caps, action, "Cancel", Action::None),
            Offset::Center
        );

        let complete = PageType::display(&format!("User {}", past_action),
            vec![
                Display::avatar(avatar, purpose),
                Display::confirmation_message(&format!("{} has been {}", target, past_action)),
            ],
            None,
            Bumper::Done,
            Offset::Center
        );

        Flow(vec![Page::Static(prompt), Page::Static(complete)])
    }

    pub fn from_form(form: Form) -> Self {
        match form {
            Form::Flow {theme, inputs, on_submit, review, success} => {
                let theme = theme.clone();
                let mut pages: Vec<Page> = vec![];

                let t = theme.clone();
                let submit = review.is_none().then(|| on_submit.clone());
                let mut submit = submit.map(|mut os| Box::new(move |ctx: &mut Context, state: &Vec<State>| {
                    let theme = t.clone();
                    let mut on_form_complete = (os)(ctx, state);
                    on_form_complete.run(ctx, &theme);

                    FormComplete::None
                }) as Box<dyn FormSubmit>);

                inputs.into_iter().rev().map(|input| {
                    let submit = submit.take();
                    Page::Static(PageType::form(&input.title(), input.build(), input.validation(), submit.clone()))
                }).collect::<Vec<Page>>().into_iter().rev().for_each(|s| pages.push(s));

                if let Some(review) = review {
                    let review = Page::Static(PageType::review(&review.title, review.getter, on_submit.clone()));
                    pages.push(review);
                }

                if let Some(success) = success {
                    let success = Page::Static(PageType::success(&success.title, success.getter, submit.clone()));
                    pages.push(success);
                }

                Flow(pages)
            },
            Form::Page {title, inputs, display, on_save} => {
                let validations = inputs.iter().map(|i| i.validation()).collect::<Vec<_>>();
                let items = inputs.into_iter().map(|i| i.build()).collect::<Vec<_>>();
                let page = Page::Static(PageType::edit(&title, items.clone(), display.clone(), validations.clone(), on_save.clone()));
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
        if self.0.is_empty() { return FlowWrapper::new(PelicanFlow::new(vec![])) }

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

        FlowWrapper::new(PelicanFlow::new(new.clone()))
    }
}

#[derive(Debug, Component, Clone)]
pub struct FlowWrapper(Stack, PelicanFlow);
impl FlowWrapper {
    pub fn new(flow: PelicanFlow) -> Self {Self(Stack::default(), flow)}

    fn inputs(page: &dyn AppPage) -> Vec<State> {
        if let Some(listener) = page.downcast_ref::<Listener>() {
            return Self::inputs(listener.page.as_ref());
        }
        let children = if let Some(page) = page.downcast_ref::<FormPage>() {
            page.1.content.children()
        } else if let Some(page) = page.downcast_ref::<EditPage>() {
            page.1.content.children()
        } else {return vec![]};
        children.iter().filter_map(|child| Input::state(&**child)).collect()
    }

    fn update(page: &mut dyn AppPage, ctx: &mut Context, inputs: Vec<State>) {
        if let Some(listener) = page.downcast_mut::<Listener>() {
            Self::update(listener.page.as_mut(), ctx, inputs);
        } else if let Some(page) = page.downcast_mut::<ReviewPage>() {
            page.on_change(inputs);
        } else if let Some(page) = page.downcast_mut::<SuccessPage>() {
            page.on_change(ctx, inputs);
        } else if let Some(page) = page.downcast_mut::<FormPage>() {
            page.on_change(inputs);
        } else if let Some(page) = page.downcast_mut::<EditPage>() {
            page.on_change(inputs);
        }
    }
}

#[on_event]
impl OnEvent<TickEvent> for FlowWrapper {
    fn on(&mut self, ctx: &mut Context, event: TickEvent) -> Vec<Box<dyn DynEvent>> {
        let mut inputs: Vec<_> = self.1.stored.iter().take(self.1.index)
            .flat_map(|page| Self::inputs(page.as_ref())).collect();
        let current = self.1.current.as_mut().unwrap().as_mut();
        inputs.extend(Self::inputs(current));
        Self::update(current, ctx, inputs);
        prism::events![event]
    }
}

impl FlowContainer for FlowWrapper {
    fn flow(&mut self) -> &mut PelicanFlow {&mut self.1}
}
