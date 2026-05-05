use ramp::prism;
use pelican_ui::event::OnEvent;
use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::{Context, Callback};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};

use crate::{FlowBuilder, PageType};
use crate::items::{Input, Display};

pub struct Root(pub PageType);
impl Root {
    pub fn new(title: &str, items: Vec<Display>, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper_a: (String, Box<dyn FlowBuilder>), bumper_b: Option<(String, Box<dyn FlowBuilder>)>) -> Self {
        Root(PageType::root(title, vec![], items, header, bumper_a, bumper_b))
    }

    pub fn custom(page: PageType) -> Self {
        Root(page)
    }
}

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
