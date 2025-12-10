use pelican_ui::{State, Context};
use pelican_ui::utils::Callback;
use pelican_ui::components::interface::navigation::NavigationEvent;

use std::cell::RefCell;
use std::rc::Rc;

use crate::{NavFn};
use crate::pages::PageType;
use crate::FnMutClone;

#[derive(Debug, Clone, Default)]
pub struct Flow { 
    pages: Vec<Box<dyn PageBuilder>>,
    on_submit: Option<Box<dyn FnMutClone>>
}

impl Flow {
    pub fn new(pages: Vec<Box<dyn PageBuilder>>) -> Self {
        Flow { pages, on_submit: None }
    }

    pub fn new_form(mut inputs: Vec<Box<dyn PageBuilder>>, review: Option<Box<dyn PageBuilder>>, success: Box<dyn PageBuilder>, on_submit: impl FnMut(&mut Context) + Clone + 'static) -> Self {
        if let Some(r) = review { inputs.push(r); }
        inputs.push(success);
        Flow { pages: inputs, on_submit: Some(Box::new(on_submit))}
    }

    pub(crate) fn build(&mut self) -> Callback {
        if self.pages.is_empty() { return Box::new(|_ctx| {}); }

        let mut pages = self.pages.clone();
        let mut first = pages.remove(0);
        let mut next_fn: Option<NavFn> = None;

        for (i, page) in pages.into_iter().rev().enumerate() {
            let callback = (i == 0).then_some(self.on_submit.clone()).flatten(); 
            let next = next_fn.take();
            let mut page = page;
            next_fn = Some(Rc::new(RefCell::new(move |ctx: &mut Context| {
                if let Some(cb) = callback.clone() { (cb.clone())(ctx) }
                let page_box = (page)(ctx.state()).build(ctx, next.clone());
                ctx.trigger_event(NavigationEvent::Push(Some(Box::new(page_box))));
            })));
        }

        Box::new(move |ctx: &mut Context| {
            let page_box = first(ctx.state()).build(ctx, next_fn.clone());
            ctx.trigger_event(NavigationEvent::Push(Some(Box::new(page_box))));
        })
    }
}

pub trait PageBuilder: FnMut(&mut State) -> PageType + 'static {
    fn clone_box(&self) -> Box<dyn PageBuilder>;
}

impl<F> PageBuilder for F where F: FnMut(&mut State) -> PageType + Clone + 'static {
    fn clone_box(&self) -> Box<dyn PageBuilder> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn PageBuilder> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn PageBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Page Builder...")
    }
}
