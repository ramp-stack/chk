use ramp::prism;
use pelican_ui::event::{OnEvent, Event, TickEvent};
use pelican_ui::drawable::{Component, Drawable, SizedTree};
use pelican_ui::{Request, drawables, Context, Callback};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::canvas::Align;
use pelican_ui::components::TextInput;
use pelican_ui::components::avatar::{Avatar, AvatarSize, AvatarContent, AvatarIconStyle};
use pelican_ui::components::list_item::{ListItemGroup, ListItem, ListItemInfoLeft};
use pelican_ui::navigation::NavigationEvent;
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::{AppPage, Flow as PelicanFlow};
use pelican_ui::components::text::{ExpandableText, TextSize, TextStyle};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::components::MessageGroups;
use std::fmt::Debug;
use pelican_ui::utils::{ValidationFn, Timestamp};

use crate::{FlowWrapper, PageType, FormItem, EditPage, Bumper};
use crate::flow::{Flow, State};
use crate::items::{Action, Input, Display};
use crate::closure::{FormSubmit, NavFn, ScreenBuilder, PageBuilder, ReviewItemGetter, SuccessGetter};

use air::names::{Secret, Id, Name};
use air::contract::{Contracts, Contract, Substance, Reactants, Reactant, Beaker};

use std::collections::BTreeMap;
use std::path::{PathBuf, Path};
use std::convert::Infallible;
use std::str::FromStr;

use std::sync::Arc;
use rand::{seq::SliceRandom, Rng};
use std::fs;

use serde::{Serialize, Deserialize};


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
        let mut items = inputs.into_iter().filter_map(|mut di| di.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        display.into_iter().for_each(|mut di| if let Some(i) = di.build(theme) {items.extend(i)});
        StackPage::new(ctx, theme, title, items, Offset::Start, header, bumper, next, flow_len)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(ctx: &mut Context, theme: &Theme, title: String, items: Vec<Box<dyn Drawable>>, offset: Offset, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize) -> Self {
        let icon = header.map(|(i, mut f)| (i, (f)(ctx, theme).build(ctx)));
        let (header, bumper) = match bumper {
            Bumper::Custom {label, action, secondary} => {
                let on_click = action.clone();
                let secondary = secondary.clone().map(|(l, a)| (l, Box::new(move |ctx: &mut Context, theme: &Theme| (a.clone().get())(ctx, theme)) as Box<dyn Callback>));
                let action = Box::new(move |ctx: &mut Context, theme: &Theme| (on_click.clone().get())(ctx, theme));
                let bumper = PelicanBumper::stack(theme, Some(&label), action, secondary);
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
