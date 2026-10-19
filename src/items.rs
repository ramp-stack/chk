use pelican_ui::{colors, Context, Callback};
use pelican_ui::drawable::Drawable;
use prism::drawable::{Component, text::Align};
use prism::events::{on_event, ChildEvent, DynEvent, Event, OnEvent, TickEvent};
use prism::hardware::{Image, Picked};
use ptsd::layout::Stack;
use std::cell::RefCell;
use std::rc::Rc;
use prism::hardware::Data;
use pelican_ui::theme::{Theme, Icons};
use ptsd::layout::Offset;
use pelican_ui::utils::{TitleSubtitle};
use pelican_ui::components::list_item::{ListItemSection, ListItemInfoLeft, ListItem as PelicanListItem};
use pelican_ui::components::{NumberGetter, QRCodeScanner, TextInput, RadioSelector, Icon, DataItem, QRCode, NumericalInput};
use pelican_ui::components::text::{ExpandableText, TextStyle, TextSize};
use pelican_ui::components::avatar::{Avatar, AvatarSize};
pub use pelican_ui::components::avatar::{AvatarContent, AvatarIconStyle};
use pelican_ui::components::button::{QuickActions, IconButtonGroup, ActionData};
use pelican_ui::components::{SearchBar, SearchbarEvent, Keypad, TextInputEvent};
use pelican_ui::interface::system::KeyboardPages;
use pelican_ui::components::{MessageGroups, Message as PelicanMessage};
use prism::air::{Id, Name, Instance};
use crate::profiles::Profile;
use crate::blocking::{Block, BlockAction};

use crate::Page;
use crate::form::State;
use crate::flow::Flow;
use crate::messages::ChatRoom;

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Text {label: String, actions: Option<Vec<(String, Option<String>, Icons, Action)>>, show_label: bool, preset: Option<String>},
    Currency {instructions: String},
    Number {instructions: String},
    Date {instructions: String},
    Time {instructions: String},
    Enumerator {items: Vec<EnumItem>},
    Avatar {content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>},
    Search {label: String, items: Vec<(ListItem, Name)>, actions: Option<Vec<(String, Option<String>, Icons, Action)>>},
    QRCodeScanner {instructions: String, alt: Option<(String, Icons, Action)>},
}

impl Input {
    pub fn currency(instructions: &str) -> Self {
        Input::Currency {instructions: instructions.to_string()}
    }

    pub fn date(instructions: &str) -> Self {
        Input::Date {instructions: instructions.to_string()}
    }

    pub fn time(instructions: &str) -> Self {
        Input::Time {instructions: instructions.to_string()}
    }

    pub fn number(instructions: &str) -> Self {
        Input::Number {instructions: instructions.to_string()}
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

    pub fn search(label: &str, items: Vec<(ListItem, Name)>, actions: Option<Vec<(String, Option<String>, Icons, Action)>>) -> Self {
        Input::Search {label: label.to_string(), items, actions}
    }

    pub fn qr_code_scanner(instructions: &str, alt: Option<(String, Icons, Action)>) -> Self {
        Input::QRCodeScanner {instructions: instructions.to_string(), alt}
    }

    pub fn build(&self, theme: &Theme) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Input::Text {show_label, label, preset, actions} => {
                let mut items: Vec<Box<dyn Drawable>> = vec![Box::new(TextInput::new(
                    theme, preset.as_deref(), show_label.then_some(label),
                    Some(&format!("Enter {}...", label.to_lowercase())),
                    None, None, KeyboardPages::EMOJI,
                ))];
                if let Some(a) = actions.as_ref()
                    && !a.is_empty() {
                        items.push(Box::new(QuickActions::new(theme, Offset::Start, a.iter().map(|(label, active, icon, action)| {
                            let on_click: Box<dyn Callback> = action.get();
                            ActionData { label: label.to_string(), active: active.clone(), icon: *icon, on_click }
                        }).collect::<Vec<ActionData>>())));
                    }

                items
            },
            Input::Enumerator {items} => vec![Box::new(RadioSelector::new(theme, 0, items.iter().map(|item| item.get()).collect::<Vec<_>>()))],
            Input::Currency {instructions} => {
                let mut drawables: Vec<Box<dyn Drawable>> = vec![Box::new(NumericalInput::numerical(theme, instructions))];
                if cfg!(any(target_os = "ios", target_os = "android")) {drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            },
            Input::Number {instructions} => {
                let mut drawables: Vec<Box<dyn Drawable>> = vec![Box::new(NumericalInput::number(theme, instructions))];
                if cfg!(any(target_os = "ios", target_os = "android")) {drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            },
            Input::Date {instructions} => {
                let mut drawables: Vec<Box<dyn Drawable>> = vec![Box::new(NumericalInput::date(theme, instructions))];
                if cfg!(any(target_os = "ios", target_os = "android")) {drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            },
            Input::Time {instructions} => {
                let mut drawables: Vec<Box<dyn Drawable>> = vec![Box::new(NumericalInput::time(theme, instructions))];
                if cfg!(any(target_os = "ios", target_os = "android")) {drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            },
            Input::Avatar {content, flair, action} => vec![Box::new(AvatarInput::new(theme, content.clone(), *flair, action.clone()))],
            Input::Search {label, items, actions} => vec![Box::new(SearchBar::new(theme, label,
                items.iter().map(|(item, id)| (item.build(theme), *id)).collect::<Vec<_>>(),
                if let Some(a) = actions.as_ref() && !a.is_empty() {
                    Some(QuickActions::new(theme, Offset::Start, a.iter().map(|(label, active, icon, action)| {
                        let on_click: Box<dyn Callback> = action.get();
                        ActionData{ label: label.to_string(), active: active.clone(), icon: *icon, on_click }
                    }).collect::<Vec<ActionData>>()))
                } else {None}
            ))],
            Input::QRCodeScanner {instructions, alt} => {
                let mut items: Vec<Box<dyn Drawable>> = vec![
                    Box::new(QRCodeScanner::new(theme, Box::new(|_ctx: &mut Context, _d: String| {}))),
                    Box::new(ExpandableText::new(
                        theme, instructions, TextSize::Md, TextStyle::Secondary, Align::Center, None,
                    )),
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
            Input::Number {..} |
            Input::Enumerator {..} |
            Input::Currency {..} |
            Input::Date {..} |
            Input::Time {..} |
            Input::Avatar {..} |
            Input::Search {..} => Offset::Start,
            Input::QRCodeScanner {..} => Offset::Center
        }
    }

    pub fn state(child: &dyn Drawable) -> Option<State> {
        if let Some(input) = child.downcast_ref::<TextInput>() {
            Some(State::Text(input.value()))
        } else if let Some(selector) = child.downcast_ref::<RadioSelector>() {
            Some(State::Enumerator(selector.value()))
        } else if let Some(input) = child.downcast_ref::<NumericalInput>() {
            Some(State::Number(input.value()))
        } else if let Some(avatar) = child.downcast_ref::<AvatarInput>() {
            Some(State::Avatar(avatar.content().clone()))
        } else if let Some(searchbar) = child.downcast_ref::<SearchBar>() {
            Some(State::Search(searchbar.results()))
        } else {
            child.downcast_ref::<QRCodeScanner>().map(|scanner| State::ScanCode(scanner.found()))
        }
    }

}

#[derive(Debug, Clone)]
pub enum Display {
    Text {text: String, size: TextSize, style: TextStyle, align: Align},
    Icon {icon: Icons},
    Image {image: String, size: (f32, f32)},
    Cta {label: String, data: Option<String>, instructions: String, actions: Vec<ActionItem>},
    Table {label: String, items: Vec<TableItem>, actions: Option<Vec<ActionItem>>},
    Currency {amount: Box<dyn NumberGetter>, instructions: String},
    Number {number: Box<dyn NumberGetter>, instructions: String},
    List {label: Option<String>, items: Vec<ListItem>, instructions: Option<String>},
    QRCode {data: String, instructions: String},
    Avatar {content: AvatarContent, purpose: AvatarPurpose},
    Actions {actions: Vec<ActionItem>, as_icons: bool},
    Messages {messages: MessageGroups},
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

    pub fn cta(label: &str, data: Option<&str>, instructions: &str, actions: Vec<ActionItem>) -> Self {
        Display::Cta {label: label.to_string(), data: data.map(|s| s.to_string()), instructions: instructions.to_string(), actions}
    }

    pub fn table(label: &str, items: Vec<TableItem>, actions: Option<Vec<ActionItem>>) -> Self {
        Display::Table {label: label.to_string(), items, actions}
    }

    pub fn qr_code(data: &str, instructions: &str) -> Self {
        Display::QRCode {data: data.to_string(), instructions: instructions.to_string()}
    }

    pub fn list(label: Option<&str>, items: Vec<ListItem>, instructions: Option<&str>) -> Self {
        Display::List{label: label.map(|i| i.to_string()), items, instructions: instructions.map(|i| i.to_string())}
    }

    pub fn currency(amount: impl NumberGetter + Clone + 'static, instructions: &str) -> Self {
        Display::Currency {amount: Box::new(amount), instructions: instructions.to_string()}
    }

    pub fn number(number: impl NumberGetter + Clone + 'static, instructions: &str) -> Self {
        Display::Number {number: Box::new(number), instructions: instructions.to_string()}
    }

    pub fn avatar(content: AvatarContent, purpose: AvatarPurpose) -> Self {
        Display::Avatar {content, purpose}
    }

    pub fn actions(actions: Vec<ActionItem>, as_icons: bool) -> Self {
        Display::Actions {actions, as_icons}
    }

    pub fn messages(ctx: &mut Context, theme: &Theme, messages: Vec<PelicanMessage>) -> Self {
        Display::Messages {messages: MessageGroups::new(ctx, theme, messages, false, false)}
    }

    pub fn build(&mut self, theme: &Theme) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Display::Icon {icon} => vec![Box::new(Icon::new(theme, *icon, Some(theme.colors().get(colors::Text::Heading)), 128.0))],
            Display::Image {image, size} => vec![Box::new(pelican_ui::components::AspectRatioImage::new(
                theme.brand().images.get(image).unwrap().clone(), *size
            ))],
            Display::Text {text, size, style, align} if !text.is_empty() => vec![
                Box::new(ExpandableText::new(theme, text, *size, *style, *align, None)),
            ],
            Display::Cta {label, data, instructions, actions} => vec![Box::new(DataItem::text(theme, label, data.as_deref(), instructions,
                Some(actions.iter().map(|ActionItem(action, label, active, icon)| {
                    let on_click: Box<dyn Callback> = action.get();
                    ActionData {label: label.to_string(), active: active.clone(), icon: *icon, on_click}
                }).collect::<Vec<ActionData>>()),
            ))],
            Display::Table {label, items, actions} => {
                let actions = actions.clone().map(|a| a.iter().map(|ActionItem(action, label, active, icon)| {
                    let on_click: Box<dyn Callback> = action.get();
                    ActionData {label: label.to_string(), active: active.clone(), icon: *icon, on_click}
                }).collect::<Vec<ActionData>>());
                vec![Box::new(DataItem::table(
                    theme, label,
                    items.iter().map(|TableItem {title, data}| (title.clone(), data.clone())).collect(),
                    actions,
                ))]
            },
            Display::Currency {amount, instructions} => vec![Box::new(NumericalInput::display_currency(theme, amount.clone(), instructions))],
            Display::Number {number, instructions} => vec![Box::new(NumericalInput::display_number(theme, number.clone(), instructions))],
            Display::List {label, items, instructions, ..} => {
                let items = items.into_iter().map(|i| i.build(&theme)).collect::<Vec<_>>();
                vec![Box::new(ListItemSection::new(&theme.clone(), label.clone(), instructions.clone(), items))]
            },
            Display::QRCode {data, instructions} => vec![
                Box::new(QRCode::new(theme, data)),
                Box::new(ExpandableText::new(
                    theme, instructions, TextSize::Md, TextStyle::Secondary, Align::Center, None,
                )),
            ],
            Display::Avatar {content, purpose} => {
                let (style, outline) = match purpose {
                    AvatarPurpose::None => (None, false),
                    AvatarPurpose::Custom{icon, style} => (Some((*icon, *style)), true)
                };

                vec![Box::new(Avatar::new(theme, content.clone(), style, outline, AvatarSize::Xxl, None))]
            }
            Display::Actions {actions, as_icons} => {
                match as_icons {
                    true => {
                        let items = actions.into_iter().map(|ActionItem(action, _, _, icon)| {
                            let action: Box<dyn Callback> = action.get();
                            (*icon, action)
                        }).collect::<Vec<_>>();
                        vec![Box::new(IconButtonGroup::new(theme, items))]
                    },
                    false => {
                        vec![Box::new(QuickActions::new(theme, Offset::Start, actions.into_iter().map(|ActionItem(action, label, _, icon)| {
                            let on_click: Box<dyn Callback> = action.get();
                            ActionData { label: label.to_string(), active: None, icon: *icon, on_click }
                        }).collect::<Vec<ActionData>>()))]
                    }
                }
            },
            Display::Messages {messages} => vec![Box::new(messages.clone())],
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

    pub fn build(&self, theme: &Theme) -> PelicanListItem {
        let ListItem {avatar, title, subtitle, secondary, flow} = self.clone();
        let has_flow = flow.is_some();
        let closure = Box::new(move |ctx: &mut Context, theme: &Theme| {
            if let Some(mut f) = flow.clone() {(f.build(ctx, theme))(ctx, theme);}
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
    Block {name: Name},
    Unblock {name: Name},
    PickPhoto,
}

impl PartialEq for Action {fn eq(&self, _: &Self) -> bool {true}}

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

    pub fn scan_qr(instructions: &str) -> Self {
        Action::Flow {
            flow: Flow::new(vec![Page::Static(crate::PageType::scan_qr(&instructions, None))]),
        }
    }

    pub fn custom(action: impl Callback + 'static) -> Self {
        Action::Custom { action: Box::new(action) }
    }

    pub fn flow(flow: Flow) -> Self {
        Action::Flow { flow }
    }

    pub fn block(profile: Instance<Profile>) -> Self {
        let profile = profile.pending();
        let avatar = profile.avatar.clone();
        let username = profile.username.clone();
        let name = profile.name.unwrap().clone();
        Action::flow(Flow::action_target(
            ("block", Action::Block {name}), "blocked", &username, avatar,
            AvatarPurpose::new(Icons::Block, AvatarIconStyle::Danger),
        ))
    }

    pub fn unblock(profile: Instance<Profile>) -> Self {
        let profile = profile.pending();
        let avatar = profile.avatar.clone();
        let username = profile.username.clone();
        let name = profile.name.unwrap().clone();
        Action::flow(Flow::action_target(
            ("unblock", Action::Unblock {name}), "unblocked", &username, avatar,
            AvatarPurpose::new(Icons::Unblock, AvatarIconStyle::Success),
        ))
    }

    pub fn message(name: Name) -> Self {
        Action::Message { name }
    }


    pub fn get(&self) -> Box<dyn Callback> {
        match self {
            Action::Share {data} => {
                let data = data.clone();
                Box::new(move |ctx: &mut Context, _: &Theme| ctx.hardware.share(&Data::Text(data.clone())))
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
                Box::new(move |ctx: &mut Context, theme: &Theme| {flow.build(ctx, theme)(ctx, theme);})
            }

            Action::Paste => {
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    let text = match ctx.hardware.get_clipboard() {
                        Some(Data::Text(text)) => text,
                        _ => String::new(),
                    };
                    ctx.emit(TextInputEvent::Set(text));
                })
            }

            Action::Copy {data} => {
                let data = data.to_string();
                Box::new(move |ctx: &mut Context, _: &Theme| {ctx.hardware.set_clipboard(&Data::Text(data.clone()))})
            }

            Action::Message {name} => {
                let recipient = name.clone();
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    let instance = ChatRoom::create(ctx, vec![recipient]);
                    let mut flow = Flow::new(vec![Page::messaging(ctx, &mut instance.clone())]);
                    flow.build(ctx, theme)(ctx, theme);
                })
            }

            Action::ChooseSearch {name} => {
                let name = name.clone();
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    ctx.emit(SearchbarEvent::Select(name.clone()))
                })
            }

            Action::Block {name} => {
                let name = name.clone();
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    let mut block = Block::create(ctx, name.clone());
                    block.send(BlockAction::Block);
                })
            }

            Action::Unblock {name} => {
                let name = name.clone();
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    let mut block = Block::create(ctx, name.clone());
                    block.send(BlockAction::Unblock);
                })
            }


            _ => Box::new(move |_ctx: &mut Context, _: &Theme| {})
        }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct ActionItem(Action, String, Option<String>, Icons);
impl ActionItem {
    pub fn new(action: Action, label: &str, icon: Icons) -> Self {
        ActionItem(action, label.to_string(), None, icon)
    }

    pub fn new_with_active(action: Action, label: &str, active: &str, icon: Icons) -> Self {
        ActionItem(action, label.to_string(), Some(active.to_string()), icon)
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct TableItem {title: String, data: String}

impl TableItem {
    pub fn new(title: &str, data: &str) -> Self {
        TableItem { title: title.to_string(), data: data.to_string() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumItem {title: String, data: String}
impl EnumItem {
    pub fn new(title: &str, data: &str) -> Self {
        EnumItem {title: title.to_string(), data: data.to_string()}
    }
    fn get(&self) -> (&str, &str, Box<dyn Callback>) {
        (&self.title, &self.data, Box::new(|_, _| {}))
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

#[doc(hidden)]
#[derive(Clone, Component)]
pub struct AvatarInput {
    layout: Stack,
    avatar: Avatar,
    #[skip] id: Id,
    #[skip] picked: Rc<RefCell<Option<Picked<Image>>>>,
}

impl AvatarInput {
    fn new(theme: &Theme, content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>) -> Self {
        let id = Id::random();
        let on_click = action.map(|action| match action {
            Action::PickPhoto => Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(PickPhoto(id)))
                as Box<dyn Callback>,
            action => action.get(),
        });
        Self {
            layout: Stack::default(),
            avatar: Avatar::new(theme, content, flair, false, AvatarSize::Xxl, on_click),
            id, picked: Rc::new(RefCell::new(None)),
        }
    }
    pub fn content(&self) -> &AvatarContent {&self.avatar.content}
}

impl std::fmt::Debug for AvatarInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AvatarInput").field("avatar", &self.avatar).finish()
    }
}

#[on_event]
impl OnEvent<PickPhoto> for AvatarInput {
    fn on(&mut self, ctx: &mut Context, event: PickPhoto) -> Vec<Box<dyn DynEvent>> {
        if event.0 == self.id {*self.picked.borrow_mut() = Some(ctx.hardware.pick::<Image>());}
        vec![]
    }
}

#[on_event]
impl OnEvent<TickEvent> for AvatarInput {
    fn on(&mut self, _ctx: &mut Context, event: TickEvent) -> Vec<Box<dyn DynEvent>> {
        let result = self.picked.borrow_mut().as_mut().and_then(Picked::try_get);
        if let Some(result) = result {
            self.picked.borrow_mut().take();
            if let Ok(image) = result {self.avatar.content = AvatarContent::Image(image);}
        }
        prism::events![event]
    }
}

#[derive(Debug, Clone)]
struct PickPhoto(Id);
impl Event for PickPhoto {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}
