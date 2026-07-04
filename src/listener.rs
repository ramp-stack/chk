use pelican_ui::theme::Theme;
use pelican_ui::drawable::{Component, SizedTree};
use pelican_ui::layout::Stack;
use pelican_ui::event::OnEvent;
use ramp::prism;
use pelican_ui::event::{Event, TickEvent};
use pelican_ui::navigation::AppPage;
use pelican_ui::Context;

use crate::{PageTypeToAppPageFn, PageBuilderContractFn, PageType};

use air::{Contract, Instance};
use std::cmp::PartialEq;

#[derive(Debug, Component, Clone)]
pub struct Listener<C: Contract + PartialEq> {
    layout: Stack,
    page: Box<dyn AppPage>,
    #[skip] instance: Instance<C>,
    #[skip] contract: C,
    #[skip] builder: Box<dyn PageTypeToAppPageFn>,
    #[skip] page_type: Box<dyn PageBuilderContractFn<C>>,
    #[skip] theme: Theme,
}

impl<C: Contract + PartialEq> AppPage for Listener<C> {}

impl<C: Contract + PartialEq> OnEvent for Listener<C> {
    fn on_event(&mut self, ctx: &mut Context, sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if event.downcast_ref::<TickEvent>().is_some() {
            let new_contract = self.profile.pending().clone();
            if self.contract != new_contract {
                self.contract = new_contract;
                let page_type = (self.page_type)(ctx, self.contract.clone());
                self.page = (self.builder)(ctx, &self.theme, page_type);
            }
        }

        vec![event]
    }
}

impl<C: Contract + PartialEq> Listener<C> {
    pub fn new(
        ctx: &mut Context, 
        theme: &Theme, 
        instance: Instance<C>, 
        contract: C, 
        mut page_type: impl PageBuilderContractFn<C> + Clone + 'static, 
        mut builder: impl PageTypeToAppPageFn + Clone + 'static, 
    ) -> Self {
        let pt = page_type(ctx, contract.clone());
        let page = builder(ctx, theme, pt);
        Listener {
            layout: Stack::default(),
            page,
            instance,
            contract,
            builder: Box::new(builder),
            page_type: Box::new(page_type),
            theme: theme.clone()
        }
    }
}
