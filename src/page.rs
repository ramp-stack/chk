use ramp::prism;
use pelican_ui::event::OnEvent;
use pelican_ui::drawable::Component;
use pelican_ui::{Context, IS_MOBILE};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};
use std::fmt::Debug;
use pelican_ui::utils::ValidationFn;

use crate::form::FormItem;
use crate::items::{Action, Input, Display};
use crate::closure::{FormSubmit, NavFn, ScreenBuilder, PageBuilder, ReviewItemGetter, SuccessGetter, FlowBuilder};

use air::Instance;
use air::names::Id;

use crate::messages::ChatRoom;
use crate::profiles::Profile;

mod edit;
pub use edit::*;
mod form;
pub use form::*;
mod presets;
pub use presets::*;
mod root;
pub use root::*;
mod stack;
pub use stack::*;

#[derive(Debug, Component, Clone)]
pub struct Screen(Stack, pub Box<dyn AppPage>, #[skip] Box<dyn PageBuilder>, #[skip] Option<NavFn>, #[skip] usize, #[skip] Theme);
impl AppPage for Screen {}
impl OnEvent for Screen {}

impl Screen {
    pub fn new(ctx: &mut Context, theme: &Theme, mut page_builder: Box<dyn PageBuilder>) -> Self {
        Screen(Stack::default(), ((page_builder)()).build(ctx, theme), page_builder, None, 1, theme.clone())
    }

    pub fn update(&mut self, ctx: &mut Context, new_len: usize, new_fn: Option<NavFn>) {
        let theme = &self.5;
        self.3 = new_fn.clone();
        self.4 = new_len;
        let mut page_type = (self.2)();
        if let Some(l) = page_type.length() { *l = self.4; }
        if let Some(nav) = page_type.nav_fn() {
            *nav = self.3.clone();
        }
        self.1 = page_type.build(ctx, theme);
    }

    pub fn new_builder(theme: &crate::Theme, page_builder: Box<dyn PageBuilder>) -> Box<dyn ScreenBuilder> {
        let theme = theme.clone();
        Box::new(move |ctx: &mut Context| Screen::new(ctx, &theme.clone(), page_builder.clone())) as Box<dyn ScreenBuilder>
    }
}

#[derive(Clone, Debug)]
pub enum PageType {
    Root {title: String, input: Vec<Input>, display: Vec<Display>, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper_a: Option<(String, Box<dyn FlowBuilder>)>, bumper_b: Option<(String, Box<dyn FlowBuilder>)>},
    Both{title: String, display: Vec<Display>, inputs: Vec<Input>, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize},
    EditAndDisplay {title: String, items: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>, flow_len: usize},
    Display{title: String, items: Vec<Display>, offset: Offset, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize},
    Input{title: String, item: Input, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper, flow_len: usize, next: Option<NavFn>},
    Form{title: String, item: Input, flow_len: usize, next: Option<NavFn>, validate: Box<dyn ValidationFn>, on_submit: Option<Box<dyn FormSubmit>>},
    Edit{title: String, input: Vec<Input>, display: Vec<Display>, validations: Vec<Box<dyn ValidationFn>>, on_save: Box<dyn FormSubmit>, flow_len: usize},
    Review{title: String, getter: Box<dyn ReviewItemGetter>, flow_len: usize, next: Option<NavFn>, on_submit: Box<dyn FormSubmit>},
    Success{title: String, getter: Box<dyn SuccessGetter>, flow_len: usize, on_submit: Option<Box<dyn FormSubmit>>},
    Messaging{room: Instance<ChatRoom>, flow_len: usize},
    Profile{profile: Instance<Profile>},
}

impl PageType {
    pub fn root(title: &str, input: Vec<Input>, display: Vec<Display>, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper_a: Option<(String, Box<dyn FlowBuilder>)>, bumper_b: Option<(String, Box<dyn FlowBuilder>)>) -> Self {
        PageType::Root { title: title.to_string(), input, display, header, bumper_a, bumper_b }
    }

    pub fn display_and_input(title: &str, inputs: Vec<Input>, display: Vec<Display>, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper) -> Self {
        PageType::Both{title: title.to_string(), display, inputs, header, bumper, flow_len: 1, next: None }
    }

    pub fn display(title: &str, items: Vec<Display>, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper, offset: Offset) -> Self {
        PageType::Display { title: title.to_string(), items, header, bumper, offset, flow_len: 1, next: None }
    }

    pub fn input(title: &str, item: Input, header: Option<(Icons, Box<dyn FlowBuilder>)>, bumper: Bumper) -> Self {
        PageType::Input { title: title.to_string(), item, header, bumper, flow_len: 1, next: None }
    }

    pub fn form(title: &str, item: Input, validate: Box<dyn ValidationFn>, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        PageType::Form { title: title.to_string(), validate, item, flow_len: 1, next: None, on_submit }
    }

    pub fn edit(title: &str, input: Vec<Input>, display: Vec<Display>, validations: Vec<Box<dyn ValidationFn>>, on_save: Box<dyn FormSubmit>) -> Self {
        PageType::Edit { title: title.to_string(), input, display, validations, on_save, flow_len: 1}
    }

    pub fn edit_and_display(title: &str, items: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        PageType::EditAndDisplay { title: title.to_string(), items, display, on_save, flow_len: 1}
    }

    pub fn review(title: &str, getter: Box<dyn ReviewItemGetter>, on_submit: Box<dyn FormSubmit>) -> Self {
        PageType::Review { title: title.to_string(), getter, flow_len: 1, next: None, on_submit }
    }

    pub fn success(title: &str, getter: Box<dyn SuccessGetter>, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        PageType::Success { title: title.to_string(), getter, flow_len: 1, on_submit }
    }

    pub fn messaging(room: Instance<ChatRoom>) -> Self {
        PageType::Messaging{ room, flow_len: 1 }
    }

    pub fn scan_qr(instructions: &str, alt: Option<(String, Icons, Action)>) -> Self {
        PageType::input("Scan QR code", Input::qr_code_scanner(instructions, alt), None, crate::Bumper::None)
    }

    pub fn display_qr_code(title: &str, data: &str, instructions: &str) -> Self {
        PageType::display(title, vec![Display::qr_code(&data, instructions)],
            None,
            Bumper::custom(
                if IS_MOBILE {"Share"} else {"Copy"}, 
                if IS_MOBILE {Action::share(&data)} else {Action::copy(&data)}
            ),
            Offset::Center,
        )
    }

    pub fn profile(profile: Instance<Profile>) -> Self {
        PageType::Profile { profile }
    }

    pub fn nav_fn(&mut self) -> Option<&mut Option<NavFn>> {
        match self {
            PageType::Root{..} |
            PageType::Messaging{..} |
            PageType::Profile{..} |
            PageType::Edit{..} |
            PageType::EditAndDisplay {..} |
            PageType::Success{..} => None,
            PageType::Display{next, ..} |
            PageType::Input{next, ..} |
            PageType::Form{next, ..} |
            PageType::Both{next, ..} |
            PageType::Review{next, ..} => Some(next),
        }
    }

    pub fn length(&mut self) -> Option<&mut usize> {
        match self {
            PageType::Profile{..} |
            PageType::Messaging{..} |
            PageType::Root{..} => None,
            PageType::Edit{flow_len, ..} |
            PageType::Display{flow_len, ..} |
            PageType::Input{flow_len, ..} |
            PageType::Form{flow_len, ..} |
            PageType::Success{flow_len, ..} |
            PageType::Both{flow_len, ..} |
            PageType::EditAndDisplay{flow_len, ..} |
            PageType::Review{flow_len, ..} => Some(flow_len)
        }
    }

    pub fn on_submit(&mut self) -> Option<&mut Box<dyn FormSubmit>> {
        match self {
            PageType::Edit{on_save, ..} => Some(on_save),
            PageType::Form{on_submit, ..} => on_submit.as_mut(),
            PageType::Review{on_submit, ..} => Some(on_submit),
            PageType::Success{on_submit, ..} => on_submit.as_mut(),
            _ => None
        }
    }

    pub fn build(&self, ctx: &mut Context, theme: &Theme) -> Box<dyn AppPage> {
        match self {
            PageType::Root{title, input, display, header, bumper_a, bumper_b} => Box::new(RootPage::new(theme, title.to_string(), input.to_vec(), display.to_vec(), header.clone(), bumper_a.clone(), bumper_b.clone())),
            PageType::Both{title, inputs, display, header, bumper, next, flow_len} => Box::new(StackPage::both(ctx, theme, title.to_string(), display.to_vec(), inputs.to_vec(), header.clone(), bumper.clone(), next.clone(), *flow_len)),
            PageType::Display{title, items, offset, header, bumper, next, flow_len} => Box::new(StackPage::display(ctx, theme, title.to_string(), items.to_vec(), *offset, header.clone(), bumper.clone(), next.clone(), *flow_len)),
            PageType::Input{title, item, header, bumper, next, flow_len} => Box::new(StackPage::input(ctx, theme, title.to_string(), item.clone(), header.clone(), bumper.clone(), next.clone(), *flow_len)),
            PageType::Form{title, item, next, flow_len, validate, on_submit} => Box::new(FormPage::new(theme, title.to_string(), item.clone(), next.clone(), *flow_len, validate.clone(), on_submit.clone())),
            PageType::Edit{title, input, display, validations, on_save, flow_len: _} => Box::new(EditPage::new(theme, title.to_string(), input.clone(), display.clone(), validations.clone(), on_save.clone())),
            PageType::EditAndDisplay{title, items, display, on_save, flow_len: _} => Box::new(EditPage::edit_and_display(theme, title.to_string(), items.clone(), display.clone(), on_save.clone())),
            PageType::Review{title, getter, next, flow_len, on_submit} => Box::new(ReviewPage::new(theme, title.to_string(), getter.clone(), next.clone(), *flow_len, on_submit.clone())),
            PageType::Success{title, getter, flow_len, on_submit} => Box::new(SuccessPage::new(theme, title.to_string(), getter.clone(), *flow_len, on_submit.clone())),
            PageType::Messaging{room, flow_len} => Box::new(MessagesPage::new(ctx, theme, room.clone(), *flow_len)),
            PageType::Profile{profile} => ProfilePage::new(ctx, theme, profile.clone())
        }
    }

    pub fn build_root(&self, ctx: &mut Context, theme: &Theme) -> Box<dyn AppPage> {
        match self {
            PageType::Root{title, input, display, header, bumper_a, bumper_b} => Box::new(RootPage::new(theme, title.to_string(), input.to_vec(), display.to_vec(), header.clone(), bumper_a.clone(), bumper_b.clone())),
            PageType::Both{title, inputs, display, header, bumper: _, next: _, flow_len: _} => Box::new(RootPage::new(theme, title.to_string(), inputs.to_vec(), display.to_vec(), header.clone(), None, None)),
            PageType::Display{title, items, offset: _, header, bumper: _, next: _, flow_len: _} => Box::new(RootPage::new(theme, title.to_string(), vec![], items.to_vec(), header.clone(), None, None)),
            PageType::Input{title, item, header, bumper: _, next: _, flow_len: _} => Box::new(RootPage::new(theme, title.to_string(), vec![item.clone()], vec![], header.clone(), None, None)),

            // PageType::Profile{..} |
            PageType::Edit{..} |
            PageType::Form{..} |
            PageType::Review{..} |
            PageType::Success{..} |
            PageType::Messaging{..} => panic!("Not an accepted root type"),

            PageType::Profile{profile} => ProfilePage::new(ctx, theme, profile.clone()),
            PageType::EditAndDisplay{title, items, display, on_save, flow_len: _} => Box::new(EditPage::root(theme, title.to_string(), items.clone(), display.clone(), on_save.clone())),
        }
    }
}


#[derive(Debug, Clone)]
pub enum Bumper {
    Default,
    Custom { label: String, action: Action, secondary: Option<(String, Action)>},
    Done,
    None,
}

impl Bumper {
    pub fn custom(label: &str, action: Action) -> Self {
        Bumper::Custom {label: label.to_string(), action, secondary: None}
    }

    pub fn double(l1: &str, a1: Action, l2: &str, a2: Action) -> Self {
        Bumper::Custom {label: l1.to_string(), action: a1, secondary: Some((l2.to_string(), a2))}
    }
}


// #[derive(Debug, Component, Clone)]
// pub struct ScanQRCodePage(Stack, PelicanPage);
// impl OnEvent for ScanQRCodePage {}
// impl AppPage for ScanQRCodePage {}
// impl ScanQRCodePage {
//     pub fn new(theme: &Theme) -> Self {
//         let header = Header::stack(theme, "Scan QR code", None);

//         let page = PelicanPage::new(
//             header, 
//             Content::new(Offset::Center, drawables![
//                 QRCodeScanner::new(theme, Box::new(|ctx: &mut Context, val: String| {
//                     ctx.send(Request::event(NavigationEvent::Pop));
//                 })),
//             ], Box::new(|_| true)), 
//             None
//         );

//         ScanQRCodePage(Stack::default(), page)
//     }
// }


// let img = Listener::new(ctx, theme, img, |ctx: &mut Context, theme: &Theme, img: &mut Image, state: StateTest| {
//     let image: Arc<RgbaImage> = Arc::new(image::open(&format!("./{}", state.0.to_string())).unwrap().into());
//     *img = Image{shape: ShapeType::Rectangle(0.0, (1448.0/6.0, 1904.0/6.0), 0.0), image: image.clone(), color: None};
// });

// Listener::new(ctx, theme, page, |ctx: &mut Context, theme: &Theme, group: &mut MessageGroup, messages: Vec<Message>| {
//     *group = MessageGroup::new(theme, messages);
// })