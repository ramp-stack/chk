use crate::ActionItem;
use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::{Context};
use ptsd::layout::{Stack, Offset};
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};

use crate::{Page, Action, PageType};
use crate::page::EditPage;
use crate::items::{Display, Item};

pub struct Root(pub Page);
impl Root {
    pub fn new(
        title: &str, items: Vec<Display>, header: Option<(Icons, Action)>,
        bumper_a: Option<(String, Action)>,
        bumper_b: Option<(String, Action)>,
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
        let (title, items, header, mut bumper_a, mut bumper_b) = match self.clone() {
            Self::Root { title, input, display, header, bumper_a, bumper_b } => {
                (title, input.into_iter().map(Item::Input).chain(display.into_iter().map(Item::Display)).collect::<Vec<_>>(), header, bumper_a, bumper_b)
            }
            Self::Content { title, items, header, .. } => {
                (title, items, header, None, None)
            }
            Self::Edit { title, header, items, display, on_save, .. } => {
                return Box::new(EditPage::root(
                    theme, is_mobile, title, header, items, display, on_save,
                ));
            }
            Self::Form { .. } | Self::Review { .. } | Self::Success { .. }
            | Self::Messaging { .. } | Self::Photo { .. } | Self::Video { .. } | Self::Playback { .. } => {
                panic!("Not an accepted root type");
            }
        };
        let length = items.len();
        let offset = match items.iter().find_map(|item| match item {Item::Display(display) => Some(display), _ => None}) {
            Some(Display::List { .. }) => Offset::Start,
            _ if length <= 1 => Offset::Center, _ => Offset::Start,
        };

        let header_icon = header.map(|(icon, action)| (icon, action.get()));

        let header = Header::home(theme, &title, header_icon);
        let content = items.into_iter().filter_map(|mut item| item.build(theme, is_mobile))
            .flatten().collect::<Vec<Box<dyn Drawable>>>();

        let second = bumper_b.as_mut().map(|(label, action)| (ActionItem::format_label(label), action.get()));
        let first = bumper_a.as_mut().map(|(label, action)| (ActionItem::format_label(label), action.get()));

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
