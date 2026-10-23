use pelican_ui::theme::Theme;
use prism::drawable::Component;
use ptsd::layout::Stack;
use prism::events::{on_change, OnChange, Event, ChildEvent};
use uuid::Uuid;
use pelican_ui::navigation::AppPage;
use pelican_ui::Context;

use crate::{MessagesPage, PageBuilder, NavFn};

#[derive(Debug, Clone, Copy)]
pub struct PageChanged(pub Uuid);
impl Event for PageChanged {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children { *child.event = Some(*self); }
    }
}

#[derive(Debug, Component, Clone)]
pub struct Listener {
    layout: Stack,
    pub page: Box<dyn AppPage>,
    #[skip]
    builder: Box<dyn PageBuilder>,
    #[skip]
    theme: Theme,
    #[skip]
    is_root: bool,
    #[skip]
    next: Option<NavFn>,
    #[skip]
    flow_len: usize,
    #[skip]
    id: Uuid,
}

impl AppPage for Listener {}

impl Listener {
    pub fn new(ctx: &mut Context, theme: &Theme, mut builder: Box<dyn PageBuilder>, is_root: bool, next: Option<NavFn>, flow_len: usize) -> Self {
        let mut page_type = builder.build(ctx, theme);
        page_type.update(flow_len, next.clone());
        let page = if is_root {
            page_type.build_root(theme, ctx.window.touch_controls)
        } else {
            page_type.build(ctx, theme)
        };
        Self {
            layout: Stack::default(),
            page, builder,
            theme: theme.clone(),
            is_root, next, flow_len,
            id: Uuid::new_v4(),
        }
    }
    pub fn id(&self) -> Uuid { self.id }
}

#[on_change(async {
    if self.builder.next().await.is_none() {std::future::pending::<()>().await;}
})]
impl OnChange<()> for Listener {
    fn on_change(&mut self, ctx: &mut Context, _: ()) {
        let mut page_type = self.builder.build(ctx, &self.theme);
        page_type.update(self.flow_len, self.next.clone());
        let mut page = if self.is_root {
            page_type.build_root(&self.theme, ctx.window.touch_controls)
        } else {
            page_type.build(ctx, &self.theme)
        };
        if let (Some(new), Some(old)) =
            (page.downcast_mut::<MessagesPage>(), self.page.downcast_mut::<MessagesPage>()) {
            new.take_input_from(old);
        }
        self.page = page;
        ctx.emit(PageChanged(self.id));
    }
}
