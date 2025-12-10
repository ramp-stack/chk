use pelican_ui::{drawables, Context};
use pelican_ui::drawable::{Color, Drawable, Align};
use pelican_ui::resources::Image;
use pelican_ui::utils::{Callback, TitleSubtitle};
use pelican_ui::components::list_item::{ListItemGroup, ListItemInfoLeft, ListItem as PelicanListItem};
use pelican_ui::components::{TextInput, RadioSelector, Icon, DataItem, QRCode, NumericalInput};
use pelican_ui::components::text::{ExpandableText, TextStyle, TextSize};

use crate::pages::RootPage;
use crate::flow::Flow;

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum Input {
    Text {label: String, actions: Option<Vec<Action>>, tag: String, check: Box<dyn ValidityFn>},
    Currency {instructions: String, tag: String, check: Box<dyn ValidityFn>},
    Enumerator {items: Vec<EnumItem>, tag: String}
}

impl Input {
    pub fn currency(instructions: &str, tag: &str, check: impl FnMut(&mut Context) -> bool + Clone + 'static) -> Self {
        Input::Currency {instructions: instructions.to_string(), tag: tag.to_string(), check: Box::new(check)}
    }

    pub fn enumerator(items: Vec<EnumItem>, tag: &str) -> Self {
        Input::Enumerator {items, tag: tag.to_string()}
    }

    pub fn text(label: &str, actions: Option<Vec<Action>>, tag: &str, check: impl FnMut(&mut Context) -> bool + Clone + 'static) -> Self {
        Input::Text {label: label.to_string(), actions, tag: tag.to_string(), check: Box::new(check)}
    }

    pub fn build(&self, ctx: &mut Context) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Input::Text {label, tag, ..} => drawables![TextInput::new(ctx, None, (label, false), Some(&format!("Enter {}...", label.to_lowercase())), None, None, tag)],
            Input::Enumerator {items, tag} => drawables![RadioSelector::new(ctx, 0, tag, items.iter().map(|item| item.get()).collect::<Vec<_>>())],
            Input::Currency {instructions, tag, ..} => drawables![NumericalInput::currency(ctx, instructions, tag)],
        })
    }

    pub fn check(&mut self) -> Option<Box<dyn ValidityFn>> {
        match self {
            Input::Text {check, ..} => Some(check.clone()),
            Input::Currency {check, ..} => Some(check.clone()),
            _ => None
        }
    }
}

#[derive(Debug, Clone)]
pub enum Display {
    Text {text: String},
    Icon {icon: String},
    Review {label: String, data: String, instructions: String},
    Table {label: String, items: Vec<TableItem>},
    Currency {amount: f32, instructions: String},
    List {items: Vec<ListItem>, flow: Option<Flow>, instructions: Option<String>},
    QRCode {data: String, instructions: String}
}

impl Display {
    pub fn text(text: &str) -> Self {
        Display::Text {text: text.to_string()}
    }

    pub fn icon(icon: &str) -> Self {
        Display::Icon {icon: icon.to_string()}
    }

    pub fn review(label: &str, data: &str, instructions: &str) -> Self {
        Display::Review {label: label.to_string(), data: data.to_string(), instructions: instructions.to_string()}
    }

    pub fn table(label: &str, items: Vec<TableItem>) -> Self {
        Display::Table {label: label.to_string(), items}
    }

    pub fn qr_code(data: &str, instructions: &str) -> Self {
        Display::QRCode {data: data.to_string(), instructions: instructions.to_string()}
    }

    pub fn list(items: Vec<ListItem>, flow: Option<Flow>, instructions: Option<&str>) -> Self {
        Display::List{items, flow, instructions: instructions.map(|i| i.to_string())}
    }

    pub fn currency(amount: f32, instructions: &str) -> Self {
        Display::Currency {amount, instructions: instructions.to_string()}
    }

    pub fn build(&mut self, ctx: &mut Context) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Display::Icon {icon} => drawables![Icon::new(ctx, icon, Some(Color::WHITE), 128.0)],
            Display::Text {text} => drawables![ExpandableText::new(ctx, text, TextSize::H4, TextStyle::Heading, Align::Center, None)],
            Display::Review {label, data, instructions} => drawables![DataItem::text(ctx, label, data, instructions, None)],
            Display::Table {label, items} => drawables![DataItem::table(ctx, label, items.iter().map(|TableItem{title, data}| (title.clone(), data.clone())).collect(), None)],
            Display::Currency {amount, instructions} => drawables![NumericalInput::display(ctx, *amount, instructions)],
            Display::List {items, instructions, ..} if items.is_empty() => drawables![ExpandableText::new(ctx, instructions.as_ref()?, TextSize::Md, TextStyle::Secondary, Align::Center, None)],
            Display::List {items, flow, ..} => {
                let mut list_items = Vec::new();

                match flow {
                    Some(flow_ref) => for item in items {
                        list_items.push(item.build(ctx, Some(flow_ref)));
                    },
                    None => for item in items {
                        list_items.push(item.build(ctx, None));
                    }
                }

                drawables![ListItemGroup::new(list_items)]
            }
            Display::QRCode {data, instructions} => drawables![QRCode::new(ctx, data), ExpandableText::new(ctx, instructions, TextSize::Md, TextStyle::Secondary, Align::Center, None)],
        })
    }
}


#[derive(Debug, Clone)]
pub struct ListItem {title: String, subtitle: String, secondary: Option<String>}

impl ListItem {
    pub fn new(title: &str, subtitle: &str, secondary: Option<&str>, _tag: &str) -> Self {
        ListItem {
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()),
        }
    }

    pub fn build(&self, ctx: &mut Context, mut flow: Option<&mut Flow>) -> PelicanListItem {
        let ListItem {title, subtitle, secondary} = self;
        let closure = flow.as_mut().map(|f| f.build());

        PelicanListItem::new(ctx, None, 
            ListItemInfoLeft::new(title, Some(subtitle), None, None), 
            secondary.as_ref().map(|s| TitleSubtitle::new(s, Some("Details"))), 
            None, flow.is_some().then_some("forward"), 
            closure.unwrap_or(Box::new(|_ctx: &mut Context| {}))
        )
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    Share {data: String}
}

impl Action {
    pub fn share(data: &str) -> Self {
        Action::Share {data: data.to_string()}
    }

    pub fn get(&self) -> Callback {
        match self {
            Action::Share {data} => {
                let share_data = data.clone();
                Box::new(move |_ctx: &mut Context| println!("Sharing data {:?}", share_data.clone()))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TableItem {title: String, data: String}

impl TableItem {
    pub fn new(title: &str, data: &str) -> Self {
        TableItem { title: title.to_string(), data: data.to_string() }
    }
}

#[derive(Debug, Clone)]
pub struct EnumItem {title: String, data: String}
impl EnumItem {
    pub fn new(title: &str, data: &str) -> Self {
        EnumItem {title: title.to_string(), data: data.to_string()}
    }

    fn get(&self) -> (&str, &str, Callback) {
        (&self.title as &str, &self.data as &str, Box::new(|_ctx: &mut Context| {}) as Box<dyn FnMut(&mut Context)>)
    }
}

pub type NavFn = Rc<RefCell<dyn FnMut(&mut Context)>>;

/// Content of a tab button: either an icon or an avatar.
#[derive(Debug, Clone)]
pub enum RootContent {
    Icon(String),
    Avatar(Image),
}

impl RootContent {
    pub fn icon(icon: &str) -> Self {
        RootContent::Icon(icon.to_string())
    }
    
    pub fn avatar(image: Image) -> Self {
        RootContent::Avatar(image)
    }
}

/// Represents a tab root with its content and associated page.
#[derive(Debug, Clone)]
pub struct Root {
    pub content: RootContent,
    pub page: RootPage,
}

impl Root {
    pub fn new(content: RootContent, page: RootPage) -> Self {
        Root {content, page}
    }
}

#[derive(Debug, Clone)]
pub(crate) struct IconButton(String, Box<dyn FnMutClone + 'static>);
impl IconButton {
    pub(crate) fn get(&self) -> (String, Callback) { 
        let closure = self.1.clone_box();
        (self.0.to_string(), Box::new(move |ctx: &mut Context| (closure.clone_box())(ctx))) 
    }

    pub(crate) fn from(this: (&str, Box<dyn FnMutClone>)) -> Self {
        IconButton(this.0.to_string(), this.1)
    }
}

pub trait FnMutClone: FnMut(&mut Context) + 'static {
    fn clone_box(&self) -> Box<dyn FnMutClone>;
}

impl<F> FnMutClone for F where F: FnMut(&mut Context) + Clone + 'static {
    fn clone_box(&self) -> Box<dyn FnMutClone> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn FnMutClone> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn FnMutClone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Clonable Closure")
    }
}

pub trait ValidityFn: FnMut(&mut Context) -> bool + 'static {
    fn clone_box(&self) -> Box<dyn ValidityFn>;
}

impl<F> ValidityFn for F where F: FnMut(&mut Context) -> bool + Clone + 'static {
    fn clone_box(&self) -> Box<dyn ValidityFn> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn ValidityFn> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn ValidityFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Valitidy check...")
    }
}
