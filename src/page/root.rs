use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::{Context, Callback};
use ptsd::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};

use crate::{Page, FlowBuilder, PageType};
use crate::page::EditPage;
use crate::items::Display;

pub struct Root(pub Page);
impl Root {
    pub fn new(
        title: &str, items: Vec<Display>, header: Option<(Icons, Box<dyn FlowBuilder>)>,
        bumper_a: Option<(String, Box<dyn FlowBuilder>)>,
        bumper_b: Option<(String, Box<dyn FlowBuilder>)>,
    ) -> Self {
        Root(Page::Static(Box::new(PageType::root(title, vec![], items, header, bumper_a, bumper_b))))
    }

    pub fn custom(page: Page) -> Self { Root(page) }
}

#[derive(Debug, Component, Clone)]
pub struct RootPage(Stack, PelicanPage);

impl AppPage for RootPage {}
impl PageType {
    pub fn build_root(&self, theme: &Theme, is_mobile: bool) -> Box<dyn AppPage> {
        let (title, mut input, mut display, header, mut bumper_a, mut bumper_b) = match self.clone() {
            Self::Root { title, input, display, header, bumper_a, bumper_b } => {
                (title, input, display, header, bumper_a, bumper_b)
            }
            Self::Both { title, inputs, display, header, .. } => {
                (title, inputs, display, header, None, None)
            }
            Self::Display { title, items, header, .. } => {
                (title, vec![], items, header, None, None)
            }
            Self::Input { title, item, header, .. } => {
                (title, vec![item], vec![], header, None, None)
            }
            Self::EditAndDisplay { title, header, items, display, on_save, .. } => {
                return Box::new(EditPage::root(
                    theme, is_mobile, title, header, items, display, on_save,
                ));
            }
            Self::Edit { .. } | Self::Form { .. } | Self::Review { .. } | Self::Success { .. }
            | Self::Messaging { .. } | Self::Photo { .. } | Self::Video { .. } => {
                panic!("Not an accepted root type");
            }
        };
        let length = input.len() + display.len();
        let offset = match display.first() {
            Some(Display::List { .. }) => Offset::Start,
            _ if length <= 1 => Offset::Center, _ => Offset::Start,
        };

        let header_icon = header.map(|(s, flow)| {
            let mut flow = flow.clone();
            (
                s,
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    ((flow)(ctx, theme).build(ctx, theme))(ctx, theme)
                }) as Box<dyn Callback>,
            )
        });

        let header = Header::home(theme, &title, header_icon);
        let mut content = input
            .iter_mut()
            .filter_map(|di| di.build(theme, is_mobile))
            .flatten()
            .collect::<Vec<Box<dyn Drawable>>>();
        content.extend(
            display
                .iter_mut()
                .filter_map(|di| di.build(theme))
                .flatten()
                .collect::<Vec<Box<dyn Drawable>>>(),
        );

        let second = bumper_b.as_mut().map(|(t, flow)| {
            let mut flow = flow.clone();
            (
                t.to_string(),
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    ((flow)(ctx, theme).build(ctx, theme))(ctx, theme)
                }) as Box<dyn Callback>,
            )
        });

        let first = bumper_a.as_mut().map(|(t, flow)| {
            let mut flow = flow.clone();
            (
                t.to_string(),
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    ((flow)(ctx, theme).build(ctx, theme))(ctx, theme)
                }) as Box<dyn Callback>,
            )
        });

        let bumper = match first.is_some() || second.is_some() {
            true => Some(PelicanBumper::home(theme, first, second)), false => None,
        };

        let page = PelicanPage::new(
            header,
            Content::new(
                offset, content,
                Box::new(|_: &mut Context, _: Vec<&mut Box<dyn Drawable>>| true),
            ), bumper,
        );
        Box::new(RootPage(Stack::default(), page))
    }
}
