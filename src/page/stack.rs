use prism::drawable::{Component, Drawable};
use ptsd::layout::{Stack, Offset};
use ptsd::navigation::AppPage;

use crate::ActionItem;
use crate::design_system::{Context, Callback};
use crate::design_system::PageView;
use crate::design_system::{Theme, Icons};
use crate::{Bumper, Action};
use crate::items::Item;
use crate::closure::NavFn;
use crate::design_system::{Header, Footer, Content};

#[derive(Debug, Component, Clone)]
pub struct StackPage(Stack, PageView);

impl AppPage for StackPage {}
impl StackPage {
    #[allow(clippy::too_many_arguments)]
    pub fn content(
        ctx: &mut Context, theme: &Theme, title: String, items: Vec<Item>, offset: Offset,
        header: Option<(Icons, Action)>, bumper: Bumper, next: Option<NavFn>,
        flow_len: usize,
    ) -> Self {
        let items = items.into_iter().filter_map(|mut item| item.build(theme, ctx.window.is_mobile))
            .flatten().collect();
        let (header, footer) = {
            let title = title.as_str();
            let icon = header.map(|(icon, action)| (icon, action.get()));
            match bumper {
                Bumper::Custom { label, action, secondary } => {
                    let on_click = match next {
                        Some(n) => {
                            let next = n.clone();
                            Box::new(move |ctx: &mut Context, theme: &Theme| {
                                (next.borrow_mut())(ctx, theme);
                                (action.clone().get())(ctx, theme);
                            }) as Box<dyn Callback>
                        }
                        None => Box::new(move |ctx: &mut Context, theme: &Theme| {
                            (action.clone().get())(ctx, theme)
                        }) as Box<dyn Callback>,
                    };
                    let secondary = secondary.clone().map(|(l, a)| {
                        (
                            ActionItem::format_label(&l),
                            Box::new(move |ctx: &mut Context, theme: &Theme| {
                                (a.clone().get())(ctx, theme);
                                (0..1).for_each(|_| {
                                    ctx.emit(ptsd::navigation::NavigationEvent::Pop)
                                })
                            }) as Box<dyn Callback>,
                        )
                    });
                    (Header::stack(title, icon),
                        Some(Footer::stack(Some(&ActionItem::format_label(&label)), on_click, secondary)))
                }
                Bumper::Default => match next {
                    Some(n) => {
                        let next = n.clone();
                        (Header::stack(title, icon), Some(Footer::stack(None,
                            Box::new(move |ctx: &mut Context, theme: &Theme| (next.borrow_mut())(ctx, theme)), None)))

                    }
                    None => (Header::stack_end(title), Some(Footer::stack_end(Some(flow_len)))),
                },
                Bumper::Done => (Header::stack_end(title), Some(Footer::stack_end(Some(flow_len)))),
                Bumper::None => (Header::stack(title, icon), None),
            }
        };
        let page = theme.page(header, Content::new(offset, items, Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true)), footer);
        Self(Stack::default(), page)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        _ctx: &mut Context, theme: &Theme, title: String, items: Vec<Box<dyn Drawable>>,
        offset: Offset, header: Option<(Icons, Action)>, bumper: Bumper,
        next: Option<NavFn>, flow_len: usize,
    ) -> Self {
        let (header, footer) = {
            let title = title.as_str();
            let icon = header.map(|(icon, action)| (icon, action.get()));
            match bumper {
                Bumper::Custom { label, action, secondary } => {
                    let on_click = match next {
                        Some(n) => {
                            let next = n.clone();
                            Box::new(move |ctx: &mut Context, theme: &Theme| {
                                (next.borrow_mut())(ctx, theme);
                                (action.clone().get())(ctx, theme);
                            }) as Box<dyn Callback>
                        }
                        None => Box::new(move |ctx: &mut Context, theme: &Theme| {
                            (action.clone().get())(ctx, theme)
                        }) as Box<dyn Callback>,
                    };
                    let secondary = secondary.clone().map(|(l, a)| {
                        (
                            ActionItem::format_label(&l),
                            Box::new(move |ctx: &mut Context, theme: &Theme| {
                                (a.clone().get())(ctx, theme);
                                (0..1).for_each(|_| {
                                    ctx.emit(ptsd::navigation::NavigationEvent::Pop)
                                })
                            }) as Box<dyn Callback>,
                        )
                    });
                    (Header::stack(title, icon),
                        Some(Footer::stack(Some(&ActionItem::format_label(&label)), on_click, secondary)))
                }
                Bumper::Default => match next {
                    Some(n) => {
                        let next = n.clone();
                        (Header::stack(title, icon), Some(Footer::stack(None,
                            Box::new(move |ctx: &mut Context, theme: &Theme| (next.borrow_mut())(ctx, theme)), None)))

                    }
                    None => (Header::stack_end(title), Some(Footer::stack_end(Some(flow_len)))),
                },
                Bumper::Done => (Header::stack_end(title), Some(Footer::stack_end(Some(flow_len)))),
                Bumper::None => (Header::stack(title, icon), None),
            }
        };
        let page = theme.page(header, Content::new(offset, items, Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true)), footer);
        Self(Stack::default(), page)
    }

}
