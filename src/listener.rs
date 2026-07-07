use pelican_ui::theme::Theme;
use pelican_ui::drawable::{Component, SizedTree};
use pelican_ui::layout::Stack;
use pelican_ui::event::OnEvent;
use ramp::prism;
use pelican_ui::event::{Event, TickEvent};
use pelican_ui::navigation::AppPage;
use pelican_ui::Context;

use crate::{PageBuilder, NavFn, PageTypeToAppPageFn, PageBuilderContractFn, PageType};

use air::{Contract, Instance};
use std::cmp::PartialEq;

#[derive(Debug, Component, Clone)]
pub struct Listener {
    layout: Stack,
    pub page: Box<dyn AppPage>,
    #[skip] builder: Box<dyn PageBuilder>,
    #[skip] theme: Theme,
    #[skip] is_root: bool,
    #[skip] next: Option<NavFn>,
    #[skip] flow_len: usize,
}

impl AppPage for Listener {}

impl Listener {
    pub fn new(
        ctx: &mut Context,
        theme: &Theme,
        mut builder: Box<dyn PageBuilder>,
        is_root: bool,
    ) -> Self {
        let page_type = builder.build(ctx, theme);

        let page = if is_root {
            page_type.build_root(ctx, theme)
        } else {
            page_type.build(ctx, theme)
        };

        Self {
            layout: Stack::default(),
            page,
            builder,
            theme: theme.clone(),
            is_root,
            next: None,
            flow_len: 0,
        }
    }

    pub fn update(
        &mut self,
        ctx: &mut Context,
        next: Option<NavFn>,
        length: usize,
    ) {
        let mut page_type = self.builder.build(ctx, &self.theme);
        page_type.update(ctx, &self.theme, length, next.clone());

        self.page = if self.is_root {
            page_type.build_root(ctx, &self.theme)
        } else {
            page_type.build(ctx, &self.theme)
        };

        self.next = next;
        self.flow_len = length;
    }
}

impl OnEvent for Listener {
    fn on_event(
        &mut self,
        ctx: &mut Context,
        _sized: &SizedTree,
        event: Box<dyn Event>,
    ) -> Vec<Box<dyn Event>> {
        if event.downcast_ref::<TickEvent>().is_some() {
            if self.builder.poll(ctx) {
                self.update(ctx, self.next.clone(), self.flow_len);
            }
        }

        vec![event]
    }
}