use pelican_ui::{Context, Component};
use pelican_ui::drawable::Drawable;
use pelican_ui::layouts::{Offset, Stack};
use pelican_ui::events::OnEvent;
use pelican_ui::components::interface::navigation::AppPage as PelicanAppPage;
use pelican_ui::components::interface::general::{Header, Bumper as PelicanBumper, Content, Page as PelicanPage};
use pelican_ui::utils::Callback;

use crate::{Action, Input, Display, FnMutClone, IconButton, NavFn};
use crate::flow::Flow;


#[derive(Debug, Clone)]
pub enum PageType {
    Display {title: String, items: Vec<Display>, bumper: Bumper, offset: Offset},
    Input {title: String, items: Input, bumper: Bumper}, // custom or continue
}

impl PageType {
    pub fn success(title: &str, icon: &str, text: &str) -> Self {
        PageType::Display { 
            title: title.to_string(),
            items: vec![Display::icon(icon), Display::text(text)], 
            bumper: Bumper::Done,
            offset: Offset::Center,
        }
    }

    pub fn review(title: &str, items: Vec<Display>) -> Self {
        PageType::Display { title: title.to_string(), items, bumper: Bumper::default(), offset: Offset::Start}
    }

    pub fn input(title: &str, items: Input, bumper: Bumper) -> Self {
        PageType::Input { title: title.to_string(), items, bumper }
    }

    pub fn display(title: &str, items: Vec<Display>, bumper: Bumper, offset: Offset) -> Self {
        PageType::Display { title: title.to_string(), items, bumper, offset}
    }

    pub fn name(&self) -> String {
        match self {
            PageType::Display {title, ..} |
            PageType::Input {title, ..} => title.to_string(),
        }
    }

    pub(crate) fn build(&mut self, ctx: &mut Context, next: Option<NavFn>) -> AppPage {
        let (offset, content, validity_fn) = match self {
            PageType::Display {items, offset, ..} => (*offset, items.iter_mut().filter_map(|di| di.build(ctx)).flatten().collect::<Vec<Box<dyn Drawable>>>(), None),
            PageType::Input {items, ..} => (Offset::Start, items.build(ctx).unwrap_or_default(), items.check()),
        };

        let validity_fn = validity_fn.map(|mut vfn| Box::new(move |ctx: &mut Context| (vfn)(ctx)) as Box<dyn FnMut(&mut Context) -> bool + 'static>);

        let bumper = match self {
            PageType::Display {bumper, ..} => bumper,
            PageType::Input {bumper, ..} => bumper,
        };

        let (header, bumper) = match bumper {
            Bumper::Custom {label, action} => {
                let on_click = action.clone();
                let bumper = PelicanBumper::stack(ctx, Some(label), false, Box::new(move |ctx: &mut Context| (on_click.clone().get())(ctx)), validity_fn);
                let header = Header::stack(ctx, &self.name(), None);
                (header, bumper)
            },
            Bumper::Default => match next {
                Some(n) => {
                    let next = n.clone();
                    let bumper = PelicanBumper::stack(ctx, None, false, Box::new(move |ctx: &mut Context| (next.borrow_mut())(ctx)), validity_fn);
                    let header = Header::stack(ctx, &self.name(), None);
                    (header, bumper)
                }
                None => (Header::stack_end(ctx, &self.name()), PelicanBumper::stack_end(ctx))
            },
            Bumper::Done => (Header::stack_end(ctx, &self.name()), PelicanBumper::stack_end(ctx)),
        };

        AppPage::new(header, Content::new(ctx, offset, content), Some(bumper))
    }
}

/// Represents the first page of a tab.
#[derive(Debug, Clone)]
pub struct RootPage {
    title: String,
    icon_button: Option<IconButton>,
    content: Vec<Display>,
    bumper: (RootBumper, Option<RootBumper>)
}

impl RootPage {
    pub fn new(title: &str, content: Vec<Display>, icon_button: Option<(&str, Box<dyn FnMutClone>)>, first: RootBumper, second: Option<RootBumper>) -> Self {
        RootPage {
            title: title.to_string(), 
            content, 
            icon_button: icon_button.map(|i| IconButton::from(i)),
            bumper: (first, second)
        }
    }

    pub fn name(&self) -> String { self.title.to_string() }

    pub(crate) fn build(&mut self, ctx: &mut Context) -> AppPage {
        let header = Header::home(ctx, &self.name(), self.icon_button.as_ref().map(|i| i.get()));
        let content = self.content.iter_mut().filter_map(|di| di.build(ctx)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        let second = self.bumper.1.as_mut().map(|i| i.get());
        let first = self.bumper.0.get();
        let bumper = PelicanBumper::home(ctx, first, second, None);

        let offset = if content.len() <= 1 {Offset::Center} else {Offset::Start};
        AppPage::new(header, Content::new(ctx, offset, content), Some(bumper))
    }
}

#[derive(Component, Debug)]
pub struct AppPage(Stack, PelicanPage);
impl OnEvent for AppPage {}
impl PelicanAppPage for AppPage {}

impl AppPage {
    pub fn new(header: Header, content: Content, bumper: Option<PelicanBumper>) -> Self {
        AppPage(Stack::default(), PelicanPage::new(header, content, bumper))
    }
}

/// Represents a bumper item on the first page of a tab.
#[derive(Debug, Clone)]
pub struct RootBumper(String, Flow);

impl RootBumper {
    pub fn new(label: &str, flow: Flow) -> Self {
        RootBumper(label.to_string(), flow)
    }

    pub fn get(&mut self) -> (String, Callback) {
        (self.0.to_string(), self.1.build())
    }
}

#[derive(Debug, Clone, Default)]
pub enum Bumper {
    #[default]
    Default,
    Custom { label: String, action: Action },
    Done,
}

impl Bumper {
    pub fn custom(label: &str, action: Action) -> Self {
        Bumper::Custom {label: label.to_string(), action}
    }
}