use pelican_ui::{Callback, Context, Request};
use pelican_ui::navigation::{NavigationEvent, Flow as PelicanFlow, FlowContainer, AppPage};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::drawable::{Drawable, Component, SizedTree};
use pelican_ui::layout::Stack;
use pelican_ui::event::OnEvent;
use ramp::prism;
use pelican_ui::event::{Event, TickEvent};
use pelican_ui::components::avatar::{AvatarContent, AvatarIconStyle};
use pelican_ui::components::list_item::ListItem as PelicanListItem;
use pelican_ui::components::SearchBar;
use pelican_ui::utils::ValidationFn;

use crate::items::{EnumItem, Input, ListItem, Action, Display};
use crate::page::{EditPage, PageType, FormPage, ReviewPage, SuccessPage};
use crate::closure::{FormSubmit, FormClosure, NavFn, ScreenBuilder, PageBuilder, ReviewItemGetter, SuccessGetter, ValidityFn};
use crate::page::Screen;

use air::names::Name;

use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub enum Form {
    Flow {
        theme: Theme,
        inputs: Vec<FormItem>,
        review: Option<Review>,
        success: Option<Success>,
        on_submit: Box<dyn FormSubmit>, 
    }, 
    Page {
        theme: Theme,
        title: String,
        inputs: Vec<FormItem>,
        display: Vec<Display>,
        on_save: Box<dyn FormSubmit>, 
    }
}

impl Form {
    pub fn flow(theme: &Theme, inputs: Vec<FormItem>, review: Option<Review>, success: Option<Success>, on_submit: Box<dyn FormSubmit>) -> Self {
        Form::Flow {
            inputs,
            theme: theme.clone(),
            review,
            success,
            on_submit,
        }
    }

    pub fn page(theme: &Theme, title: &str, inputs: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        Form::Page {
            title: title.to_string(),
            inputs,
            display,
            on_save,
            theme: theme.clone(),
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
    TextWithPreset(String, String, Box<dyn FormClosure>, Option<Vec<(String, Icons, Action)>>, Box<dyn ValidityFn>),
    Number(String, NumberVariant, Box<dyn ValidityFn>),
    Enum(String, Vec<EnumItem>),
    Search(String, Vec<(ListItem, Name)>),
    ScanQR(String, String, Option<(String, Icons, Action)>),
    Avatar(String),
    AvatarWithPreset(String, AvatarContent),
}

pub struct FormStorage(pub HashMap<String, String>);

impl FormItem {
    pub fn text(text: &str, actions: Option<Vec<(String, Icons, Action)>>, valid: impl ValidityFn + 'static) -> Self {
        let text = text.to_string();
        FormItem::Text(text.to_string(), Box::new(move |storage: &mut FormStorage, value: String| {storage.0.insert(text.to_string(), value);}), actions, Box::new(valid))
    }

    pub fn text_with_preset(text: &str, preset: &str, actions: Option<Vec<(String, Icons, Action)>>, valid: impl ValidityFn + 'static) -> Self {
        let text = text.to_string();
        FormItem::TextWithPreset(text.to_string(), preset.to_string(), Box::new(move |storage: &mut FormStorage, value: String| {storage.0.insert(text.to_string(), value);}), actions, Box::new(valid))
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

    pub fn search(title: &str, items: Vec<(ListItem, Name)>) -> Self {
        FormItem::Search(title.to_string(), items)
    }

    pub fn avatar(title: &str) -> Self {
        FormItem::Avatar(title.to_string())
    }

    pub fn avatar_with_preset(title: &str, avatar: AvatarContent) -> Self {
        FormItem::AvatarWithPreset(title.to_string(), avatar)
    }

    pub fn scan_qr_code(title: &str, instructions: &str, alt: Option<(String, Icons, Action)>) -> Self {
        FormItem::ScanQR(title.to_string(), instructions.to_string(), alt)
    }
}

impl FormItem {
    fn title(&self) -> String {
        match self {
            FormItem::Search(title, ..) |
            FormItem::Text(title, ..) |
            FormItem::TextWithPreset(title, ..) |
            FormItem::Number(title, ..) |
            FormItem::Avatar(title, ..) |
            FormItem::AvatarWithPreset(title, ..) |
            FormItem::ScanQR(title, ..) |
            FormItem::Enum(title, ..) => title.to_string()
        }
    }

    pub fn validation(&self) -> Box<dyn ValidationFn> {
        match self {
            FormItem::Text(_, _, _, validation) | FormItem::TextWithPreset(_, _, _, _, validation) => {
                use pelican_ui::components::TextInput;

                let validation = validation.clone();
                Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
                    if let Some(input) = children[0].as_any_mut().downcast_mut::<TextInput>() {
                        let result = (validation.clone())(ctx, input.value());
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
                Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
                    if let Some(input) = children[0].as_any_mut().downcast_mut::<NumericalInput>() {
                        let result = (validation.clone())(ctx, input.value());
                        input.error(result.clone());
                        result.is_ok()
                    } else {
                        true
                    }
                })
            },
            FormItem::Search(_, _) => Box::new(|ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
                if let Some(searchbar) = children[0].as_any_mut().downcast_mut::<SearchBar>() {!searchbar.results().is_empty()} else {true}
            }),
            _ => Box::new(|ctx: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        }
    }

    pub fn build(&self) -> Input {
        match self {
            FormItem::Text(title, _, actions, _) => {
                let title = title.clone();
                Input::text(&title, false, None, actions.clone())
            },
            FormItem::TextWithPreset(title, preset, _, actions, _) => {
                let title = title.clone();
                Input::text(&title, true, Some(preset.to_string()), actions.clone())
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
            FormItem::ScanQR(_, instructions, alt) => Input::qr_code_scanner(instructions, alt.clone()),
            FormItem::Avatar(_) => Input::avatar(AvatarContent::default(), Some((Icons::Edit, AvatarIconStyle::Secondary)), Some(Action::None)),
            FormItem::AvatarWithPreset(_, avatar) => Input::avatar(avatar.clone(), Some((Icons::Edit, AvatarIconStyle::Secondary)), Some(Action::None))
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Flow(Vec<Box<dyn ScreenBuilder>>);
impl Flow{
    pub fn new(theme: &Theme, pages: Vec<Box<dyn PageBuilder>>) -> Self {
        let pages = pages.into_iter().map(|p| Screen::new_builder(theme, p)).collect::<Vec<_>>();
        Flow(pages)
    }

    pub fn from_form(form: Form) -> Self {
        match form {
            Form::Flow {theme, inputs, on_submit, review, success} => {
                let theme = theme.clone();
                let mut pages: Vec<Box<dyn ScreenBuilder>> = vec![];

                let t = theme.clone();
                let mut submit = review.is_none().then(|| on_submit.clone());
                let mut submit = submit.map(|mut os| Box::new(move |ctx: &mut Context, state: &Vec<State>| {
                    let theme = t.clone();
                    let out = (os)(ctx, &state);
                    if let Some(mut f) = out {
                        println!("OUT");
                        let flow = FlowWrapper::new(PelicanFlow::new(vec![(f()).build(ctx, &theme)]));
                        ctx.emit(NavigationEvent::restart(flow));
                    }
                    None
                }) as Box<dyn FormSubmit>);

                inputs.into_iter().rev().map(|input| {
                    let mut submit = submit.take();
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
                        let success = success.clone();
                        PageType::success(&success.title, success.getter)
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
                ctx.emit(NavigationEvent::Next);
            }))));
        });

        let mut first = (first)(ctx);
        if !new.is_empty() { first.update(ctx, length, next_fn.clone()); }
        new.push(Box::new(first));
        new.reverse();

        Box::new(move |ctx: &mut Context, _: &Theme| {
            let flow = FlowWrapper::new(PelicanFlow::new(new.clone()));
            ctx.emit(NavigationEvent::push(flow));
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Text(String),
    Enumerator(usize),
    Number(String),
    Avatar(AvatarContent),
    Search(Vec<Name>),
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

#[derive(Clone, Debug)]
pub enum NumberVariant {
    Currency,
    Date,
    Time,
}
