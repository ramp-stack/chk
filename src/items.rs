use pelican_ui::{drawables, colors, Context, Callback, Request, Hardware};
use pelican_ui::drawable::Drawable;
use pelican_ui::canvas::{Align, RgbaImage, ShapeType, Image};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::utils::{TitleSubtitle};
use pelican_ui::components::list_item::{ListItemSection, ListItemInfoLeft, ListItem as PelicanListItem};
use pelican_ui::components::{TextInput, RadioSelector, Icon, DataItem, QRCode, NumericalInput};
use pelican_ui::components::text::{ExpandableText, TextStyle, TextSize};
use pelican_ui::components::avatar::{Avatar, AvatarSize};
pub use pelican_ui::components::avatar::{AvatarContent, AvatarIconStyle};
use pelican_ui::components::button::{SecondaryButton, QuickActions};
use pelican_ui::components::SearchBar;

use std::sync::Arc;

use crate::flow::{Flow, State};

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Text {label: String, actions: Option<Vec<(String, Icons, Action)>>, show_label: bool, preset: Option<String>},
    Currency {instructions: String}, //, on_edited: Box<dyn EditedFn>},
    Date {instructions: String}, //, on_edited: Box<dyn EditedFn>},
    Time {instructions: String}, //, on_edited: Box<dyn EditedFn>},
    Enumerator {items: Vec<EnumItem>},
    Avatar {content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>},
    Search {items: Vec<ListItem>},
}

impl Input {
    pub fn currency(instructions: &str) -> Self { //}, on_edited: impl FnMut(&mut Context, &mut String) + Clone + 'static) -> Self {
        Input::Currency {instructions: instructions.to_string()} //, on_edited: Box::new(on_edited)}
    }

    pub fn date(instructions: &str) -> Self { //, on_edited: impl FnMut(&mut Context, &mut String) + Clone + 'static) -> Self {
        Input::Date {instructions: instructions.to_string()} //, on_edited: Box::new(on_edited)}
    }

    pub fn time(instructions: &str) -> Self { //, on_edited: impl FnMut(&mut Context, &mut String) + Clone + 'static) -> Self {
        Input::Time {instructions: instructions.to_string()} //, on_edited: Box::new(on_edited)}
    }

    pub fn enumerator(items: Vec<EnumItem>) -> Self {
        Input::Enumerator {items}
    }

    pub fn text(label: &str, show_label: bool, preset: Option<String>, actions: Option<Vec<(String, Icons, Action)>>) -> Self {
        Input::Text {label: label.to_string(), show_label, preset, actions}
    }

    pub fn avatar(content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>) -> Self {
        Input::Avatar {content, flair, action}
    }

    pub fn search(items: Vec<ListItem>) -> Self {
        Input::Search {items}
    }

    pub fn build(&self, theme: &Theme) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Input::Text {show_label, label, preset, actions} => {
                let mut items = drawables![TextInput::new(theme, preset.as_deref(), show_label.then_some(label), Some(&format!("Enter {}...", label.to_lowercase())), None, None)];
                actions.as_ref().map(|a| items.push(Box::new(QuickActions::new(theme, a.into_iter().map(|(label, icon, action)| {
                    let action: Box<dyn Callback> = action.get();
                    (label.to_string(), *icon, action)
                }).collect::<Vec<(String, Icons, Box<dyn Callback>)>>()))));

                items
            },
            Input::Enumerator {items} => drawables![RadioSelector::new(theme, 0, items.iter().map(|item| item.get()).collect::<Vec<_>>())],
            Input::Currency {instructions} => drawables![NumericalInput::numerical(theme, instructions)],
            Input::Date {instructions} => drawables![NumericalInput::date(theme, instructions)],
            Input::Time {instructions} => drawables![NumericalInput::time(theme, instructions)],
            Input::Avatar {content, flair, action} => drawables![Avatar::new(theme, content.clone(), *flair, flair.is_some(), AvatarSize::Xxl, action.as_ref().map(|a| a.get()))],
            Input::Search {items} => drawables![SearchBar::new(theme, items.iter().map(|item| item.build(theme)).collect::<Vec<_>>())]
        })
    }

    #[allow(clippy::borrowed_box)]
    pub fn store_in(child: &Box<dyn Drawable>, state: &mut Vec<State>) {
        if let Some(input) = child.downcast_ref::<TextInput>() {
            state.push(State::Text(input.value()));
        } else if let Some(selector) = child.downcast_ref::<RadioSelector>() {
            state.push(State::Enumerator(selector.value()));
        } else if let Some(input) = child.downcast_ref::<NumericalInput>() {
            state.push(State::Number(input.value()));
        } else if let Some(avatar) = child.downcast_ref::<Avatar>() {
            state.push(State::Avatar(avatar.content.clone()))
        } else if let Some(searchbar) = child.downcast_ref::<SearchBar>() {
            state.push(State::Search(searchbar.results()))
        }
    }
}

#[derive(Debug, Clone)]
pub enum Display {
    Text {text: String, size: TextSize, style: TextStyle, align: Align},
    Icon {icon: Icons},
    Image {image: Arc<RgbaImage>, size: (f32, f32)},
    Review {label: String, data: String, instructions: String},
    Table {label: String, items: Vec<TableItem>},
    Currency {amount: f32, instructions: String},
    List {label: Option<String>, items: Vec<ListItem>, instructions: Option<String>},
    QRCode {data: String, instructions: String},
    Avatar {content: AvatarContent},
    Actions {actions: Vec<ActionItem>}
}

impl Display {
    pub fn text(text: &str) -> Self {
        Display::Text {text: text.to_string(), size: TextSize::Md, style: TextStyle::Primary, align: Align::Left}
    }

    pub fn instructions(text: &str) -> Self {
        Display::Text {text: text.to_string(), size: TextSize::Md, style: TextStyle::Secondary, align: Align::Center}
    }

    pub fn label(text: &str) -> Self {
        Display::Text {text: text.to_string(), size: TextSize::H5, style: TextStyle::Heading, align: Align::Left}
    }

    pub fn icon(icon: Icons) -> Self {
        Display::Icon {icon}
    }

    pub fn image(image: Arc<RgbaImage>, size: (f32, f32)) -> Self {
        Display::Image {image, size}
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

    pub fn list(label: Option<&str>, items: Vec<ListItem>, instructions: Option<&str>) -> Self {
        Display::List{label: label.map(|i| i.to_string()), items, instructions: instructions.map(|i| i.to_string())}
    }

    pub fn currency(amount: f32, instructions: &str) -> Self {
        Display::Currency {amount, instructions: instructions.to_string()}
    }

    pub fn avatar(content: AvatarContent) -> Self {
        Display::Avatar {content}
    }

    pub fn actions(actions: Vec<ActionItem>) -> Self {
        Display::Actions {actions}
    }

    pub fn build(&mut self, theme: &Theme) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Display::Icon {icon} => drawables![Icon::new(theme, *icon, Some(theme.colors().get(colors::Text::Heading)), 128.0)],
            Display::Image {image, size} => drawables![Image{shape: ShapeType::Rectangle(0.0, *size, 0.0), image: image.clone(), color: None}],
            Display::Text {text, size, style, align} if !text.is_empty() => drawables![ExpandableText::new(theme, text, *size, *style, *align, None)],
            Display::Review {label, data, instructions} => drawables![DataItem::text(theme, label, data, instructions, Some(Vec::<(String, Icons, Box<dyn Callback>)>::new()))],
            Display::Table {label, items} => drawables![DataItem::table(theme, label, items.iter().map(|TableItem{title, data}| (title.clone(), data.clone())).collect(), Some(Vec::<(String, Icons, Box<dyn Callback>)>::new()))],
            Display::Currency {amount, instructions} => drawables![NumericalInput::display(theme, *amount, instructions)],
            Display::List {items, instructions, ..} if items.is_empty() => drawables![ExpandableText::new(theme, instructions.as_ref()?, TextSize::Md, TextStyle::Secondary, Align::Center, None)],
            Display::List {label, items, ..} => drawables![ListItemSection::new(theme, label.clone(), items.iter_mut().map(|item| item.build(theme)).collect::<Vec<_>>())],
            Display::QRCode {data, instructions} => drawables![QRCode::new(theme, data), ExpandableText::new(theme, instructions, TextSize::Md, TextStyle::Secondary, Align::Center, None)],
            Display::Avatar {content} => drawables![Avatar::new(theme, content.clone(), None, false, AvatarSize::Xxl, None)],
            Display::Actions {actions} => actions.iter_mut().map(|ActionItem(a, l, i)| Box::new(SecondaryButton::medium(theme, *i, l, None, a.get())) as Box<dyn Drawable>).collect::<Vec<_>>(),
            _ => return None
        })
    }
}

#[derive(Debug, Clone)]
pub struct ListItem {
    avatar: Option<AvatarContent>, 
    title: String, 
    subtitle: String, 
    secondary: Option<String>, 
    flow: Option<Flow>, 
}

impl PartialEq for ListItem {
    fn eq(&self, other: &Self) -> bool {
        self.avatar == other.avatar &&
        self.title == other.title &&
        self.subtitle == other.subtitle &&
        self.secondary == other.secondary
    }
}

impl ListItem {
    pub fn plain(title: &str, subtitle: &str, secondary: Option<&str>, flow: Option<Flow>) -> Self {
        ListItem {
            avatar: None,
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()),
            flow,
        }
    }

    pub fn avatar(avatar: AvatarContent, title: &str, subtitle: &str, secondary: Option<&str>, flow: Option<Flow>) -> Self {
        ListItem {
            avatar: Some(avatar),
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            secondary: secondary.map(|s| s.to_string()),
            flow,
        }
    }

    pub(crate) fn build(&self, theme: &Theme) -> PelicanListItem {
        let ListItem {avatar, title, subtitle, secondary, flow} = self.clone();
        let has_flow = flow.is_some();
        let closure = Box::new(move |ctx: &mut Context, theme: &Theme| {
            // (on_click.clone())(ctx, theme);
            if let Some(mut f) = flow.clone() {(f.build(ctx))(ctx, theme);}
        });

        PelicanListItem::new(theme, avatar.clone(), 
            ListItemInfoLeft::new(&title, Some(&subtitle), None, None), 
            secondary.as_ref().map(|s| TitleSubtitle::new(s, Some("Details"))), 
            None, has_flow.then_some(Icons::Forward), closure
        )
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    Share {data: String},
    SelectImage,
    Custom {action: Box<dyn Callback>},
    None,
    Flow {flow: Flow},
    Paste,
    Copy {data: String}
    // Navigate {flow: Flow},
}

impl PartialEq for Action {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Action::Share { data: a }, Action::Share { data: b }) => a == b,
            (Action::Flow {..}, Action::Flow {..}) => true,
            (Action::SelectImage, Action::SelectImage) => true,
            (Action::None, Action::None) => true,
            _ => false,
        }
    }
}

impl Action {
    pub fn share(data: &str) -> Self {
        Action::Share {data: data.to_string()}
    }

    pub fn copy(data: &str) -> Self {
        Action::Copy {data: data.to_string()}
    }

    pub fn select_image() -> Self {
        Action::SelectImage
    }

    pub fn scan_qr() -> Self {
        // Action::Flow {
        //     flow: Flow::new()
        // }

        Action::None
    }

    pub fn custom(action: impl Callback + 'static) -> Self {
        Action::Custom {action: Box::new(action)}
    }

    pub fn flow(flow: Flow) -> Self {
        Action::Flow {flow}
    }

    // pub fn navigate(flow: Flow) -> Self {
    //     Action::Navigate {flow}
    // }

    pub fn get(&self) -> Box<dyn Callback> {
        match self {
            Action::Share {data} => {
                let share_data = data.clone();
                Box::new(move |_ctx: &mut Context, _: &Theme| println!("Sharing data {:?}", share_data.clone()))
            }

            Action::SelectImage => {
                Box::new(move |_ctx: &mut Context, _: &Theme| println!("Selecting image"))
            }

            Action::Custom {action} => {
                let mut action = action.clone();
                Box::new(move |ctx: &mut Context, theme: &Theme| (action)(ctx, theme))
            }

            Action::Flow {flow} => {
                let mut flow = flow.clone();
                Box::new(move |ctx: &mut Context, _: &Theme| {flow.build(ctx);})
            }

            Action::Paste => {
                Box::new(move |ctx: &mut Context, _: &Theme| {ctx.send(Request::Hardware(Hardware::GetClipboard))})
            }

            Action::Copy {data} => {
                let data = data.to_string();
                Box::new(move |ctx: &mut Context, _: &Theme| {ctx.send(Request::Hardware(Hardware::SetClipboard(data.to_string())))})
            }

            // Action::Navigate {flow} => flow.clone().build(),

            _ => Box::new(move |_ctx: &mut Context, _: &Theme| println!("Doing nothing here..."))
        }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct ActionItem(Action, String, Icons);
impl ActionItem {
    pub fn new(action: Action, label: &str, icon: Icons) -> Self {
        ActionItem(action, label.to_string(), icon)
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct TableItem {title: String, data: String}

impl TableItem {
    pub fn new(title: &str, data: &str) -> Self {
        TableItem { title: title.to_string(), data: data.to_string() }
    }
}

#[derive(Clone)]
pub struct EnumItem {title: String, data: String} //, callback: Box<dyn Callback>}
impl std::fmt::Debug for EnumItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnumItem").field("title", &self.title).field("data", &self.data).finish()
    }
}

impl EnumItem {
    pub fn new(title: &str, data: &str) -> Self { //, callback: impl FnMut(&mut Context) + Clone + 'static) -> Self {
        EnumItem {title: title.to_string(), data: data.to_string(), } //callback: Box::new(callback)}
    }

    fn get(&self) -> (&str, &str, Box<dyn Callback>) {
        // let mut callback = self.callback.clone();
        (&self.title as &str, &self.data as &str, Box::new(|_, _|{})) //Box::new(move |ctx: &mut Context| {(callback)(ctx)}) as Box<dyn FnMut(&mut Context)>)
    }
}

impl PartialEq for EnumItem {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title &&
        self.data == other.data
    }
}
