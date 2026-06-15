use ramp::prism;
use pelican_ui::event::OnEvent;
use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::{Context, Callback};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};

use crate::{Bumper, FlowBuilder};
use crate::items::{Input, Display};
use crate::closure::NavFn;

#[derive(Debug, Component, Clone)]
pub struct StackPage(Stack, PelicanPage);
impl OnEvent for StackPage {}
impl AppPage for StackPage {}
impl StackPage {
    #[allow(clippy::too_many_arguments)]
    pub fn display(ctx: &mut Context, theme: &Theme, title: String, items: Vec<Display>, offset: Offset, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize) -> Self {
        let items = items.into_iter().filter_map(|mut di| di.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        StackPage::new(ctx, theme, title, items, offset, header, bumper, next, flow_len)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn input(ctx: &mut Context, theme: &Theme, title: String, item: Input, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize) -> Self {
        let offset = item.offset();
        let item = item.build(theme).into_iter().flatten().collect();
        StackPage::new(ctx, theme, title, item, offset, header, bumper, next, flow_len)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn both(ctx: &mut Context, theme: &Theme, title: String, display: Vec<Display>, inputs: Vec<Input>, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize) -> Self {
        let mut items = inputs.into_iter().filter_map(|di| di.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        display.into_iter().for_each(|mut di| if let Some(i) = di.build(theme) {items.extend(i)});
        StackPage::new(ctx, theme, title, items, Offset::Start, header, bumper, next, flow_len)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(ctx: &mut Context, theme: &Theme, title: String, items: Vec<Box<dyn Drawable>>, offset: Offset, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize) -> Self {
        let icon = header.map(|(i, mut f)| (i, (f)(ctx, theme).build(ctx)));
        let (header, bumper) = match bumper {
            Bumper::Custom {label, action, secondary} => {
                let on_click = match next {
                    Some(n) => {
                        let next = n.clone();
                        Box::new(move |ctx: &mut Context, theme: &Theme| {
                            (next.borrow_mut())(ctx, theme);
                            (action.clone().get())(ctx, theme);
                        }) as Box<dyn Callback>
                    }
                    None => Box::new(move |ctx: &mut Context, theme: &Theme| (action.clone().get())(ctx, theme)) as Box<dyn Callback>,
                };
                let secondary = secondary.clone().map(|(l, a)| (l, Box::new(move |ctx: &mut Context, theme: &Theme| {
                    (a.clone().get())(ctx, theme);
                    (0..1).for_each(|_| ctx.emit(pelican_ui::navigation::NavigationEvent::Pop))
                }) as Box<dyn Callback>));
                let bumper = PelicanBumper::stack(theme, Some(&label), on_click, secondary);
                let header = Header::stack(theme, &title, icon);
                (header, Some(bumper))
            },
            Bumper::Default => match next {
                Some(n) => {
                    let next = n.clone();
                    let bumper = PelicanBumper::stack(theme, None, Box::new(move |ctx: &mut Context, theme: &Theme| (next.borrow_mut())(ctx, theme)), None);
                    let header = Header::stack(theme, &title, icon);
                    (header, Some(bumper))
                }
                None => (Header::stack_end(theme, &title), Some(PelicanBumper::stack_end(theme, Some(flow_len))))
            },
            Bumper::Done => (Header::stack_end(theme, &title), Some(PelicanBumper::stack_end(theme, Some(flow_len)))),
            Bumper::None => (Header::stack(theme, &title, icon), None),
        };

        let page = PelicanPage::new(header, Content::new(offset, items, Box::new(|_, _| true)), bumper);
        StackPage(Stack::default(), page)
    }
}
