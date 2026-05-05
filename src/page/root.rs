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
pub struct RootPage(Stack, PelicanPage);
impl OnEvent for RootPage {}
impl AppPage for RootPage {}
impl RootPage {
    pub fn new(theme: &Theme, title: String, mut input: Vec<Input>, mut display: Vec<Display>, header: Option<(Icons, Box<dyn FlowBuilder>)>, mut bumper_a: Option<(String, Box<dyn FlowBuilder>)>, mut bumper_b: Option<(String, Box<dyn FlowBuilder>)>) -> Self {
        let length = input.len() + display.len();
        let offset = match display.first() {
            Some(Display::List {..}) => Offset::Start,
            _ if length <= 1 => Offset::Center,
            _ => Offset::Start,
        };
        
        let header_icon = header.map(|(s, flow)| {
            let mut flow = flow.clone();
            (s, Box::new(move |ctx: &mut Context, theme: &Theme| ((flow)(ctx, theme).build(ctx))(ctx, theme)) as Box<dyn Callback>) 
        });

        let header = Header::home(theme, &title, header_icon);
        let mut content = input.iter_mut().filter_map(|di| di.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        content.extend(display.iter_mut().filter_map(|di| di.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>());

        let second = bumper_b.as_mut().map(|(t, flow)| {
            let mut flow = flow.clone();
            (t.to_string(), Box::new(move |ctx: &mut Context, theme: &Theme| ((flow)(ctx, theme).build(ctx))(ctx, theme)) as Box<dyn Callback>)
        });

        let first = bumper_a.as_mut().map(|(t, flow)| {
            let mut flow = flow.clone();
            (t.to_string(), Box::new(move |ctx: &mut Context, theme: &Theme| ((flow)(ctx, theme).build(ctx))(ctx, theme)) as Box<dyn Callback>)
        });

        let bumper = PelicanBumper::home(theme, first, second);
        let page = PelicanPage::new(header, Content::new(offset, content, Box::new(|_, _| true)), Some(bumper));
        RootPage(Stack::default(), page)
    }
}
