use pelican_ui::{drawables, Context};
use pelican_ui::drawable::{Drawable, Align};
use pelican_ui::utils::{Callback, TitleSubtitle};
use pelican_ui::components::list_item::{ListItemSection, ListItemInfoLeft, ListItem as PelicanListItem};
use pelican_ui::components::{Checkbox, CheckboxList, TextInput, RadioSelector, Icon, DataItem, QRCode, NumericalInput};
use pelican_ui::components::text::{ExpandableText, TextStyle, TextSize};
use pelican_ui::components::avatar::{Avatar, AvatarSize, AvatarContent, AvatarIconStyle};
use pelican_ui::plugin::PelicanUI;

use crate::pages::RootPage;
use crate::flow::Flow;

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum Input {
    Text {label: String, actions: Option<Vec<Action>>, tag: String, check: Box<dyn ValidityFn>},
    Currency {instructions: String, tag: String, check: Box<dyn ValidityFn>},
    Date {instructions: String, tag: String, check: Box<dyn ValidityFn>},
    Time {instructions: String, tag: String, check: Box<dyn ValidityFn>},
    Enumerator {items: Vec<EnumItem>, tag: String},
    Avatar {content: AvatarContent, flair: Option<(String, AvatarIconStyle)>, action: Option<Action>},
    Boolean {items: Vec<ChecklistItem>}
}

impl Input {
    pub fn currency(instructions: &str, tag: &str, check: impl FnMut(&mut Context) -> bool + Clone + 'static) -> Self {
        Input::Currency {instructions: instructions.to_string(), tag: tag.to_string(), check: Box::new(check)}
    }

    pub fn date(instructions: &str, tag: &str, check: impl FnMut(&mut Context) -> bool + Clone + 'static) -> Self {
        Input::Date {instructions: instructions.to_string(), tag: tag.to_string(), check: Box::new(check)}
    }

    pub fn time(instructions: &str, tag: &str, check: impl FnMut(&mut Context) -> bool + Clone + 'static) -> Self {
        Input::Time {instructions: instructions.to_string(), tag: tag.to_string(), check: Box::new(check)}
    }

    pub fn enumerator(items: Vec<EnumItem>, tag: &str) -> Self {
        Input::Enumerator {items, tag: tag.to_string()}
    }

    pub fn text(label: &str, actions: Option<Vec<Action>>, tag: &str, check: impl FnMut(&mut Context) -> bool + Clone + 'static) -> Self {
        Input::Text {label: label.to_string(), actions, tag: tag.to_string(), check: Box::new(check)}
    }

    pub fn avatar(content: AvatarContent, flair: Option<(String, AvatarIconStyle)>, action: Option<Action>) -> Self {
        Input::Avatar {content, flair, action}
    }

    pub fn checklist(items: Vec<ChecklistItem>) -> Self {
        Input::Boolean {items}
    }

    pub fn build(&self, ctx: &mut Context) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Input::Text {label, tag, ..} => drawables![TextInput::new(ctx, None, (label, false), Some(&format!("Enter {}...", label.to_lowercase())), None, None, tag)],
            Input::Enumerator {items, tag} => drawables![RadioSelector::new(ctx, 0, tag, items.iter().map(|item| item.get()).collect::<Vec<_>>())],
            Input::Currency {instructions, tag, ..} => drawables![NumericalInput::currency(ctx, instructions, tag)],
            Input::Date {instructions, tag, ..} => drawables![NumericalInput::date(ctx, instructions, tag)],
            Input::Time {instructions, tag, ..} => drawables![NumericalInput::time(ctx, instructions, tag)],
            Input::Avatar {content, flair, action} => drawables![Avatar::new(ctx, content.clone(), flair.clone(), flair.is_some(), AvatarSize::Xxl, action.as_ref().map(|a| a.get()))],
            Input::Boolean {items} => drawables![CheckboxList::new(items.iter().map(|item| item.get(ctx)).collect::<Vec<_>>())]
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
    Text {text: String, size: TextSize, style: TextStyle, align: Align},
    Icon {icon: String},
    Review {label: String, data: String, instructions: String},
    Table {label: String, items: Vec<TableItem>},
    Currency {amount: f32, instructions: String},
    List {label: Option<String>, items: Vec<ListItem>, flow: Option<Flow>, instructions: Option<String>},
    QRCode {data: String, instructions: String},
    Avatar {content: AvatarContent}
}

impl Display {
    pub fn instructions(text: &str) -> Self {
        Display::Text {text: text.to_string(), size: TextSize::Md, style: TextStyle::Secondary, align: Align::Center}
    }

    pub fn label(text: &str) -> Self {
        Display::Text {text: text.to_string(), size: TextSize::H5, style: TextStyle::Heading, align: Align::Left}
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

    pub fn list(label: Option<&str>, items: Vec<ListItem>, flow: Option<Flow>, instructions: Option<&str>) -> Self {
        Display::List{label: label.map(|i| i.to_string()), items, flow, instructions: instructions.map(|i| i.to_string())}
    }

    pub fn currency(amount: f32, instructions: &str) -> Self {
        Display::Currency {amount, instructions: instructions.to_string()}
    }

    pub fn avatar(content: AvatarContent) -> Self {
        Display::Avatar {content}
    }

    pub fn build(&mut self, ctx: &mut Context) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Display::Icon {icon} => {
                let color = ctx.get::<PelicanUI>().get().0.theme().colors.text.heading;
                drawables![Icon::new(ctx, icon, Some(color), 128.0)]
            }
            Display::Text {text, size, style, align} => drawables![ExpandableText::new(ctx, text, *size, *style, *align, None)],
            Display::Review {label, data, instructions} => drawables![DataItem::text(ctx, label, data, instructions, None)],
            Display::Table {label, items} => drawables![DataItem::table(ctx, label, items.iter().map(|TableItem{title, data}| (title.clone(), data.clone())).collect(), None)],
            Display::Currency {amount, instructions} => drawables![NumericalInput::display(ctx, *amount, instructions)],
            Display::List {items, instructions, ..} if items.is_empty() => drawables![ExpandableText::new(ctx, instructions.as_ref()?, TextSize::Md, TextStyle::Secondary, Align::Center, None)],
            Display::List {label, items, flow, ..} => {
                let mut list_items = Vec::new();

                match flow {
                    Some(flow_ref) => for item in items {
                        list_items.push(item.build(ctx, Some(flow_ref)));
                    },
                    None => for item in items {
                        list_items.push(item.build(ctx, None));
                    }
                }

                drawables![ListItemSection::new(ctx, label.clone(), list_items)]
            }
            Display::QRCode {data, instructions} => drawables![QRCode::new(ctx, data), ExpandableText::new(ctx, instructions, TextSize::Md, TextStyle::Secondary, Align::Center, None)],
            Display::Avatar {content} => drawables![Avatar::new(ctx, content.clone(), None, false, AvatarSize::Xxl, None)],
        })
    }
}

#[derive(Debug, Clone)]
pub struct ListItem {avatar: Option<AvatarContent>, title: String, subtitle: String, secondary: Option<String>}

impl ListItem {
    pub fn plain(title: &str, subtitle: &str, secondary: Option<&str>, _tag: &str) -> Self {
        ListItem {
            avatar: None,
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()),
        }
    }

    pub fn avatar(avatar: AvatarContent, title: &str, subtitle: &str, secondary: Option<&str>, _tag: &str) -> Self {
        ListItem {
            avatar: Some(avatar),
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()),
        }
    }

    pub(crate) fn build(&self, ctx: &mut Context, mut flow: Option<&mut Flow>) -> PelicanListItem {
        let ListItem {avatar, title, subtitle, secondary} = self;
        let closure = flow.as_mut().map(|f| f.build());

        PelicanListItem::new(ctx, avatar.clone(), 
            ListItemInfoLeft::new(title, Some(subtitle), None, None), 
            secondary.as_ref().map(|s| TitleSubtitle::new(s, Some("Details"))), 
            None, flow.is_some().then_some("forward"), 
            closure.unwrap_or(Box::new(|_ctx: &mut Context| {}))
        )
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    Share {data: String},
    SelectImage,
    Custom {action: Box<dyn FnMutClone>},
    None,
    Navigate {flow: Flow},
}

impl Action {
    pub fn share(data: &str) -> Self {
        Action::Share {data: data.to_string()}
    }

    pub fn select_image() -> Self {
        Action::SelectImage
    }

    pub fn custom(action: impl FnMutClone + 'static) -> Self {
        Action::Custom {action: Box::new(action)}
    }

    pub fn navigate(flow: Flow) -> Self {
        Action::Navigate {flow}
    }

    pub fn get(&self) -> Callback {
        match self {
            Action::Share {data} => {
                let share_data = data.clone();
                Box::new(move |_ctx: &mut Context| println!("Sharing data {:?}", share_data.clone()))
            }

            Action::SelectImage => {
                Box::new(move |_ctx: &mut Context| println!("Selecting image"))
            }

            Action::Custom {action} => {
                let mut action = action.clone();
                Box::new(move |ctx: &mut Context| (action)(ctx))
            }

            Action::Navigate {flow} => flow.clone().build(),

            _ => Box::new(move |_ctx: &mut Context| println!("Doing nothing here..."))
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

#[derive(Debug, Clone)]
pub struct ChecklistItem {title: String, subtitle: Option<String>, is_selected: bool}
impl ChecklistItem {
    pub fn new(title: &str, subtitle: Option<&str>, is_selected: bool) -> Self {
        ChecklistItem {title: title.to_string(), subtitle: subtitle.map(|s| s.to_string()), is_selected}
    }

    fn get(&self, ctx: &mut Context) -> Checkbox {
        Checkbox::new(ctx, &self.title, self.subtitle.clone(), self.is_selected, &self.title)
    }
}

pub type NavFn = Rc<RefCell<dyn FnMut(&mut Context)>>;

/// Content of a tab button: either an icon or an avatar.
#[derive(Debug, Clone)]
pub enum RootContent {
    Icon(String),
    Avatar(AvatarContent),
}

impl RootContent {
    pub fn icon(icon: &str) -> Self {
        RootContent::Icon(icon.to_string())
    }
    
    pub fn avatar(content: AvatarContent) -> Self {
        RootContent::Avatar(content)
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