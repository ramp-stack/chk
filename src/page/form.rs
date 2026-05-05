use ramp::prism;

use pelican_ui::event::OnEvent;
use pelican_ui::drawable::Component;
use pelican_ui::{Context, Callback};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::Theme;
use pelican_ui::utils::ValidationFn;

use crate::form::State;
use crate::items::Input;
use crate::closure::{FormSubmit, NavFn};

#[derive(Debug, Component, Clone)]
pub struct FormPage(Stack, pub PelicanPage, #[skip] Theme, #[skip] Option<NavFn>, #[skip] Option<Box<dyn FormSubmit>>, #[skip] Vec<State>);
impl OnEvent for FormPage {}
impl AppPage for FormPage {}
impl FormPage {
    pub fn new(theme: &Theme, title: String, item: Input, next: Option<NavFn>, _flow_len: usize, validate: Box<dyn ValidationFn>, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        let header = Header::stack(theme, &title, None);
        let content = item.build(theme).unwrap_or_default();
        let bumper = PelicanBumper::stack(theme, None, Box::new(|_: &mut Context, _: &Theme| {}), None);
        let page = PelicanPage::new(header, Content::new(Offset::Start, content, validate), Some(bumper));

        FormPage(Stack::default(), page, theme.clone(), next.clone(), on_submit.clone(), vec![])
    }

    pub fn on_change(&mut self, new: Vec<State>) {
        if new != self.5 {
            self.5 = new.clone();
            let theme = &self.2;
            let submit = self.4.clone();
            let closure: Box<dyn Callback> = match self.3.clone(){
                Some(nav) => Box::new(move |ctx: &mut Context, theme: &Theme| {
                    if let Some(mut on_submit) = submit.clone() {(on_submit)(ctx, &new);}
                    (nav.borrow_mut())(ctx, theme);
                }),
                None => Box::new(move |ctx: &mut Context, _theme: &Theme| {
                    if let Some(mut on_submit) = submit.clone() {(on_submit)(ctx, &new);}
                }),
            };
            self.1.bumper = Some(PelicanBumper::stack(theme, None, closure, None));
        }
    }
}
