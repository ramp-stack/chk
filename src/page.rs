use ramp::prism;
use pelican_ui::event::OnEvent;
use pelican_ui::drawable::{Component, Drawable};
use pelican_ui::{Request, drawables, Context, Callback};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::canvas::Align;
use pelican_ui::components::TextInput;
use pelican_ui::components::avatar::{Avatar, AvatarSize};
use pelican_ui::components::list_item::{ListItemGroup, ListItem, ListItemInfoLeft};
use pelican_ui::navigation::NavigationEvent;
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::{AppPage, Flow as PelicanFlow};
use pelican_ui::components::text::{ExpandableText, TextSize, TextStyle};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::components::MessageGroups;
pub use pelican_ui::components::{Profile, Message};
use std::fmt::Debug;
use pelican_ui::utils::ValidationFn;

use crate::FlowWrapper;
use crate::flow::{Flow, State};
use crate::items::{Action, Input, Display};
use crate::closure::{FormSubmit, NavFn, ScreenBuilder, PageBuilder, ReviewItemGetter, SuccessGetter};

pub struct Root;
impl Root {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(title: &str, items: Vec<Display>, header: Option<(Icons, Flow)>, bumper_a: (String, Flow), bumper_b: Option<(String, Flow)>) -> PageType {
        PageType::root(title, items, header, bumper_a, bumper_b)
    }
}

#[derive(Debug, Component, Clone)]
pub struct Screen(Stack, pub Box<dyn AppPage>, #[skip] Box<dyn PageBuilder>, #[skip] Option<NavFn>, #[skip] usize, #[skip] Theme);
impl AppPage for Screen {}
impl OnEvent for Screen {}

impl Screen {
    pub fn new(ctx: &mut Context, theme: &Theme, mut page_builder: Box<dyn PageBuilder>) -> Self {
        Screen(Stack::default(), ((page_builder)(theme)).build(ctx, theme), page_builder, None, 1, theme.clone())
    }

    pub fn update(&mut self, ctx: &mut Context, new_len: usize, new_fn: Option<NavFn>) {
        let theme = &self.5;
        self.3 = new_fn.clone();
        self.4 = new_len;
        let mut page_type = (self.2)(theme);
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
    Root {title: String, items: Vec<Display>, header: Option<(Icons, Flow)>, bumper_a: (String, Flow), bumper_b: Option<(String, Flow)>},
    Display{title: String, items: Vec<Display>, offset: Offset, header: Option<(Icons, Flow)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize},
    Input{title: String, item: Input, header: Option<(Icons, Flow)>, bumper: Bumper, flow_len: usize, next: Option<NavFn>},
    Form{title: String, item: Input, flow_len: usize, next: Option<NavFn>, validate: Box<dyn ValidationFn>, on_submit: Option<Box<dyn FormSubmit>>},
    Review{title: String, getter: Box<dyn ReviewItemGetter>, flow_len: usize, next: Option<NavFn>, on_submit: Box<dyn FormSubmit>},
    Success{title: String, getter: Box<dyn SuccessGetter>, flow_len: usize},
    Messaging{messages: Vec<Message>, profiles: Vec<Profile>, flow_len: usize},
}

impl PageType {
    pub fn root(title: &str, items: Vec<Display>, header: Option<(Icons, Flow)>, bumper_a: (String, Flow), bumper_b: Option<(String, Flow)>) -> Self {
        PageType::Root { title: title.to_string(), items, header, bumper_a, bumper_b }
    }

    pub fn display(title: &str, items: Vec<Display>, header: Option<(Icons, Flow)>, bumper: Bumper, offset: Offset) -> Self {
        PageType::Display { title: title.to_string(), items, header, bumper, offset, flow_len: 1, next: None }
    }

    pub fn input(title: &str, item: Input, header: Option<(Icons, Flow)>, bumper: Bumper) -> Self {
        PageType::Input { title: title.to_string(), item, header, bumper, flow_len: 1, next: None }
    }

    pub fn form(title: &str, item: Input, validate: Box<dyn ValidationFn>, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        PageType::Form { title: title.to_string(), validate, item, flow_len: 1, next: None, on_submit }
    }

    pub fn review(title: &str, getter: Box<dyn ReviewItemGetter>, on_submit: Box<dyn FormSubmit>) -> Self {
        PageType::Review { title: title.to_string(), getter, flow_len: 1, next: None, on_submit }
    }

    pub fn success(title: &str, getter: Box<dyn SuccessGetter>) -> Self {
        PageType::Success { title: title.to_string(), getter, flow_len: 1 }
    }

    pub fn messaging(messages: Vec<Message>, profiles: Vec<Profile>) -> Self {
        PageType::Messaging{ messages, profiles, flow_len: 1 }
    }

    pub fn nav_fn(&mut self) -> Option<&mut Option<NavFn>> {
        match self {
            PageType::Root{..} |
            PageType::Messaging{..} |
            PageType::Success{..} => None,
            PageType::Display{next, ..} |
            PageType::Input{next, ..} |
            PageType::Form{next, ..} |
            PageType::Review{next, ..} => Some(next),
        }
    }

    pub fn length(&mut self) -> Option<&mut usize> {
        match self {
            PageType::Messaging{..} |
            PageType::Root{..} => None,
            PageType::Display{flow_len, ..} |
            PageType::Input{flow_len, ..} |
            PageType::Form{flow_len, ..} |
            PageType::Success{flow_len, ..} |
            PageType::Review{flow_len, ..} => Some(flow_len)
        }
    }

    pub fn on_submit(&mut self) -> Option<&mut Box<dyn FormSubmit>> {
        match self {
            PageType::Form{on_submit, ..} => on_submit.as_mut(),
            PageType::Review{on_submit, ..} => Some(on_submit),
            _ => None
        }
    }

    pub fn build(&self, ctx: &mut Context, theme: &Theme) -> Box<dyn AppPage> {
        match self {
            PageType::Root{title, items, header, bumper_a, bumper_b} => Box::new(RootPage::new(theme, title.to_string(), items.to_vec(), header.clone(), bumper_a.clone(), bumper_b.clone())),
            PageType::Display{title, items, offset, header, bumper, next, flow_len} => Box::new(StackPage::display(ctx, theme, title.to_string(), items.to_vec(), *offset, header.clone(), bumper.clone(), next.clone(), *flow_len)),
            PageType::Input{title, item, header, bumper, next, flow_len} => Box::new(StackPage::input(ctx, theme, title.to_string(), item.clone(), header.clone(), bumper.clone(), next.clone(), *flow_len)),
            PageType::Form{title, item, next, flow_len, validate, on_submit} => Box::new(FormPage::new(theme, title.to_string(), item.clone(), next.clone(), *flow_len, validate.clone(), on_submit.clone())),
            PageType::Review{title, getter, next, flow_len, on_submit} => Box::new(ReviewPage::new(theme, title.to_string(), getter.clone(), next.clone(), *flow_len, on_submit.clone())),
            PageType::Success{title, getter, flow_len} => Box::new(SuccessPage::new(theme, title.to_string(), getter.clone(), *flow_len)),
            PageType::Messaging{messages, profiles, flow_len} => Box::new(MessagesPage::new(ctx, theme, messages.clone(), profiles.clone(), *flow_len))
        }
    }
}

#[derive(Debug, Component, Clone)]
pub struct RootPage(Stack, PelicanPage);
impl OnEvent for RootPage {}
impl AppPage for RootPage {}
impl RootPage {
    pub fn new(theme: &Theme, title: String, items: Vec<Display>, header: Option<(Icons, Flow)>, bumper_a: (String, Flow), mut bumper_b: Option<(String, Flow)>) -> Self {
        let header_icon = header.map(|(s, flow)| {
            let mut flow = flow.clone();
            (s, Box::new(move |ctx: &mut Context, theme: &Theme| (flow.build(ctx))(ctx, theme)) as Box<dyn Callback>) 
        });

        let header = Header::home(theme, &title, header_icon);
        let content = items.clone().iter_mut().filter_map(|di| di.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        let second = bumper_b.as_mut().map(|(t, flow)| {
            let mut flow = flow.clone();
            (t.to_string(), Box::new(move |ctx: &mut Context, theme: &Theme| (flow.build(ctx))(ctx, theme)) as Box<dyn Callback>)
        });

        let (title, mut flow) = bumper_a.clone();
        let first = (title.to_string(), Box::new(move |ctx: &mut Context, theme: &Theme| (flow.build(ctx))(ctx, theme)) as Box<dyn Callback>);
        let bumper = PelicanBumper::home(theme, first, second);

        let offset = match items.first() {
            Some(Display::List {items, ..}) if items.is_empty() => Offset::Center,
            Some(Display::List {..}) => Offset::Start,
            _ if content.len() <= 1 => Offset::Center,
            _ => Offset::Start,
        };

        let page = PelicanPage::new(header, Content::new(offset, content, Box::new(|_| true)), Some(bumper));
        RootPage(Stack::default(), page)
    }
}

#[derive(Debug, Component, Clone)]
pub struct StackPage(Stack, PelicanPage);
impl OnEvent for StackPage {}
impl AppPage for StackPage {}
impl StackPage {
    #[allow(clippy::too_many_arguments)]
    pub fn display(ctx: &mut Context, theme: &Theme, title: String, items: Vec<Display>, offset: Offset, header: Option<(Icons, Flow)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize) -> Self {
        let items = items.into_iter().filter_map(|mut di| di.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        StackPage::new(ctx, theme, title, items, offset, header, bumper, next, flow_len)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn input(ctx: &mut Context, theme: &Theme, title: String, item: Input, header: Option<(Icons, Flow)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize) -> Self {
        let offset = item.offset();
        let item = item.build(theme).into_iter().flatten().collect();
        StackPage::new(ctx, theme, title, item, offset, header, bumper, next, flow_len)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(ctx: &mut Context, theme: &Theme, title: String, items: Vec<Box<dyn Drawable>>, offset: Offset, header: Option<(Icons, Flow)>, bumper: Bumper, next: Option<NavFn>, flow_len: usize) -> Self {
        let icon = header.map(|(i, mut f)| (i, f.build(ctx)));
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

        let page = PelicanPage::new(header, Content::new(offset, items, Box::new(|_| true)), bumper);
        StackPage(Stack::default(), page)
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

#[derive(Debug, Component, Clone)]
pub struct FormPage(Stack, pub PelicanPage, #[skip] Theme, #[skip] Option<NavFn>, #[skip] Option<Box<dyn FormSubmit>>, #[skip] bool);
impl OnEvent for FormPage {}
impl AppPage for FormPage {}
impl FormPage {
    pub fn new(theme: &Theme, title: String, item: Input, next: Option<NavFn>, _flow_len: usize, validate: Box<dyn ValidationFn>, on_submit: Option<Box<dyn FormSubmit>>) -> Self {
        let header = Header::stack(theme, &title, None);
        let content = item.build(theme).unwrap_or_default();
        let bumper = PelicanBumper::stack(theme, None, Box::new(|_: &mut Context, _: &Theme| {}), None);
        let page = PelicanPage::new(header, Content::new(Offset::Start, content, validate), Some(bumper));

        FormPage(Stack::default(), page, theme.clone(), next.clone(), on_submit.clone(), false)
    }

    pub fn on_change(&mut self, new: Vec<State>) {
        if !self.5 {
            self.5 = true;
            let theme = &self.2;
            let submit = self.4.clone();
            let closure: Box<dyn Callback> = match self.3.clone(){
                Some(nav) => Box::new(move |ctx: &mut Context, theme: &Theme| {
                    if let Some(mut on_submit) = submit.clone() {(on_submit)(ctx, &new);}
                    (nav.borrow_mut())(ctx, theme);
                }),
                None => Box::new(move |ctx: &mut Context, _theme: &Theme| {
                    if let Some(mut on_submit) = submit.clone() {(on_submit)(ctx, &new);}
                }),
            };
            self.1.bumper = Some(PelicanBumper::stack(theme, None, closure, None));
        }
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
        let page = PelicanPage::new(header, Content::new(Offset::Start, Vec::new(), Box::new(|_| true)), Some(bumper));
        ReviewPage(Stack::default(), page, item_getter, theme.clone(), next.clone(), on_submit.clone(), false)
    }

    pub fn on_change(&mut self, new: Vec<State>) {
        if !self.6 {
            self.6 = true;
            let theme = &self.3;
            let items = (self.2)(&new);
            let content = items.into_iter().filter_map(|mut i| i.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
            self.1.content = Content::new(Offset::Start, content, Box::new(|_| true));
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
pub struct SuccessPage(Stack, pub PelicanPage, #[skip] Box<dyn SuccessGetter>, #[skip] Theme, #[skip] bool);
impl OnEvent for SuccessPage {}
impl AppPage for SuccessPage {}
impl SuccessPage {
    pub fn new(theme: &Theme, title: String, getter: Box<dyn SuccessGetter>, flow_len: usize) -> Self {
        let header = Header::stack_end(theme, &title);
        let bumper = Some(PelicanBumper::stack_end(theme, Some(flow_len)));
        let page = PelicanPage::new(header, Content::new(Offset::Center, vec![], Box::new(|_| true)), bumper);
        SuccessPage(Stack::default(), page, getter, theme.clone(), false)
    }

    pub fn on_change(&mut self, new: Vec<State>) {
        if !self.4 {
            self.4 = true;
            use pelican_ui::colors;
            use pelican_ui::components::Icon;
            let theme = self.3.clone();
            let (icon, description) = (self.2)(new);
            self.1.content = Content::new(Offset::Center, drawables![
                Icon::new(&theme, icon, Some(theme.colors().get(colors::Text::Heading)), 128.0),
                ExpandableText::new(&theme, &description, TextSize::H4, TextStyle::Heading, Align::Center, None)
            ], Box::new(|_| true));
        }
    }
}

#[derive(Debug, Component, Clone)]
pub struct MessagesPage(Stack, PelicanPage);
impl OnEvent for MessagesPage {}
impl AppPage for MessagesPage {}
impl MessagesPage {
    pub fn new(_ctx: &mut Context, theme: &Theme, messages: Vec<Message>, profiles: Vec<Profile>, flow_len: usize) -> Self {
        let profiles: Vec<Profile> = profiles.into_iter().filter(|p| *p != Profile::me()).collect();
        let taken_profiles: Vec<Profile> = profiles.clone();
        let is_group = profiles.len() > 1;
        let header = Header::messaging(theme, profiles.clone(), flow_len, Box::new(FlowWrapper::new(PelicanFlow::new(vec![match is_group {
            true => Box::new(GroupMessageInfoPage::new(theme, taken_profiles.clone())),
            false => Box::new(ProfilePage::new(theme, taken_profiles[0].clone())),
        }]))));

        let bumper = Some(PelicanBumper::input(theme, "Message...", |_ctx: &mut Context, val: &mut String| {println!("Create Message From: {:?}", val)}));
        let messages = MessageGroups::new(theme, messages, profiles, false);
        let page = PelicanPage::new(header, Content::new(Offset::End, drawables![messages], Box::new(|_| true)), bumper);

        MessagesPage(Stack::default(), page)
    }
}

#[derive(Debug, Component, Clone)]
pub struct GroupMessageInfoPage(Stack, PelicanPage);
impl OnEvent for GroupMessageInfoPage {}
impl AppPage for GroupMessageInfoPage {}
impl GroupMessageInfoPage {
    pub fn new(theme: &Theme, profiles: Vec<Profile>) -> Self {
        let header = Header::stack(theme, "Group info", None);
        let profiles = ListItemGroup::new(profiles.into_iter().map(|p| ListItem::new(theme, Some(p.avatar()),
            ListItemInfoLeft::new(&p.name, Some("did::48anxiSatoETwhiLaceolduxWMoadoletaTawhoraldCCOdalotwevalouhEwBKONLAatHOHX"), None, None), 
            None, None, Some(Icons::Forward), Box::new(move |ctx: &mut Context, theme: &Theme| {
                let page: Box<dyn AppPage> = Box::new(ProfilePage::new(theme, p.clone()));
                let flow = FlowWrapper::new(PelicanFlow::new(vec![page]));
                ctx.send(Request::event(NavigationEvent::push(flow)));
            })
        )).collect());

        let page = PelicanPage::new(header, Content::new(Offset::Start, drawables![profiles], Box::new(|_| true)), None);
        GroupMessageInfoPage(Stack::default(), page)
    }
}

#[derive(Debug, Component, Clone)]
pub struct ProfilePage(Stack, PelicanPage);
impl OnEvent for ProfilePage {}
impl AppPage for ProfilePage {}
impl ProfilePage {
    pub fn new(theme: &Theme, profile: Profile) -> Self {
        let header = Header::stack(theme, &profile.name, None);

        let page = PelicanPage::new(
            header, 
            Content::new(Offset::Start, drawables![
                Avatar::new(theme, profile.avatar(), None, false, AvatarSize::Xxl, None),
                TextInput::default(theme),
                TextInput::default(theme)
            ], Box::new(|_| true)), 
            None
        );

        ProfilePage(Stack::default(), page)
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