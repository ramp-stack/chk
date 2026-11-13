use prism::drawable::Component;
use prism::Context;
use ptsd::layout::Stack;
use ptsd::navigation::AppPage;

use crate::design_system::{PageView};
use crate::design_system::Theme;
use crate::form::FormValues;
use crate::items::Input;
use crate::closure::{FormSubmit, NavFn};
use crate::design_system::{Header, Footer, Content};

#[derive(Debug, Component, Clone)]
pub struct FormPage(
    Stack, pub PageView,
    #[skip] Theme,
    #[skip] Option<NavFn>,
    #[skip] Option<Box<dyn FormSubmit>>,
    #[skip] Option<FormValues>,
    #[skip] String,
    #[skip] crate::form::Submission,
);

impl AppPage for FormPage {}
impl FormPage {
    pub fn new(
        theme: &Theme, id: String, title: String, content: Content,
        next: Option<NavFn>, on_submit: Option<Box<dyn FormSubmit>>,
    ) -> Self {
        let footer = Footer::stack(None, Box::new(|_: &mut Context, _: &Theme| {}), None);
        let mut page = theme.page(Header::stack(&title, None), content, Some(footer));
        page.disable_actions(true);

        FormPage(Stack::default(), page, theme.clone(), next.clone(), on_submit.clone(), None, id, crate::form::Submission::default())
    }

    pub fn values(&self) -> FormValues {
        FormValues::new(self.1.content().children().iter().filter_map(|child| Input::state(&**child))
            .map(|state| (self.6.clone(), state)))
    }

    pub fn submitting(&self) -> bool {self.7.pending()}
    pub fn on_change(&mut self, new: FormValues) {
        self.5 = Some(new);
        let id = self.1.content().id();
        self.1.on_primary(Box::new(move |ctx: &mut Context| {
            ctx.emit(crate::form::SubmitRequest(id));
        }));
    }
}

#[prism::events::on_event]
impl prism::events::OnEvent<crate::form::SubmitRequest> for FormPage {
    fn on(&mut self, ctx: &mut Context, event: crate::form::SubmitRequest) -> Vec<Box<dyn prism::events::DynEvent>> {
        if event.0 != self.1.content().id() {return prism::events![event];}
        if self.7.pending() {return vec![];}
        let values = self.5.clone().unwrap_or_else(|| self.values());
        let result = self.4.as_mut().map(|submit| submit(ctx, &values)).unwrap_or(crate::FormComplete::None);
        self.7.start(event.0, result, ctx, &self.2, self.3.clone());
        if self.7.pending() {self.1.disable_actions(true);}
        vec![]
    }
}
#[prism::events::on_event]
impl prism::events::OnEvent<crate::form::SubmitFinished> for FormPage {
    fn on(&mut self, ctx: &mut Context, event: crate::form::SubmitFinished) -> Vec<Box<dyn prism::events::DynEvent>> {
        if event.0 != self.1.content().id() {return prism::events![event];}
        self.7.finish();
        event.1.then(self.3.clone()).run(ctx, &self.2);
        ctx.emit(crate::design_system::FormEvent::Refresh(event.0));
        vec![]
    }
}
#[prism::events::on_event]
impl prism::events::OnEvent<crate::design_system::FormEvent> for FormPage {
    fn on(&mut self, _: &mut Context, event: crate::design_system::FormEvent) -> Vec<Box<dyn prism::events::DynEvent>> {
        use crate::design_system::FormEvent;
        if self.7.pending() && let FormEvent::Validated {id, ..} = event && id == self.1.content().id() {
            return prism::events![FormEvent::Validated {id, valid: false}];
        }
        prism::events![event]
    }
}
