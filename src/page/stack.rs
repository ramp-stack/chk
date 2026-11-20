use prism::drawable::{Component, Drawable};
use prism::events::ListenCallback;
use ptsd::layout::{Stack, Offset};
use ptsd::navigation::AppPage;

use std::ops::{Deref, DerefMut};

use crate::design_system::{Content, Context, Footer, Header, PageEvent, PageView, Theme};
use crate::items::Item;

#[derive(Debug, Component, Clone)]
pub struct StackPage<D: Drawable + Clone = PageView>(Stack, D);

impl<D: Drawable + Clone> AppPage for StackPage<D> {}
impl<D: Drawable + Clone> StackPage<D> {
    pub fn new(content: D) -> Self {StackPage(Stack::default(), content)}
}

impl<D: Drawable + Clone> Deref for StackPage<D> {
    type Target = D;
    fn deref(&self) -> &D {&self.1}
}
impl<D: Drawable + Clone> DerefMut for StackPage<D> {
    fn deref_mut(&mut self) -> &mut D {&mut self.1}
}

impl StackPage<Box<dyn Drawable>> {
    pub fn edge_to_edge(theme: &Theme, header: Header, content: Box<dyn Drawable>) -> Self {
        StackPage(Stack::fill(), theme.edge_to_edge(header, content))
    }
}

impl StackPage<PageView> {
    #[allow(clippy::too_many_arguments, reason = "Page setup is supplied directly at construction.")]
    pub fn content(
        ctx: &mut Context, theme: &Theme, header: Header, items: Vec<Item>, offset: Offset,
        footer: Option<Footer>, on_event: Option<Box<dyn PageEvent>>,
        on_create: Option<ListenCallback<PageView>>,
    ) -> Self {
        let items = items.into_iter().filter_map(|mut item| item.build(ctx, theme))
            .flatten().collect();
        let content = Content::new(offset, items,
            Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true));
        StackPage(Stack::default(), theme.page(
            ctx, header, content, footer, vec![], false, None, on_event, None, on_create,
        ))
    }
}
