use ramp::prism;

use pelican_ui::event::{OnEvent, Event, TickEvent};
use pelican_ui::drawable::{Component, Drawable, SizedTree};
use pelican_ui::{Context, Callback, drawables};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::canvas::Align;
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::components::text::{ExpandableText, TextSize, TextStyle};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::components::{MessageGroups, SendStickerEvent, SendGifEvent, Sticker};
use pelican_ui::navigation::{NavigationEvent};
use pelican_ui::utils::Timestamp;
use pelican_ui::components::{SendPhotoEvent, SendVideoEvent};

use crate::messages::{ChatRoom, Message, ChatRoomAction, MessageContent};
use crate::profiles::{Profile, ProfileAction};
use crate::blocking::Block;
use crate::{Page, PageBuilder, ActionItem, PageType, FormItem, Bumper, Listener};
use crate::flow::Flow;
use crate::form::{State, FormValidState, FormComplete};
use crate::items::{Action, Display, AvatarPurpose, TableItem};
use crate::closure::{FormSubmit, NavFn, ReviewItemGetter, SuccessGetter, FlowBuilder};

use maverick_os::air::{Id, Name, Instance};

use std::str::FromStr;
use std::sync::Arc;

mod messages;
pub use messages::{MessagesPage, MessageDetails};
mod profiles;
pub use profiles::{ProfilePage, ProfileView};


#[derive(Debug, Clone)]
pub struct Success {
    pub title: String,
    pub getter: Box<dyn SuccessGetter>,
}

impl Success {
    pub fn new(title: &str, getter: impl SuccessGetter + 'static) -> Self {
        Success{title: title.to_string(), getter: Box::new(getter)}
    }
}

#[derive(Debug, Clone)]
pub struct Review {
    pub title: String,
    pub getter: Box<dyn ReviewItemGetter>,
}

impl Review {
    pub fn new(title: &str, getter: impl ReviewItemGetter + 'static) -> Self {
        Review{title: title.to_string(), getter: Box::new(getter)}
    }
}

#[derive(Debug, Component, Clone)]
pub struct ReviewPage(Stack, pub PelicanPage, #[skip] Box<dyn ReviewItemGetter>, #[skip] Theme, #[skip] Option<NavFn>, #[skip] Box<dyn FormSubmit>, #[skip] bool);
impl OnEvent for ReviewPage {}
impl AppPage for ReviewPage {}
impl ReviewPage {
    pub fn new(theme: &Theme, title: String, item_getter: Box<dyn ReviewItemGetter>, next: Option<NavFn>, _flow_len: usize, on_submit: Box<dyn FormSubmit>) -> Self {
        let header = Header::stack(theme, &title, None);
        let bumper = PelicanBumper::stack(theme, None, Box::new(|_ctx: &mut Context, _theme: &Theme| {}), None);
        let page = PelicanPage::new(header, Content::new(Offset::Start, Vec::new(), Box::new(|_, _| true)), Some(bumper));
        ReviewPage(Stack::default(), page, item_getter, theme.clone(), next.clone(), on_submit.clone(), false)
    }

    pub fn on_change(&mut self, new: Vec<State>) {
        if !self.6 {
            self.6 = true;
            let theme = &self.3;
            let items = (self.2)(&new);
            let content = items.into_iter().filter_map(|mut i| i.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
            self.1.content = Content::new(Offset::Start, content, Box::new(|_, _| true));
        }

        let mut on_submit = self.5.clone();
        *self.1.bumper.as_mut().unwrap().on_click()[0] = {
            let mut on_click = self.4.clone().map(|n| {
                Box::new(move |ctx: &mut Context, theme: &crate::Theme| {
                    (on_submit)(ctx, &new);
                    (n.borrow_mut())(ctx, theme);
                }) as Box<dyn Callback>
            }).unwrap_or(Box::new(|_ctx: &mut Context, _theme: &crate::Theme| {}));
            let theme = self.3.clone();
            Box::new(move |ctx: &mut Context| (on_click)(ctx, &theme))
        }
    }
}

#[derive(Debug, Component, Clone)]
pub struct SuccessPage(Stack, pub PelicanPage, #[skip] Box<dyn SuccessGetter>, #[skip] Theme, #[skip] bool, #[skip] Option<Box<dyn FormSubmit>>);
impl OnEvent for SuccessPage {}
impl AppPage for SuccessPage {}
impl SuccessPage {
    pub fn new(theme: &Theme, title: String, getter: Box<dyn SuccessGetter>, flow_len: usize, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        let header = Header::stack_end(theme, &title);
        let bumper = Some(PelicanBumper::stack_end(theme, Some(flow_len)));
        let page = PelicanPage::new(header, Content::new(Offset::Center, vec![], Box::new(|_, _| true)), bumper);
        SuccessPage(Stack::default(), page, getter, theme.clone(), false, on_submit.clone())
    }

    pub fn on_change(&mut self, ctx: &mut Context, new: Vec<State>) {
        use pelican_ui::colors;
        use pelican_ui::components::Icon;
        if let Some(on_submit) = &mut self.5 {(on_submit)(ctx, &new);}
        let theme = self.3.clone();
        let (icon, description) = (self.2)(new);
        self.1.content = Content::new(Offset::Center, drawables![
            Icon::new(&theme, icon, Some(theme.colors().get(colors::Text::Heading)), 128.0),
            ExpandableText::new(&theme, &description, TextSize::H4, TextStyle::Heading, Align::Center, None)
        ], Box::new(|_, _| true));
    }
}
