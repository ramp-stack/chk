use pelican_ui::theme::Theme;
use prism::drawable::Component;
use ptsd::layout::Stack;
use prism::events::{on_event, OnEvent};
use prism::events::{DynEvent, TickEvent};
use pelican_ui::navigation::AppPage;
use pelican_ui::Context;

use crate::{MessagesPage, PageBuilder, NavFn};

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
    pub fn new(ctx: &mut Context, theme: &Theme, mut builder: Box<dyn PageBuilder>, is_root: bool) -> Self {
        let page_type = builder.build(ctx, theme);

        Self {
            layout: Stack::default(),
            page: match is_root {
                true => page_type.build_root(theme),
                false => page_type.build(ctx, theme),
            },
            builder,
            theme: theme.clone(),
            is_root,
            next: None,
            flow_len: 0,
        }
    }

    pub fn update(&mut self, ctx: &mut Context, next: Option<NavFn>, length: usize) {
        let mut page_type = self.builder.build(ctx, &self.theme);
        page_type.update(length, next.clone());

        let mut page = match self.is_root {
            true => page_type.build_root(&self.theme),
            false => page_type.build(ctx, &self.theme)
        };

        if let (Some(new), Some(old)) = (page.downcast_mut::<MessagesPage>(), self.page.downcast_mut::<MessagesPage>()) {
            new.take_input_from(old);
        }

        self.page = page;
        self.next = next;
        self.flow_len = length;
    }
}

#[on_event]
impl OnEvent<TickEvent> for Listener {
    fn on(&mut self, ctx: &mut Context, event: TickEvent) -> Vec<Box<dyn DynEvent>> {
        if self.builder.poll(ctx) {
            self.update(ctx, self.next.clone(), self.flow_len);
        }

        prism::events![event]
    }
}
