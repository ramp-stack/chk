use pelican_ui::{Callback, Context, Request};
use pelican_ui::navigation::{NavigationEvent, Flow as PelicanFlow, FlowContainer, AppPage};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::drawable::{Drawable, Component, SizedTree};
use pelican_ui::layout::Stack;
use pelican_ui::event::OnEvent;
use ramp::prism;
use pelican_ui::event::{Event, TickEvent};
use pelican_ui::components::avatar::AvatarContent;
use pelican_ui::components::list_item::ListItem as PelicanListItem;
use pelican_ui::utils::ValidationFn;

use crate::items::{EnumItem, Input, ListItem, Action};
use crate::page::{PageType, FormPage, ReviewPage, SuccessPage};
use crate::closure::{FormSubmit, FormClosure, NavFn, ScreenBuilder, PageBuilder, ReviewItemGetter, SuccessGetter, ValidityFn};
use crate::page::Screen;

use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct Form {
    theme: Theme,
    inputs: Vec<FormItem>,
    review: Option<Review>,
    success: Option<Success>,
    on_submit: Box<dyn FormSubmit>, 
}

impl Form {
    pub fn new(theme: &Theme, inputs: Vec<FormItem>, review: Option<Review>, success: Option<Success>, on_submit: Box<dyn FormSubmit>) -> Self {
        Form {
            inputs,
            theme: theme.clone(),
            review,
            success,
            on_submit,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Success {
    title: String,
    getter: Box<dyn SuccessGetter>,
}

impl Success {
    pub fn new(title: &str, getter: impl SuccessGetter + 'static) -> Self {
        Success{title: title.to_string(), getter: Box::new(getter)}
    }
}

#[derive(Debug, Clone)]
pub struct Review {
    title: String,
    getter: Box<dyn ReviewItemGetter>,
}

impl Review {
    pub fn new(title: &str, getter: impl ReviewItemGetter + 'static) -> Self {
        Review{title: title.to_string(), getter: Box::new(getter)}
    }
}

#[derive(Debug, Clone)]
pub enum FormItem {
    Text(String, Box<dyn FormClosure>, Option<Vec<(String, Icons, Action)>>, Box<dyn ValidityFn>),
    Number(String, NumberVariant, Box<dyn ValidityFn>),
    Enum(String, Vec<EnumItem>),
    Search(String, Vec<ListItem>),
}

pub struct FormStorage(pub HashMap<String, String>);

impl FormItem {
    pub fn text(text: &str, actions: Option<Vec<(String, Icons, Action)>>, valid: impl ValidityFn + 'static) -> Self {
        let text = text.to_string();
        FormItem::Text(text.to_string(), Box::new(move |storage: &mut FormStorage, value: String| {storage.0.insert(text.to_string(), value);}), actions, Box::new(valid))
    }

    pub fn number(title: &str, number: NumberVariant, valid: impl ValidityFn + 'static) -> Self {
        FormItem::Number(title.to_string(), number, Box::new(valid))
    }

    pub fn enumerator(label: &str, items: Vec<(&str, &str)>) -> Self {
        let items = items.into_iter().map(|(a, b)| {
            EnumItem::new(a, b)
        }).collect::<Vec<EnumItem>>();
        FormItem::Enum(label.to_string(), items)
    }

    pub fn search(title: &str, items: Vec<ListItem>) -> Self {
        FormItem::Search(title.to_string(), items)
    }
}

impl FormItem {
    fn title(&self) -> String {
        match self {
            FormItem::Search(title, ..) |
            FormItem::Text(title, ..) |
            FormItem::Number(title, ..) |
            FormItem::Enum(title, ..) => title.to_string()
        }
    }

    fn validation(&self) -> Box<dyn ValidationFn> {
        match self {
            FormItem::Text(_, _, _, validation) => {
                use pelican_ui::components::TextInput;

                let validation = validation.clone();
                Box::new(move |children: &mut Vec<Box<dyn Drawable>>| {
                    if let Some(input) = children[0].as_any_mut().downcast_mut::<TextInput>() {
                        let result = (validation.clone())(input.value());
                        input.error(result.clone().map(|_| {}));
                        result.is_ok()
                    } else {
                        true
                    }
                })
            },
            FormItem::Number(_, _, validation) => {
                use pelican_ui::components::NumericalInput;
                
                let validation = validation.clone();
                Box::new(move |children: &mut Vec<Box<dyn Drawable>>| {
                    if let Some(input) = children[0].as_any_mut().downcast_mut::<NumericalInput>() {
                        let result = (validation.clone())(input.value());
                        input.error(result.clone());
                        result.is_ok()
                    } else {
                        true
                    }
                })
            },
            _ => Box::new(|_: &mut Vec<Box<dyn Drawable>>| true),
        }
    }

    fn build(&self) -> Input {
        match self {
            FormItem::Text(title, _, actions, _) => {
                let title = title.clone();
                Input::text(&title, false, None, actions.clone())
            },
            FormItem::Number(_, variant, _) => {
                match variant {
                    NumberVariant::Currency => Input::currency("Enter dollar amount"),
                    NumberVariant::Date => Input::date("Enter date"),
                    NumberVariant::Time => Input::time("Enter time"),
                }
            },
            FormItem::Enum(_, items) => {
                Input::enumerator(items.clone())
            },
            FormItem::Search(_, items) => {
                Input::search(items.clone())
            },
        }
    }
}

// Box::new(|children| {
//     let mut result = true;
    // children.iter().for_each(|c|
    //     // TODO: Add rest of catches here. Allow for custom closure.
    //     if let Some(input) = (*c).as_any().downcast_ref::<TextInput>() {
    //         result = !input.value().is_empty() && input.validate();
    //     }
    // );

//     result
// })

#[derive(Debug, Clone, Default)]
pub struct Flow(Vec<Box<dyn ScreenBuilder>>);
impl Flow{
    pub fn new(pages: Vec<Box<dyn ScreenBuilder>>) -> Self {
        Flow(pages)
    }

    pub fn from_form(form: Form) -> Self {
        let theme = form.theme;
        let mut pages: Vec<Box<dyn ScreenBuilder>> = vec![];

        let mut submit = form.review.is_none().then(|| form.on_submit.clone());
        form.inputs.into_iter().rev().map(|input| {
            let submit = submit.take();
            let page = Box::new(move |_: &Theme| PageType::form(&input.title(), input.build(), input.validation(), submit.clone())) as Box<dyn PageBuilder>;
            Screen::new_builder(&theme, page)
        }).collect::<Vec<Box<dyn ScreenBuilder>>>().into_iter().rev().for_each(|s| pages.push(s));

        if let Some(review) = form.review {
            let review = Box::new(move |_: &Theme| {
                let review = review.clone();
                PageType::review(&review.title, review.getter, form.on_submit.clone())
            }) as Box<dyn PageBuilder>;

            pages.push(Screen::new_builder(&theme, review));
        }

        if let Some(success) = form.success {
            let success = Box::new(move |_: &Theme| {
                let success = success.clone();
                PageType::success(&success.title, success.getter)
            }) as Box<dyn PageBuilder>;
            pages.push(Screen::new_builder(&theme, success));
        }

        // if let Some(r) = form.review() {pages.push(Screen::new_builder(builder, Review(r)));}
        // pages.push(Screen::new_builder(builder, Success(form.success())));
        Flow(pages)
    }


    
    pub(crate) fn build(&mut self, ctx: &mut Context) -> Box<dyn Callback> {
        let mut new: Vec<Box<dyn AppPage>> = vec![];
        let length = self.0.len();
        if self.0.is_empty() { return Box::new(|_ctx, _| {}); }

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
                ctx.send(Request::event(NavigationEvent::Next));
            }))));
        });

        let mut first = (first)(ctx);
        if !new.is_empty() { first.update(ctx, length, next_fn.clone()); }
        new.push(Box::new(first));
        new.reverse();

        Box::new(move |ctx: &mut Context, _: &Theme| {
            let flow = FlowWrapper::new(PelicanFlow::new(new.clone()));
            ctx.send(Request::event(NavigationEvent::push(flow))); // this needs to push the flow
        })
    }
}

#[derive(Debug, Clone)]
pub enum State {
    Text(String),
    Enumerator(usize),
    Number(String),
    Avatar(AvatarContent),
    Search(Vec<PelicanListItem>),
    ScanCode(Option<String>),
}

#[derive(Debug, Component, Clone)]
pub struct FlowWrapper(Stack, PelicanFlow, #[skip] Vec<State>);
impl OnEvent for FlowWrapper {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {        
        if event.downcast_ref::<TickEvent>().is_some() {
            if let Some(screen) = self.1.current.as_mut().unwrap().downcast_mut::<Screen>().as_mut() && let Some(page) = screen.1.downcast_mut::<ReviewPage>() {
                page.on_change(self.2.clone());
            } else if let Some(screen) = self.1.current.as_mut().unwrap().downcast_mut::<Screen>().as_mut() && let Some(page) = screen.1.downcast_mut::<SuccessPage>() {
                page.on_change(self.2.clone());
            } else {
                let index = self.1.index;
                self.2 = Vec::new();

                if self.1.stored.is_empty() && let Some(screen) = self.1.current.as_mut().unwrap().downcast_mut::<Screen>() && let Some(page) = screen.1.downcast_mut::<FormPage>() {
                    page.1.content.children().iter().for_each(|child| Input::store_in(child, &mut self.2));
                    page.on_change(self.2.clone());
                }

                for (i, each) in self.1.stored.iter_mut().enumerate() {
                    if i == index && let Some(screen) = self.1.current.as_mut().unwrap().downcast_mut::<Screen>() && let Some(page) = screen.1.downcast_mut::<FormPage>() {
                        page.1.content.children().iter().for_each(|child| Input::store_in(child, &mut self.2));
                        page.on_change(self.2.clone());
                    }

                    if let Some(screen) = each.downcast_mut::<Screen>() && let Some(page) = screen.1.downcast_mut::<FormPage>() {
                        page.1.content.children().iter().for_each(|child| Input::store_in(child, &mut self.2));
                        page.on_change(self.2.clone());
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

#[derive(Clone, Debug)]
pub enum NumberVariant {
    Currency,
    Date,
    Time,
}
