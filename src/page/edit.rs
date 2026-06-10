use ramp::prism;

use pelican_ui::event::{OnEvent, Event, TickEvent};
use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::Context;
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::Theme;
use pelican_ui::utils::ValidationFn;
use pelican_ui::drawable::SizedTree;

use crate::{FormItem, State};
use crate::items::{Input, Display};
use crate::closure::FormSubmit;

#[derive(Debug, Component, Clone)]
pub struct EditPage(Stack, pub PelicanPage, #[skip] Theme, #[skip] Box<dyn FormSubmit>, #[skip] Vec<State>, #[skip] bool);
impl OnEvent for EditPage {
    fn on_event(&mut self, ctx: &mut Context, sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if event.downcast_ref::<TickEvent>().is_some() && self.5 {
            let mut new = vec![];
            self.1.content.children().iter().for_each(|child| Input::store_in(child, &mut new));
            self.on_change(new);
        }

        vec![event]
    }
}
impl AppPage for EditPage {}
impl EditPage {
    pub fn new(theme: &Theme, title: String, input: Vec<Input>, display: Vec<Display>, validations: Vec<Box<dyn ValidationFn>>, on_save: Box<dyn FormSubmit>) -> Self {
        let header = Header::stack(theme, &title, None);
        let mut content = input.into_iter().flat_map(|i| i.build(theme)).flatten().collect::<Vec<_>>();
        display.into_iter().for_each(|mut d| if let Some(r) = d.build(theme) {content.extend(r)});

        let validation = Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
            validations.clone().into_iter().enumerate().any(|(i, mut validation)| {
                let child = vec![&mut *children[i]];
                !(validation)(ctx, child)
            })
        }) as Box<dyn ValidationFn>;
        
        let bumper = PelicanBumper::stack(theme, Some("Save"), Box::new(|_: &mut Context, _: &Theme| {}), None);
        let page = PelicanPage::new(header, Content::new(Offset::Start, content, validation), Some(bumper));

        EditPage(Stack::default(), page, theme.clone(), on_save.clone(), vec![], false)
    }

    pub fn edit_and_display(theme: &Theme, title: String, items: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        let header = Header::stack(theme, &title, None);
        let validations = items.iter().map(|i| i.validation()).collect::<Vec<_>>();
        let inputs = items.into_iter().map(|i| i.build()).collect::<Vec<Input>>();
        let mut content = inputs.into_iter().flat_map(|i| i.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        display.into_iter().for_each(|mut d| if let Some(r) = d.build(theme) {content.extend(r)});

        let validation = Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
            validations.clone().into_iter().enumerate().any(|(i, mut validation)| {
                let child = vec![&mut *children[i]];
                !(validation)(ctx, child)
            })
        }) as Box<dyn ValidationFn>;
        
        let bumper = PelicanBumper::stack(theme, Some("Save"), Box::new(|_: &mut Context, _: &Theme| {}), None);
        let page = PelicanPage::new(header, Content::new(Offset::Start, content, validation), Some(bumper));

        EditPage(Stack::default(), page, theme.clone(), on_save.clone(), vec![], false)
    }

    pub fn root(theme: &Theme, title: String, items: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        let header = Header::home(theme, &title, None);
        let validations = items.iter().map(|i| i.validation()).collect::<Vec<_>>();
        let inputs = items.into_iter().map(|i| i.build()).collect::<Vec<Input>>();
        let mut content = inputs.into_iter().flat_map(|i| i.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        display.into_iter().for_each(|mut d| if let Some(r) = d.build(theme) {content.extend(r)});

        let validation = Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
            validations.clone().into_iter().enumerate().any(|(i, mut validation)| {
                let child = vec![&mut *children[i]];
                !(validation)(ctx, child)
            })
        }) as Box<dyn ValidationFn>;
        
        let bumper = PelicanBumper::home(theme, Some(("Save".to_string(), Box::new(|_: &mut Context, _: &Theme| {}))), None);
        let page = PelicanPage::new(header, Content::new(Offset::Start, content, validation), Some(bumper));

        EditPage(Stack::default(), page, theme.clone(), on_save.clone(), vec![], true)
    }

    pub fn on_change(&mut self, new: Vec<State>) {
        if new != self.4 {
            self.4 = new.clone();
            let theme = &self.2;
            let mut on_save = self.3.clone();
            let closure = Box::new(move |ctx: &mut Context, _theme: &Theme| {(on_save)(ctx, &new);});
            self.1.bumper = Some(PelicanBumper::stack(theme, Some("Save"), closure, None));
        }
    }
}
