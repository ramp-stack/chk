use crate::ActionItem;
use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::{Context, Callback};
use ptsd::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};

use crate::{Bumper, Action};
use crate::items::Item;
use crate::closure::NavFn;

#[derive(Debug, Component, Clone)]
pub struct StackPage(Stack, PelicanPage);

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
        Self::new(ctx, theme, title, items, offset, header, bumper, next, flow_len)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        _ctx: &mut Context, theme: &Theme, title: String, items: Vec<Box<dyn Drawable>>,
        offset: Offset, header: Option<(Icons, Action)>, bumper: Bumper,
        next: Option<NavFn>, flow_len: usize,
    ) -> Self {
        let icon = header.map(|(icon, action)| (icon, action.get()));
        let (header, bumper) = match bumper {
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
                                ctx.emit(pelican_ui::navigation::NavigationEvent::Pop)
                            })
                        }) as Box<dyn Callback>,
                    )
                });
                let bumper = PelicanBumper::stack(theme, Some(&ActionItem::format_label(&label)), on_click, secondary);
                let header = Header::stack(theme, &title, icon);
                (header, Some(bumper))
            }
            Bumper::Default => match next {
                Some(n) => {
                    let next = n.clone();
                    let bumper = PelicanBumper::stack(
                        theme, None,
                        Box::new(move |ctx: &mut Context, theme: &Theme| {
                            (next.borrow_mut())(ctx, theme)
                        }),
                        None,
                    );
                    let header = Header::stack(theme, &title, icon);
                    (header, Some(bumper))
                }
                None => (
                    Header::stack_end(theme, &title),
                    Some(PelicanBumper::stack_end(theme, Some(flow_len))),
                ),
            },
            Bumper::Done => (
                Header::stack_end(theme, &title),
                Some(PelicanBumper::stack_end(theme, Some(flow_len))),
            ), Bumper::None => (Header::stack(theme, &title, icon), None),
        };

        let page = PelicanPage::new(
            header,
            Content::new(
                offset, items,
                Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
            ), bumper,
        );
        StackPage(Stack::default(), page)
    }
}
