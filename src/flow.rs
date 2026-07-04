use pelican_ui::{Callback, Context};
use pelican_ui::navigation::{NavigationEvent, Flow as PelicanFlow, FlowContainer, AppPage};
use pelican_ui::theme::Theme;
use pelican_ui::drawable::{Component, SizedTree};
use pelican_ui::layout::Stack;
use pelican_ui::event::OnEvent;
use ramp::prism;
use pelican_ui::event::{Event, TickEvent};

use crate::form::{Form, State, FormComplete};
use crate::items::Input;
use crate::page::{EditPage, PageType, FormPage, ReviewPage, SuccessPage};
use crate::closure::{FormSubmit, NavFn, ScreenBuilder, PageBuilder};
use crate::page::Screen;
use crate::{Action, Bumper, Offset, Display, AvatarContent, AvatarPurpose};
use air::Instance;
use crate::profiles::Profile;

use std::rc::Rc;
use std::cell::RefCell;
use std::fmt::Debug;

#[derive(Debug, Clone, Default)]
pub struct Flow(Vec<Box<dyn ScreenBuilder>>);
impl Flow{
    pub fn new(theme: &Theme, pages: Vec<Box<dyn PageBuilder>>) -> Self {
        let pages = pages.into_iter().map(|p| Screen::new_builder(theme, p)).collect::<Vec<_>>();
        Flow(pages)
    }

    pub fn action_target(theme: &Theme, action: &str, past_action: &str, target: &str, avatar: AvatarContent, purpose: AvatarPurpose) -> Self {
        let action_caps = action.chars().next().map(|c| c.to_uppercase().collect::<String>() + &action[c.len_utf8()..]).unwrap_or_default();

        let prompt = PageType::display(&format!("{} user", action_caps), 
            vec![
                Display::avatar(avatar.clone(), purpose.clone()),
                Display::confirmation_message(&format!("Are you sure you want to {} {}", action, target.clone())),
            ], 
            None, 
            Bumper::double(&action_caps, Action::None, "Cancel", Action::None), 
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

        Self::new(theme, vec![
            Box::new(move || prompt.clone()),
            Box::new(move || complete.clone()),
        ])
    }

    pub fn from_form(form: Form) -> Self {
        match form {
            Form::Flow {theme, inputs, on_submit, review, success} => {
                let theme = theme.clone();
                let mut pages: Vec<Box<dyn ScreenBuilder>> = vec![];

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
                    let page = Box::new(move || PageType::form(&input.title(), input.build(), input.validation(), submit.clone())) as Box<dyn PageBuilder>;
                    Screen::new_builder(&theme, page)
                }).collect::<Vec<Box<dyn ScreenBuilder>>>().into_iter().rev().for_each(|s| pages.push(s));

                if let Some(review) = review {
                    let review = Box::new(move || {
                        let review = review.clone();
                        PageType::review(&review.title, review.getter, on_submit.clone())
                    }) as Box<dyn PageBuilder>;

                    pages.push(Screen::new_builder(&theme, review));
                }

                if let Some(success) = success {
                    let success = Box::new(move || {
                        println!("Built sucess");
                        let submit = submit.take();
                        let success = success.clone();
                        PageType::success(&success.title, success.getter, submit.clone())
                    }) as Box<dyn PageBuilder>;
                    pages.push(Screen::new_builder(&theme, success));
                }

                Flow(pages)
            },
            Form::Page {theme, title, inputs, display, on_save} => {
                let theme = theme;
                let validations = inputs.iter().map(|i| i.validation()).collect::<Vec<_>>();
                let items = inputs.into_iter().map(|i| i.build()).collect::<Vec<_>>();
                let page = Box::new(move || PageType::edit(&title, items.clone(), display.clone(), validations.clone(), on_save.clone())) as Box<dyn PageBuilder>;
                Flow(vec![Screen::new_builder(&theme, page)])
            }
        }
    }

    pub(crate) fn build(&mut self, ctx: &mut Context) -> Box<dyn Callback> {
        let flow = self.build_as_flow(ctx);

        Box::new(move |ctx: &mut Context, _: &Theme| {
            ctx.emit(NavigationEvent::push(flow.clone()));
        })
    }

    pub(crate) fn build_as_flow(&mut self, ctx: &mut Context) -> FlowWrapper {
        let mut new: Vec<Box<dyn AppPage>> = vec![];
        let length = self.0.len();
        if self.0.is_empty() { return FlowWrapper::new(PelicanFlow::new(vec![])) }

        let mut pages = self.0.clone();
        let mut first = pages.remove(0);
        let mut next_fn: Option<NavFn> = None;

        pages.into_iter().rev().for_each(|mut page| {
            // let callback = (i == 0).then_some(self.1.clone()).flatten(); 
            let mut page: Screen = (page)(ctx);
            page.update(ctx, length, next_fn.take());
            new.push(Box::new(page));
            next_fn = Some(NavFn(Rc::new(RefCell::new(move |ctx: &mut Context, _: &Theme| {
                // if let Some(cb) = callback.clone() { (cb.clone())(ctx) } // on_submit
                ctx.emit(NavigationEvent::Next);
            }))));
        });

        let mut first = (first)(ctx);
        if !new.is_empty() { first.update(ctx, length, next_fn.clone()); }
        new.push(Box::new(first));
        new.reverse();

        FlowWrapper::new(PelicanFlow::new(new.clone()))
    }
    
}

#[derive(Debug, Component, Clone)]
pub struct FlowWrapper(Stack, PelicanFlow, #[skip] Vec<State>);
impl OnEvent for FlowWrapper {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {        
        if event.downcast_ref::<TickEvent>().is_some() {
            if let Some(screen) = self.1.current.as_mut().unwrap().downcast_mut::<Screen>().as_mut() && let Some(page) = screen.1.downcast_mut::<ReviewPage>() {
                page.on_change(self.2.clone());
            } else if let Some(screen) = self.1.current.as_mut().unwrap().downcast_mut::<Screen>().as_mut() && let Some(page) = screen.1.downcast_mut::<SuccessPage>() {
                page.on_change(ctx, self.2.clone());
            } else {
                let index = self.1.index;
                self.2 = Vec::new();
                if self.1.stored.is_empty() && let Some(screen) = self.1.current.as_mut().unwrap().downcast_mut::<Screen>() {
                    if let Some(page) = screen.1.downcast_mut::<FormPage>() {
                        page.1.content.children().iter().for_each(|child| Input::store_in(child, &mut self.2));
                        page.on_change(self.2.clone());
                    } else if let Some(page) = screen.1.downcast_mut::<EditPage>() {
                        page.1.content.children().iter().for_each(|child| Input::store_in(child, &mut self.2));
                        page.on_change(self.2.clone());
                    }
                }

                for (i, each) in self.1.stored.iter_mut().enumerate() {
                    if i == index && let Some(screen) = self.1.current.as_mut().unwrap().downcast_mut::<Screen>() {
                        if let Some(page) = screen.1.downcast_mut::<FormPage>() {
                            page.1.content.children().iter().for_each(|child| Input::store_in(child, &mut self.2));
                            page.on_change(self.2.clone());
                        } else if let Some(page) = screen.1.downcast_mut::<EditPage>() {
                            page.1.content.children().iter().for_each(|child| Input::store_in(child, &mut self.2));
                            page.on_change(self.2.clone());
                        }
                    }

                    if let Some(screen) = each.downcast_mut::<Screen>() {
                        if let Some(page) = screen.1.downcast_mut::<FormPage>() {
                            page.1.content.children().iter().for_each(|child| Input::store_in(child, &mut self.2));
                            page.on_change(self.2.clone());
                        } else if let Some(page) = screen.1.downcast_mut::<EditPage>() {
                            page.1.content.children().iter().for_each(|child| Input::store_in(child, &mut self.2));
                            page.on_change(self.2.clone());
                        }
                    }
                }
            }
        }
        vec![event]
    }
}

impl FlowWrapper {
    pub fn new(flow: PelicanFlow) -> Self {Self(Stack::default(), flow, vec![])}
}

impl FlowContainer for FlowWrapper {
    fn flow(&mut self) -> &mut PelicanFlow {&mut self.1}
}

