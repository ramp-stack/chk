use futures::{StreamExt, stream::FuturesUnordered};
use prism::events::{DynEvent, OnEvent, on_event};
use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::{Context, Callback};
use ptsd::layout::{Stack, Offset};
use ptsd::Validate;
use pelican_ui::interface::general::{
    Header, Content, FormEvent, Bumper as PelicanBumper, Page as PelicanPage,
};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::Theme;
use pelican_ui::utils::ValidationFn;

use std::ops::Range;
use std::future::Future;
use std::pin::Pin;

use crate::{FormItem, State, FlowBuilder, Icons};
use crate::items::{Input, Display};
use crate::closure::FormSubmit;

#[derive(Debug, Component, Clone)]
pub struct EditPage(
    Stack, pub PelicanPage,
    #[skip] Box<dyn FormSubmit>,
    #[skip] Option<Vec<State>>,
    #[skip] bool,
);
#[on_event]
impl OnEvent<FormEvent> for EditPage {
    fn on(&mut self, _ctx: &mut Context, event: FormEvent) -> Vec<Box<dyn DynEvent>> {
        if self.4
            && let FormEvent::Changed(id) = event
            && id == self.1.content.id() {
            let new = self
                .1
                .content
                .children()
                .iter()
                .filter_map(|child| Input::state(&**child))
                .collect();
            self.on_change(new);
        }

        prism::events![event]
    }
}
impl AppPage for EditPage {}
impl EditPage {
    pub fn new(
        theme: &Theme, is_mobile: bool, title: String, input: Vec<Input>, display: Vec<Display>,
        validations: Vec<Box<dyn ValidationFn>>, on_save: Box<dyn FormSubmit>,
    ) -> Self {
        let header = Header::stack(theme, &title, None);
        let content = Self::content(theme, is_mobile, input, display, validations);

        let bumper = PelicanBumper::stack(
            theme, Some("Save"),
            Box::new(|_: &mut Context, _: &Theme| {}),
            None,
        );
        let page = PelicanPage::new(header, content, Some(bumper));

        EditPage(Stack::default(), page, on_save.clone(), None, false)
    }

    pub fn edit_and_display(
        theme: &Theme, is_mobile: bool, title: String,
        mut header: Option<(Icons, Box<dyn FlowBuilder>)>, items: Vec<FormItem>,
        display: Vec<Display>, on_save: Box<dyn FormSubmit>,
    ) -> Self {
        let header = header.as_mut().map(|(icon, flow)| {
            let mut flow = flow.clone();
            (
                *icon,
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    ((flow)(ctx, theme).build(ctx, theme))(ctx, theme)
                }) as Box<dyn Callback>,
            )
        });

        let header = Header::stack(theme, &title, header);
        let validations = items.iter().map(|i| i.validation()).collect::<Vec<_>>();
        let inputs = items.into_iter().map(|i| i.build()).collect::<Vec<Input>>();
        let content = Self::content(theme, is_mobile, inputs, display, validations);

        let bumper = PelicanBumper::stack(
            theme, Some("Save"),
            Box::new(|_: &mut Context, _: &Theme| {}),
            None,
        );
        let page = PelicanPage::new(header, content, Some(bumper));

        EditPage(Stack::default(), page, on_save.clone(), None, false)
    }

    pub fn root(
        theme: &Theme, is_mobile: bool, title: String,
        mut header: Option<(Icons, Box<dyn FlowBuilder>)>, items: Vec<FormItem>,
        display: Vec<Display>, on_save: Box<dyn FormSubmit>,
    ) -> Self {
        let header = header.as_mut().map(|(icon, flow)| {
            let mut flow = flow.clone();
            (
                *icon,
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    ((flow)(ctx, theme).build(ctx, theme))(ctx, theme)
                }) as Box<dyn Callback>,
            )
        });

        let header = Header::home(theme, &title, header);
        let validations = items.iter().map(|i| i.validation()).collect::<Vec<_>>();
        let inputs = items.into_iter().map(|i| i.build()).collect::<Vec<Input>>();
        let content = Self::content(theme, is_mobile, inputs, display, validations);

        let bumper = PelicanBumper::home(
            theme,
            Some(("Save".to_string(), Box::new(|_: &mut Context, _: &Theme| {}))),
            None,
        );
        let page = PelicanPage::new(header, content, Some(bumper));

        EditPage(Stack::default(), page, on_save.clone(), None, true)
    }

    fn content(theme: &Theme, is_mobile: bool, inputs: Vec<Input>, display: Vec<Display>, validations: Vec<Box<dyn ValidationFn>>) -> Content {
        let mut children = Vec::new();
        let mut validators = Vec::new();
        let mut validations = validations.into_iter();
        for input in inputs {
            let start = children.len();
            children.extend(input.build(theme, is_mobile).unwrap_or_default());
            if let Some(validation) = validations.next() {
                validators.push((start..children.len(), validation));
            }
        }
        for mut item in display { children.extend(item.build(theme).unwrap_or_default()); }
        Content::new(Offset::Start, children, Box::new(FormValidation(validators)))
    }

    pub fn on_change(&mut self, new: Vec<State>) {
        if self.3.as_ref() == Some(&new) { return; }
        self.3 = Some(new.clone());
        let mut on_save = self.2.clone();
        *self.1.bumper.as_mut().unwrap().on_click()[0] = Box::new(move |ctx: &mut Context| {
            (on_save)(ctx, &new);
        });
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
            let mut waits: FuturesUnordered<_> =
                self.0.iter_mut().map(|(_, validation)| validation.change()).collect();
            while let Some(update) = waits.next().await { if update.is_some() { return update; } }
            None
        })
    }
}
