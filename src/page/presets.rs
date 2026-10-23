use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::Context;
use prism::hardware::Hardware;
use ptsd::layout::{Stack, Offset};
use prism::drawable::text::Align;
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::components::text::{ExpandableText, TextSize, TextStyle};
use pelican_ui::theme::Theme;

use crate::form::{State, FormComplete};
use crate::closure::{FormSubmit, NavFn, ReviewItemGetter, SuccessGetter};

mod messages;
pub use messages::{MessagesPage, MessageDetails};
mod profiles;
pub use profiles::{ProfilePage, ProfileView};

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
    #[skip] Option<NavFn>, #[skip] Box<dyn FormSubmit>, #[skip] Option<Vec<State>>,
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
        ReviewPage(Stack::default(), page, item_getter, theme.clone(), next.clone(), on_submit.clone(), None)
    }

    pub fn on_change(&mut self, hardware: &Hardware, new: Vec<State>) {
        if self.6.as_ref() == Some(&new) {return;}
        self.6 = Some(new.clone());
        let theme = self.3.clone();
        let items = (self.2)(&new);
        let content = items.into_iter().filter_map(|mut item| item.build(hardware, &theme))
            .flatten().collect();
        self.1.content = Content::new(
            Offset::Start, content, Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
        );

        let mut on_submit = self.5.clone();
        let next = self.4.clone();
        *self.1.bumper.as_mut().unwrap().on_click()[0] = Box::new(move |ctx: &mut Context| {
            let mut completion = (on_submit)(ctx, &new);
            match &mut completion {
                FormComplete::None => {
                    if let Some(next) = &next {(next.borrow_mut())(ctx, &theme);}
                },
                _ => completion.run(ctx, &theme),
            }
        });
    }

}

#[derive(Debug, Component, Clone)]
pub struct SuccessPage(
    Stack, pub PelicanPage, #[skip] Box<dyn SuccessGetter>, #[skip] Theme,
    #[skip] Option<Box<dyn FormSubmit>>, #[skip] Option<Vec<State>>,
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

    pub fn on_change(&mut self, ctx: &mut Context, new: Vec<State>) {
        use pelican_ui::colors;
        use pelican_ui::components::Icon;
        if self.5.as_ref() == Some(&new) {return;}
        self.5 = Some(new.clone());
        if let Some(mut on_submit) = self.4.take() {(on_submit)(ctx, &new).run(ctx, &self.3);}
        let theme = self.3.clone();
        let (icon, description) = (self.2)(new);
        self.1.content = Content::new(Offset::Center, vec![
            Box::new(Icon::new(&theme, icon, Some(theme.colors().get(colors::Text::Heading)), 128.0)),
            Box::new(ExpandableText::new(&theme, &description, TextSize::H4, TextStyle::Heading, Align::Center, None))
        ], Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true));
    }
}
