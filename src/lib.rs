#![doc(html_logo_url = "https://raw.githubusercontent.com/ramp-stack/chk/main/logo.png")]

mod structs;
use structs::*;
mod state;
mod orange;
mod flow;
pub use flow::{PageBuilder};
mod pages;

pub enum Theme {
    Dark(Color),
    Light(Color),
    Auto(Color),
}

pub trait Application {
    fn start(ctx: &mut Context) -> Vec<Root>;
    fn theme(_assets: &mut Assets) -> Theme { Theme::Dark(Color::from_hex("#ffdd00ff", 255)) }
    fn on_event(_ctx: &mut Context, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {vec![event]}
}

extern crate self as chk;

#[doc(hidden)]
pub mod __private {
    pub use chk::{Theme, Application, structs::RootContent};
    pub use pelican_ui::start as pelican_start;

    use pelican_ui::{Context, Assets};
    use pelican_ui::events::Event;
    use pelican_ui::theme::Theme as PelicanTheme;
    use pelican_ui::components::interface::navigation::AppPage as PelicanAppPage;
    use pelican_ui::components::interface::general::Interface;
    use pelican_ui::components::interface::navigation::RootInfo;
    use pelican_ui::components::avatar::AvatarContent;

    pub struct CHK<A: Application>(A);

    impl<A: Application> pelican_ui::Application for CHK<A> {
        fn interface(ctx: &mut Context) -> Interface {
            let roots: Vec<RootInfo> = A::start(ctx).into_iter().map(|mut r| {
                let title = r.page.name();
                match r.content {
                    RootContent::Avatar(image) => RootInfo::avatar(AvatarContent::Image(image), &title, Box::new(r.page.build(ctx)) as Box<dyn PelicanAppPage>),
                    RootContent::Icon(icon) => RootInfo::icon(&icon, &title, Box::new(r.page.build(ctx)) as Box<dyn PelicanAppPage>),
                }
            }).collect();

            Interface::new(ctx, roots)
        }

        fn theme(assets: &mut Assets) -> PelicanTheme {
            match A::theme(assets) {
                Theme::Dark(c) => PelicanTheme::dark(assets, c),
                Theme::Light(c) => PelicanTheme::light(assets, c),
                Theme::Auto(c) => PelicanTheme::from(assets, c),
            }
        }

        fn on_event(_: &mut Interface, ctx: &mut Context, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
            A::on_event(ctx, event)
        }
    }
}

#[macro_export]
macro_rules! start {
    ($app:ty) => {
        pub(crate) use $crate::__private::*;

        pelican_start!(CHK<$app>);
    };
}

pub use chk::flow::Flow;

pub use chk::structs::{
    Root,
    RootContent,
    Display,
    ListItem,
    Action,
    TableItem,
    Input,
    EnumItem
};

pub use chk::pages::{
    PageType, 
    RootPage,
    Bumper,
    RootBumper,
};

pub use pelican_ui::{
    Assets,
    Context,
    State,
    drawable::Color,
    layouts::Offset,
    events::{Event, TickEvent},
};

pub mod examples {
    pub use crate::orange::Orange;
}