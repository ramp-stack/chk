use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::Context;
use ptsd::layout::{Stack, Offset};
use prism::drawable::text::Align;
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::components::text::{Text, TextSize, TextStyle};
use pelican_ui::theme::Theme;

use crate::form::FormValues;
use crate::closure::{FormSubmit, NavFn, ReviewItemGetter, SuccessGetter};

#[derive(Debug, Clone)]
pub struct Success {
    pub title: String,
    pub getter: Box<dyn SuccessGetter>,
}

impl Success {
    pub fn new(title: &str, getter: impl SuccessGetter + 'static) -> Self {
        Success{title: title.to_string(), getter: Box::new(getter)}
    }
}

#[derive(Debug, Clone)]
pub struct Review {
    pub title: String,
    pub getter: Box<dyn ReviewItemGetter>,
}

impl Review {
    pub fn new(title: &str, getter: impl ReviewItemGetter + 'static) -> Self {
        Review{title: title.to_string(), getter: Box::new(getter)}
    }
}

#[derive(Debug, Component, Clone)]
pub struct ReviewPage(
    Stack, pub PelicanPage, #[skip] Box<dyn ReviewItemGetter>, #[skip] Theme,
    #[skip] Option<NavFn>, #[skip] Box<dyn FormSubmit>, #[skip] Option<FormValues>,
    #[skip] crate::form::Submission,
);

impl AppPage for ReviewPage {}
impl ReviewPage {
    pub fn new(
        theme: &Theme, title: String, item_getter: Box<dyn ReviewItemGetter>, next: Option<NavFn>,
        _flow_len: usize, on_submit: Box<dyn FormSubmit>,
    ) -> Self {
        let header = Header::stack(theme, &title, None);
        let bumper = PelicanBumper::stack(theme, None, Box::new(|_ctx: &mut Context, _theme: &Theme| {}), None);
        let page = PelicanPage::new(header, Content::new(
            Offset::Start, Vec::new(), Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        ), Some(bumper));
        ReviewPage(Stack::default(), page, item_getter, theme.clone(), next.clone(), on_submit.clone(), None, crate::form::Submission::default())
    }

    pub fn on_change(&mut self, new: FormValues) {
        if self.6.as_ref() == Some(&new) {return;}
        self.6 = Some(new.clone());
        let theme = self.3.clone();
        let items = (self.2)(&new);
        let content = items.into_iter().filter_map(|mut item| item.build(&theme))
            .flatten().collect();
        self.1.content = Content::new(
            Offset::Start, content, Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        );

        let id = self.1.content.id();
        *self.1.bumper.as_mut().unwrap().on_click()[0] = Box::new(move |ctx: &mut Context| {
            ctx.emit(crate::form::SubmitRequest(id));
        });
    }
    pub(crate) fn submitting(&self) -> bool {self.7.pending}

}

#[derive(Debug, Component, Clone)]
pub struct SuccessPage(
    Stack, pub PelicanPage, #[skip] Box<dyn SuccessGetter>, #[skip] Theme,
    #[skip] Option<Box<dyn FormSubmit>>, #[skip] Option<FormValues>,
);

impl AppPage for SuccessPage {}
impl SuccessPage {
    pub fn new(theme: &Theme, title: String, getter: Box<dyn SuccessGetter>, flow_len: usize, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        let header = Header::stack_end(theme, &title);
        let bumper = Some(PelicanBumper::stack_end(theme, Some(flow_len)));
        let page = PelicanPage::new(header, Content::new(
            Offset::Center, vec![], Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        ), bumper);
        SuccessPage(Stack::default(), page, getter, theme.clone(), on_submit.clone(), None)
    }

    pub fn on_change(&mut self, ctx: &mut Context, new: FormValues) {
        use pelican_ui::colors;
        use pelican_ui::components::Icon;
        if self.5.as_ref() == Some(&new) {return;}
        self.5 = Some(new.clone());
        if let Some(mut on_submit) = self.4.take() {(on_submit)(ctx, &new).run(ctx, &self.3);}
        let theme = self.3.clone();
        let (icon, description) = (self.2)(new);
        self.1.content = Content::new(Offset::Center, vec![
            Box::new(Icon::new(&theme, icon, Some(theme.colors().get(colors::Text::Heading)), 128.0)),
            Box::new(Text::new(
                &theme, &description, TextSize::H4, TextStyle::Heading, Align::Center, None,
            ))
        ], Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true));
    }
}

#[prism::events::on_event]
impl prism::events::OnEvent<crate::form::SubmitRequest> for ReviewPage {
    fn on(&mut self, ctx: &mut Context, event: crate::form::SubmitRequest) -> Vec<Box<dyn prism::events::DynEvent>> {
        if event.0 != self.1.content.id() {return prism::events![event];}
        if self.7.pending {return vec![];}
        let Some(values) = self.6.as_ref() else {return vec![];};
        let result = (self.5)(ctx, values);
        self.7.start(event.0, result, ctx, &self.3, self.4.clone());
        if self.7.pending {self.1.bumper.as_mut().unwrap().disable(true);}
        vec![]
    }
}
#[prism::events::on_event]
impl prism::events::OnEvent<crate::form::SubmitFinished> for ReviewPage {
    fn on(&mut self, ctx: &mut Context, event: crate::form::SubmitFinished) -> Vec<Box<dyn prism::events::DynEvent>> {
        if event.0 != self.1.content.id() {return prism::events![event];}
        self.7.pending = false;
        self.1.bumper.as_mut().unwrap().disable(false);
        event.1.then(self.4.clone()).run(ctx, &self.3);
        vec![]
    }
}
