use pelican_ui::{drawables, colors, Context, Callback, IS_MOBILE};
use pelican_ui::drawable::Drawable;
use pelican_ui::canvas::{Align, ShapeType, Image};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::layout::Offset;
use pelican_ui::utils::{TitleSubtitle};
use pelican_ui::components::list_item::{ListItemSection, ListItemInfoLeft, ListItem as PelicanListItem};
use pelican_ui::components::{QRCodeScanner, TextInput, RadioSelector, Icon, DataItem, QRCode, NumericalInput};
use pelican_ui::components::text::{ExpandableText, TextStyle, TextSize};
use pelican_ui::components::avatar::{Avatar, AvatarSize};
pub use pelican_ui::components::avatar::{AvatarContent, AvatarIconStyle};
use pelican_ui::components::button::{SecondaryButton, QuickActions, IconButtonGroup, ActionData};
use pelican_ui::components::{SearchBar, SearchbarEvent, Keypad, TextInputEvent};

use std::sync::Arc;
use air::names::Name;
use air::Instance;
use crate::profiles::Profile;

use crate::form::State;
use crate::flow::Flow;
use crate::ListItemGetter;

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Text {label: String, actions: Option<Vec<(String, Option<String>, Icons, Action)>>, show_label: bool, preset: Option<String>},
    Currency {instructions: String}, //, on_edited: Box<dyn EditedFn>},
    Date {instructions: String}, //, on_edited: Box<dyn EditedFn>},
    Time {instructions: String}, //, on_edited: Box<dyn EditedFn>},
    Enumerator {items: Vec<EnumItem>},
    Avatar {content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>},
    Search {items: Vec<(ListItem, Name)>, actions: Option<Vec<(String, Option<String>, Icons, Action)>>},
    QRCodeScanner {instructions: String, alt: Option<(String, Icons, Action)>},
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

    pub fn text(label: &str, show_label: bool, preset: Option<String>, actions: Option<Vec<(String, Option<String>, Icons, Action)>>) -> Self {
        Input::Text {label: label.to_string(), show_label, preset, actions}
    }

    pub fn avatar(content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>) -> Self {
        Input::Avatar {content, flair, action}
    }

    pub fn search(items: Vec<(ListItem, Name)>, actions: Option<Vec<(String, Option<String>, Icons, Action)>>) -> Self {
        Input::Search {items, actions}
    }

    pub fn qr_code_scanner(instructions: &str, alt: Option<(String, Icons, Action)>) -> Self {
        Input::QRCodeScanner {instructions: instructions.to_string(), alt}
    }

    pub fn build(&self, theme: &Theme) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Input::Text {show_label, label, preset, actions} => {
                let mut items = drawables![TextInput::new(theme, preset.as_deref(), show_label.then_some(label), Some(&format!("Enter {}...", label.to_lowercase())), None, None)];
                if let Some(a) = actions.as_ref()
                    && !a.is_empty() {
                        items.push(Box::new(QuickActions::new(theme, Offset::Start, a.iter().map(|(label, active, icon, action)| {
                            let on_click: Box<dyn Callback> = action.get();
                            ActionData { label: label.to_string(), active: active.clone(), icon: *icon, on_click }
                        }).collect::<Vec<ActionData>>())));
                    }

                items
            },
            Input::Enumerator {items} => drawables![RadioSelector::new(theme, 0, items.iter().map(|item| item.get()).collect::<Vec<_>>())],
            Input::Currency {instructions} => {
                let mut drawables = drawables![NumericalInput::numerical(theme, instructions)];
                if IS_MOBILE {drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            },
            Input::Date {instructions} => drawables![NumericalInput::date(theme, instructions)],
            Input::Time {instructions} => drawables![NumericalInput::time(theme, instructions)],
            Input::Avatar {content, flair, ..} => drawables![Avatar::new(theme, content.clone(), *flair, flair.is_some(), AvatarSize::Xxl, Some(Box::new(|ctx: &mut Context, theme: &Theme| ctx.pick_photo())))],
            Input::Search {items, actions} => drawables![SearchBar::new(theme, 
                items.iter().map(|(item, id)| (item.build(theme), *id)).collect::<Vec<_>>(), 
                if let Some(a) = actions.as_ref() && !a.is_empty() {
                    Some(QuickActions::new(theme, Offset::Start, a.iter().map(|(label, active, icon, action)| {
                        let on_click: Box<dyn Callback> = action.get();
                        ActionData{ label: label.to_string(), active: active.clone(), icon: *icon, on_click }
                    }).collect::<Vec<ActionData>>()))
                } else {None}
            )],
            Input::QRCodeScanner {instructions, alt} => {
                let mut items = drawables![
                    QRCodeScanner::new(theme, Box::new(|_ctx: &mut Context, _d: String| {})),
                    ExpandableText::new(theme, instructions, TextSize::Md, TextStyle::Secondary, Align::Center, None)
                ];
                if let Some((label, icon, action)) = alt.as_ref() {
                    items.push(Box::new(ExpandableText::new(theme, "or", TextSize::Md, TextStyle::Secondary, Align::Center, None)));
                    items.push(Box::new(QuickActions::new(theme, Offset::Center, vec![ActionData::new(label, None, *icon, action.get())])));
                }
                items
            },
        })
    }

    pub fn offset(&self) -> Offset {
        match self {
            Input::Text {..} |
            Input::Enumerator {..} |
            Input::Currency {..} |
            Input::Date {..} |
            Input::Time {..} |
            Input::Avatar {..} |
            Input::Search {..} => Offset::Start,
            Input::QRCodeScanner {..} => Offset::Center
        }
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
        } else if let Some(scanner) = child.downcast_ref::<QRCodeScanner>() {
            state.push(State::ScanCode(scanner.found()))
        }
    }
}

#[derive(Debug, Clone)]
pub enum Display {
    Text {text: String, size: TextSize, style: TextStyle, align: Align},
    Icon {icon: Icons},
    Image {image: String, size: (f32, f32)},
    Cta {label: String, data: Option<String>, instructions: String, actions: Vec<(String, Option<String>, Icons, Action)>},
    Table {label: String, items: Vec<TableItem>},
    Currency {amount: f32, instructions: String},
    List {label: Option<String>, item_getter: Arc<Box<dyn ListItemGetter>>, instructions: Option<String>},
    QRCode {data: String, instructions: String},
    Avatar {content: AvatarContent, purpose: AvatarPurpose},
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
        Display::Text {text: text.to_string(), size: TextSize::H5, style: TextStyle::Heading, align: Align::Center}
    }

    pub fn confirmation_message(text: &str) -> Self {
        Display::Text {text: text.to_string(), size: TextSize::H4, style: TextStyle::Heading, align: Align::Center}
    }

    pub fn icon(icon: Icons) -> Self {
        Display::Icon {icon}
    }

    pub fn image(image: &str, size: (f32, f32)) -> Self {
        Display::Image {image: image.to_string(), size}
    }

    pub fn cta(label: &str, data: Option<&str>, instructions: &str, actions: Vec<(String, Option<String>, Icons, Action)>) -> Self {
        Display::Cta {label: label.to_string(), data: data.map(|s| s.to_string()), instructions: instructions.to_string(), actions}
    }

    pub fn table(label: &str, items: Vec<TableItem>) -> Self {
        Display::Table {label: label.to_string(), items}
    }

    pub fn qr_code(data: &str, instructions: &str) -> Self {
        Display::QRCode {data: data.to_string(), instructions: instructions.to_string()}
    }

    pub fn list(label: Option<&str>, item_getter: Arc<Box<dyn ListItemGetter>>, instructions: Option<&str>) -> Self {
        // let item_getter: Arc<Box<dyn ListItemGetter>> = Arc::new(Box::new(|ctx: &mut Context| vec![]));
        Display::List{label: label.map(|i| i.to_string()), item_getter, instructions: instructions.map(|i| i.to_string())}
    }

    pub fn currency(amount: f32, instructions: &str) -> Self {
        Display::Currency {amount, instructions: instructions.to_string()}
    }

    pub fn avatar(content: AvatarContent, purpose: AvatarPurpose) -> Self {
        Display::Avatar {content, purpose}
    }

    pub fn actions(actions: Vec<ActionItem>) -> Self {
        Display::Actions {actions}
    }

    pub fn build(&mut self, theme: &Theme) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Display::Icon {icon} => drawables![Icon::new(theme, *icon, Some(theme.colors().get(colors::Text::Heading)), 128.0)],
            Display::Image {image, size} => drawables![Image{shape: ShapeType::Rectangle(0.0, *size, 0.0), image: theme.brand().images.get(&image.to_string()).unwrap().clone(), color: None}],
            Display::Text {text, size, style, align} if !text.is_empty() => drawables![ExpandableText::new(theme, text, *size, *style, *align, None)],
            Display::Cta {label, data, instructions, actions} => drawables![DataItem::text(theme, label, data.as_deref(), instructions, 
                Some(actions.iter().map(|(label, active, icon, action)| {
                    let on_click: Box<dyn Callback> = action.get();
                    ActionData {label: label.to_string(), active: active.clone(), icon: *icon, on_click}
                }).collect::<Vec<ActionData>>()),
            )],
            Display::Table {label, items} => drawables![DataItem::table(theme, label, items.iter().map(|TableItem{title, data}| (title.clone(), data.clone())).collect(), None)],
            Display::Currency {amount, instructions} => drawables![NumericalInput::display(theme, *amount, instructions)],
            Display::List {label, item_getter, instructions, ..} => {
                let item_getter = item_getter.clone();
                let theme = theme.clone();
                drawables![ListItemSection::new(&theme.clone(), label.clone(), instructions.clone(), move |ctx: &mut Context| {
                    (item_getter)(ctx).iter_mut().map(|item| item.build(&theme.clone())).collect::<Vec<_>>()
                })]
            },
            Display::QRCode {data, instructions} => drawables![QRCode::new(theme, data), ExpandableText::new(theme, instructions, TextSize::Md, TextStyle::Secondary, Align::Center, None)],
            Display::Avatar {content, purpose} => {
                let (style, outline) = match purpose {
                    AvatarPurpose::None => (None, false),
                    AvatarPurpose::Custom{icon, style} => (Some((*icon, *style)), true)
                };

                drawables![Avatar::new(theme, content.clone(), style, outline, AvatarSize::Xxl, None)]
            }
            Display::Actions {actions} => {
                let items = actions.into_iter().map(|ActionItem(action, _, icon)| {
                    let action: Box<dyn Callback> = action.get();
                    (*icon, action)
                }).collect::<Vec<_>>();

                drawables![IconButtonGroup::new(theme, items)]
            }
            //actions.iter_mut().map(|ActionItem(a, l, i)| Box::new(SecondaryButton::medium(theme, *i, l, None, a.get())) as Box<dyn Drawable>).collect::<Vec<_>>(),
            _ => return None
        })
    }
}

#[derive(Debug, Clone)]
pub struct ListItem {
    avatar: Option<AvatarContent>, 
    pub title: String, 
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
    Share { data: String },
    SelectImage,
    Custom { action: Box<dyn Callback> },
    None,
    Flow { flow: Flow },
    Paste,
    Copy { data: String },
    Message { name: Name },
    ChooseSearch { name: Name },
}

impl PartialEq for Action {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Action::Share { .. }, Action::Share { .. })
                | (Action::SelectImage, Action::SelectImage)
                | (Action::Custom { .. }, Action::Custom { .. })
                | (Action::None, Action::None)
                | (Action::Flow { .. }, Action::Flow { .. })
                | (Action::Paste, Action::Paste)
                | (Action::Copy { .. }, Action::Copy { .. })
                | (Action::Message { .. }, Action::Message { .. })
                | (Action::ChooseSearch { .. }, Action::ChooseSearch { .. })
        )
    }
}

impl Action {
    pub fn choose_search(name: Name) -> Self {
        Action::ChooseSearch {name}
    }

    pub fn share(data: &str) -> Self {
        Action::Share { data: data.to_string() }
    }

    pub fn copy(data: &str) -> Self {
        Action::Copy { data: data.to_string() }
    }

    pub fn select_image() -> Self {
        Action::SelectImage
    }

    pub fn scan_qr(theme: &Theme, instructions: &str) -> Self {
        let instructions = instructions.to_string();
        Action::Flow {
            flow: Flow::new(theme, vec![Box::new(move || crate::PageType::scan_qr(&instructions.clone(), None))]),
        }
    }

    pub fn custom(action: impl Callback + 'static) -> Self {
        Action::Custom { action: Box::new(action) }
    }

    pub fn flow(flow: Flow) -> Self {
        Action::Flow { flow }
    }

    pub fn block(theme: &Theme, profile: Instance<Profile>) -> Self {
        let theme = theme.clone();
        let avatar = profile.pending().avatar.clone();
        let username = profile.pending().username.clone();
        Action::flow(Flow::action_target(&theme, "block", "blocked", &username, avatar, AvatarPurpose::new(Icons::Block, AvatarIconStyle::Danger)))
    }

    pub fn unblock(theme: &Theme, profile: Instance<Profile>) -> Self {
        let theme = theme.clone();
        let avatar = profile.pending().avatar.clone();
        let username = profile.pending().username.clone();
        Action::flow(Flow::action_target(&theme, "unblock", "unblocked", &username, avatar, AvatarPurpose::new(Icons::Unblock, AvatarIconStyle::Success)))
    }

    pub fn message(name: Name) -> Self {
        Action::Message { name }
    }

    // pub fn navigate(flow: Flow) -> Self {
    //     Action::Navigate {flow}
    // }

    pub fn get(&self) -> Box<dyn Callback> {
        match self {
            Action::Share {data} => {
                let data = data.clone();
                Box::new(move |ctx: &mut Context, _: &Theme| ctx.share_social(data.clone()))
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
                Box::new(move |ctx: &mut Context, theme: &Theme| {flow.build(ctx)(ctx, theme);})
            }

            Action::Paste => {
                Box::new(move |ctx: &mut Context, _: &Theme| {ctx.emit(TextInputEvent::Set(ctx.get_clipboard().unwrap_or_default()));})
            }

            Action::Copy {data} => {
                let data = data.to_string();
                Box::new(move |ctx: &mut Context, _: &Theme| {ctx.set_clipboard(data.to_string())})
            }

            Action::Message {name} => {
                let recipient = name.clone();
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    let mut instance = ctx.create::<crate::messages::ChatRoom>(air::Id::random());
                    instance.apply(crate::messages::AddMember(recipient));
                    instance.share(recipient);
                    println!("Created room with members {:?}", recipient);

                    let mut flow = Flow::new(&theme, vec![Box::new(move || crate::PageType::messaging(instance.clone()))]);
                    flow.build(ctx)(ctx, theme);
                })
            }

            Action::ChooseSearch {name} => {
                let name = name.clone();
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    ctx.emit(SearchbarEvent::Select(name.clone()))
                })
            }

            // Action::Navigate {flow} => flow.clone().build(),

            _ => Box::new(move |_ctx: &mut Context, _: &Theme| {})
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

#[derive(Debug, Clone)]
pub enum AvatarPurpose {
    None,
    Custom {icon: Icons, style: AvatarIconStyle}
}

impl AvatarPurpose {
    pub fn new(icon: Icons, style: AvatarIconStyle) -> Self {
        AvatarPurpose::Custom {icon, style}
    }
}