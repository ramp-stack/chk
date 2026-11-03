use pelican_ui::drawable::Component;
use pelican_ui::Context;
use ptsd::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::Theme;
use pelican_ui::utils::ValidationFn;

use crate::form::FormValues;
use crate::items::Input;
use crate::closure::{FormSubmit, NavFn};

#[derive(Debug, Component, Clone)]
pub struct FormPage(
    Stack, pub PelicanPage,
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
        theme: &Theme, is_mobile: bool, id: String, title: String, item: Input,
        next: Option<NavFn>, validate: Box<dyn ValidationFn>,
        on_submit: Option<Box<dyn FormSubmit>>,
    ) -> Self {
        let header = Header::stack(theme, &title, None);
        let content = item.build(theme, is_mobile).unwrap_or_default();
        let mut bumper =
            PelicanBumper::stack(theme, None, Box::new(|_: &mut Context, _: &Theme| {}), None);
        // Validation runs asynchronously. Do not allow an unvalidated form to
        // advance during the first frame (including an empty selector).
        bumper.disable(true);
        let page =
            PelicanPage::new(header, Content::new(Offset::Start, content, validate), Some(bumper));

        FormPage(Stack::default(), page, theme.clone(), next.clone(), on_submit.clone(), None, id, crate::form::Submission::default())
    }

    pub(crate) fn values(&self) -> FormValues {
        FormValues::new(self.1.content.children().iter().filter_map(|child| Input::state(&**child))
            .map(|state| (self.6.clone(), state)))
    }

    pub(crate) fn submitting(&self) -> bool {self.7.pending}
    pub fn on_change(&mut self, new: FormValues) {
        self.5 = Some(new);
        let id = self.1.content.id();
        *self.1.bumper.as_mut().unwrap().on_click()[0] = Box::new(move |ctx: &mut Context| {
            ctx.emit(crate::form::SubmitRequest(id));
        });
    }
}

#[prism::events::on_event]
impl prism::events::OnEvent<crate::form::SubmitRequest> for FormPage {
    fn on(&mut self, ctx: &mut Context, event: crate::form::SubmitRequest) -> Vec<Box<dyn prism::events::DynEvent>> {
        if event.0 != self.1.content.id() {return prism::events![event];}
        if self.7.pending {return vec![];}
        let values = self.5.clone().unwrap_or_else(|| self.values());
        let result = self.4.as_mut().map(|submit| submit(ctx, &values)).unwrap_or(crate::FormComplete::None);
        self.7.start(event.0, result, ctx, &self.2, self.3.clone());
        if self.7.pending {self.1.bumper.as_mut().unwrap().disable(true);}
        vec![]
    }
}
#[prism::events::on_event]
impl prism::events::OnEvent<crate::form::SubmitFinished> for FormPage {
    fn on(&mut self, ctx: &mut Context, event: crate::form::SubmitFinished) -> Vec<Box<dyn prism::events::DynEvent>> {
        if event.0 != self.1.content.id() {return prism::events![event];}
        self.7.pending = false;
        event.1.then(self.3.clone()).run(ctx, &self.2);
        ctx.emit(pelican_ui::interface::general::FormEvent::Refresh(event.0));
        vec![]
    }
}
#[prism::events::on_event]
impl prism::events::OnEvent<pelican_ui::interface::general::FormEvent> for FormPage {
    fn on(&mut self, _: &mut Context, event: pelican_ui::interface::general::FormEvent) -> Vec<Box<dyn prism::events::DynEvent>> {
        use pelican_ui::interface::general::FormEvent;
        if self.7.pending && let FormEvent::Validated {id, ..} = event && id == self.1.content.id() {
            return prism::events![FormEvent::Validated {id, valid: false}];
        }
        prism::events![event]
    }
}
