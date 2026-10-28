use pelican_ui::{colors, Context, Callback};
use pelican_ui::drawable::Drawable;
use prism::drawable::{Component, text::Align};
use prism::events::{on_change, on_event, ChildEvent, DynEvent, Event, OnEvent, OnChange};
use prism::hardware::{Image, Picked};
use ptsd::layout::Stack;
use uuid::Uuid;
use pelican_ui::components::QrCodeFound;
use prism::hardware::Data;
use pelican_ui::theme::{Theme, Icons};
use ptsd::layout::Offset;
use pelican_ui::utils::{TitleSubtitle};
use pelican_ui::components::list_item::{
    ListItemSection, ListItemInfoLeft, ListItem as PelicanListItem,
};
use pelican_ui::components::{
    QRCodeScanner, TextInput, RadioSelector, Icon, DataItem, QRCode, NumericalInput,
};
pub use pelican_ui::components::Number;
use pelican_ui::components::text::{Text, TextStyle, TextSize};
use pelican_ui::components::avatar::{Avatar, AvatarSize};
pub use pelican_ui::components::avatar::{AvatarContent, AvatarIconStyle};
use pelican_ui::components::button::{QuickActions, IconButtonGroup, ActionData};
use pelican_ui::components::{SearchBar, SearchbarEvent, Keypad, TextInputEvent};
use pelican_ui::interface::general::FormEvent;
use pelican_ui::components::{MessageGroups, Message as PelicanMessage};
use prism::air::{Id, Name, Instance};

use crate::profiles::Profile;
use crate::blocking::{Block, BlockAction};
use crate::Page;
use crate::form::State;
use crate::flow::Flow;
use crate::messages::ChatRoom;

pub type InputAction = (String, Option<String>, Icons, Action);

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Text {
        id: Uuid,
        label: String,
        actions: Option<Vec<InputAction>>,
        show_label: bool,
        preset: Option<String>,
    },
    Currency { instructions: String },
    Number { instructions: String },
    Date { instructions: String },
    Time { instructions: String },
    Enumerator { items: Vec<EnumItem> },
    Avatar {
        content: AvatarContent,
        flair: Option<(Icons, AvatarIconStyle)>,
        action: Option<Action>,
    },
    Search {
        id: Uuid,
        label: String,
        items: Vec<(ListItem, Name)>,
        actions: Option<Vec<InputAction>>,
    },
    QRCodeScanner {
        instructions: String,
        alt: Option<(String, Icons, Action)>,
        on_find: Box<dyn QrCodeFound>,
    },
}

impl Input {
    pub fn currency(instructions: &str) -> Self {
        Input::Currency { instructions: instructions.to_string() }
    }

    pub fn date(instructions: &str) -> Self {
        Input::Date { instructions: instructions.to_string() }
    }

    pub fn time(instructions: &str) -> Self {
        Input::Time { instructions: instructions.to_string() }
    }

    pub fn number(instructions: &str) -> Self {
        Input::Number { instructions: instructions.to_string() }
    }

    pub fn enumerator(items: Vec<EnumItem>) -> Self { Input::Enumerator { items } }

    pub fn text(
        id: Uuid, label: &str, show_label: bool, preset: Option<String>,
        actions: Option<Vec<InputAction>>,
    ) -> Self { Input::Text { id, label: label.to_string(), show_label, preset, actions } }

    pub fn avatar(content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>) -> Self {
        Input::Avatar { content, flair, action }
    }

    pub fn search(id: Uuid, label: &str, items: Vec<(ListItem, Name)>, actions: Option<Vec<InputAction>>) -> Self {
        Input::Search { id, label: label.to_string(), items, actions }
    }

    pub fn qr_code_scanner(instructions: &str, alt: Option<(String, Icons, Action)>, on_find: Box<dyn QrCodeFound>) -> Self {
        Input::QRCodeScanner { instructions: instructions.to_string(), alt, on_find }
    }

    pub fn build(&self, theme: &Theme, is_mobile: bool) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Input::Text { id, show_label, label, preset, actions } => {
                let mut items: Vec<Box<dyn Drawable>> = vec![Box::new(TextInput::new(
                    theme, *id, preset.as_deref(), show_label.then_some(label),
                    Some(&format!("Enter {}...", label.to_lowercase())),
                    None, None,
                ))];
                if let Some(a) = actions.as_ref()
                    && !a.is_empty() {
                    items.push(Box::new(QuickActions::new(
                        theme, Offset::Start,
                        a.iter()
                            .map(|(label, active, icon, action)| {
                                let on_click: Box<dyn Callback> = action.get();
                                ActionData {
                                    label: label.to_string(),
                                    active: active.clone(),
                                    icon: *icon,
                                    on_click,
                                }
                            })
                            .collect::<Vec<ActionData>>(),
                    )));
                }

                items
            }
            Input::Enumerator { items } => vec![Box::new(RadioSelector::new(
                theme, 0, items.iter().map(|item| item.get()).collect::<Vec<_>>(),
            ))],
            Input::Currency { instructions } => {
                let mut drawables: Vec<Box<dyn Drawable>> =
                    vec![Box::new(NumericalInput::numerical(theme, instructions))];
                if is_mobile { drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            }
            Input::Number { instructions } => {
                let mut drawables: Vec<Box<dyn Drawable>> =
                    vec![Box::new(NumericalInput::number(theme, instructions))];
                if is_mobile { drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            }
            Input::Date { instructions } => {
                let mut drawables: Vec<Box<dyn Drawable>> =
                    vec![Box::new(NumericalInput::date(theme, instructions))];
                if is_mobile { drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            }
            Input::Time { instructions } => {
                let mut drawables: Vec<Box<dyn Drawable>> =
                    vec![Box::new(NumericalInput::time(theme, instructions))];
                if is_mobile { drawables.push(Box::new(Keypad::new(theme))); }
                drawables
            }
            Input::Avatar { content, flair, action } => {
                vec![Box::new(AvatarInput::new(theme, content.clone(), *flair, action.clone()))]
            }
            Input::Search { id, label, items, actions } => vec![Box::new(SearchBar::new(
                theme, *id, label,
                items.iter().map(|(item, id)| (item.build(theme), *id)).collect::<Vec<_>>(),
                if let Some(a) = actions.as_ref()
                    && !a.is_empty() {
                    Some(QuickActions::new(
                        theme, Offset::Start,
                        a.iter()
                            .map(|(label, active, icon, action)| {
                                let on_click: Box<dyn Callback> = action.get();
                                ActionData {
                                    label: label.to_string(),
                                    active: active.clone(),
                                    icon: *icon,
                                    on_click,
                                }
                            })
                            .collect::<Vec<ActionData>>(),
                    ))
                } else {
                    None
                },
            ))],
            Input::QRCodeScanner { instructions, alt, on_find } => {
                let mut items: Vec<Box<dyn Drawable>> = vec![
                    Box::new(QRCodeScanner::new(theme, on_find.clone())),
                    Box::new(Text::new(
                        theme, instructions, TextSize::Md, TextStyle::Secondary, Align::Center,
                        None,
                    )),
                ];
                if let Some((label, icon, action)) = alt.as_ref() {
                    items.push(Box::new(Text::new(
                        theme, "or", TextSize::Md, TextStyle::Secondary, Align::Center, None,
                    )));
                    items.push(Box::new(QuickActions::new(
                        theme, Offset::Center,
                        vec![ActionData::new(label, None, *icon, action.get())],
                    )));
                }
                items
            }
        })
    }

    pub fn offset(&self) -> Offset {
        match self {
            Input::Text { .. }
            | Input::Number { .. }
            | Input::Enumerator { .. }
            | Input::Currency { .. }
            | Input::Date { .. }
            | Input::Time { .. }
            | Input::Avatar { .. }
            | Input::Search { .. } => Offset::Start,
            Input::QRCodeScanner { .. } => Offset::Center,
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
    Text { text: String, size: TextSize, style: TextStyle, align: Align },
    Icon { icon: Icons },
    Image { image: String, size: (f32, f32) },
    Cta { label: String, data: Option<String>, instructions: String, actions: Vec<ActionItem> },
    Table { label: String, items: Vec<TableItem>, actions: Option<Vec<ActionItem>> },
    Currency { amount: Number, instructions: String },
    Number { number: Number, instructions: String },
    List { label: Option<String>, items: Vec<ListItem>, instructions: Option<String> },
    QRCode { data: String, instructions: String },
    Avatar { content: AvatarContent, purpose: AvatarPurpose },
    Actions { actions: Vec<ActionItem>, as_icons: bool },
    Messages { messages: MessageGroups },
}

impl Display {
    pub fn text(text: &str) -> Self {
        Display::Text {
            text: text.to_string(),
            size: TextSize::Md,
            style: TextStyle::Primary,
            align: Align::Left,
        }
    }

    pub fn instructions(text: &str) -> Self {
        Display::Text {
            text: text.to_string(),
            size: TextSize::Md,
            style: TextStyle::Secondary,
            align: Align::Center,
        }
    }

    pub fn label(text: &str) -> Self {
        Display::Text {
            text: text.to_string(),
            size: TextSize::H5,
            style: TextStyle::Heading,
            align: Align::Center,
        }
    }

    pub fn confirmation_message(text: &str) -> Self {
        Display::Text {
            text: text.to_string(),
            size: TextSize::H4,
            style: TextStyle::Heading,
            align: Align::Center,
        }
    }

    pub fn icon(icon: Icons) -> Self { Display::Icon { icon } }

    pub fn image(image: &str, size: (f32, f32)) -> Self {
        Display::Image { image: image.to_string(), size }
    }

    pub fn cta(label: &str, data: Option<&str>, instructions: &str, actions: Vec<ActionItem>) -> Self {
        Display::Cta {
            label: label.to_string(),
            data: data.map(|s| s.to_string()),
            instructions: instructions.to_string(),
            actions,
        }
    }

    pub fn table(label: &str, items: Vec<TableItem>, actions: Option<Vec<ActionItem>>) -> Self {
        Display::Table { label: label.to_string(), items, actions }
    }

    pub fn qr_code(data: &str, instructions: &str) -> Self {
        Display::QRCode { data: data.to_string(), instructions: instructions.to_string() }
    }

    pub fn list(label: Option<&str>, items: Vec<ListItem>, instructions: Option<&str>) -> Self {
        Display::List {
            label: label.map(|i| i.to_string()),
            items,
            instructions: instructions.map(|i| i.to_string()),
        }
    }

    pub fn currency(amount: impl Into<Number>, instructions: &str) -> Self {
        Display::Currency { amount: amount.into(), instructions: instructions.to_string() }
    }

    pub fn number(number: impl Into<Number>, instructions: &str) -> Self {
        Display::Number { number: number.into(), instructions: instructions.to_string() }
    }

    pub fn avatar(content: AvatarContent, purpose: AvatarPurpose) -> Self {
        Display::Avatar { content, purpose }
    }

    pub fn actions(actions: Vec<ActionItem>, as_icons: bool) -> Self {
        Display::Actions { actions, as_icons }
    }

    pub fn messages(ctx: &mut Context, theme: &Theme, messages: Vec<PelicanMessage>) -> Self {
        Display::Messages { messages: MessageGroups::new(ctx, theme, messages, false, false) }
    }

    pub fn build(&mut self, theme: &Theme) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Display::Icon { icon } => vec![Box::new(Icon::new(
                theme, *icon, Some(theme.colors().get(colors::Text::Heading)), 128.0,
            ))],
            Display::Image { image, size } => match theme.brand().images.get(image) {
                Some(image) => vec![Box::new(pelican_ui::components::AspectRatioImage::new(
                    image.clone(), size.0,
                ))],
                None => vec![Box::new(pelican_ui::components::AspectRatioImage::svg(
                    theme.brand().svgs.get(image).unwrap().clone(), size.0,
                ))],
            },
            Display::Text { text, size, style, align } if !text.is_empty() => {
                vec![Box::new(Text::new(theme, text, *size, *style, *align, None))]
            }
            Display::Cta { label, data, instructions, actions } => vec![Box::new(DataItem::text(
                theme, label, data.as_deref(), instructions,
                Some(
                    actions
                        .iter()
                        .map(|ActionItem(action, label, active, icon)| {
                            let on_click: Box<dyn Callback> = action.get();
                            ActionData {
                                label: label.to_string(),
                                active: active.clone(),
                                icon: *icon,
                                on_click,
                            }
                        })
                        .collect::<Vec<ActionData>>(),
                ),
            ))],
            Display::Table { label, items, actions } => {
                let actions = actions.clone().map(|a| {
                    a.iter()
                        .map(|ActionItem(action, label, active, icon)| {
                            let on_click: Box<dyn Callback> = action.get();
                            ActionData {
                                label: label.to_string(),
                                active: active.clone(),
                                icon: *icon,
                                on_click,
                            }
                        })
                        .collect::<Vec<ActionData>>()
                });
                vec![Box::new(DataItem::table(
                    theme, label,
                    items
                        .iter()
                        .map(|TableItem { title, data }| (title.clone(), data.clone()))
                        .collect(),
                    actions,
                ))]
            }
            Display::Currency { amount, instructions } => vec![Box::new(
                NumericalInput::display_currency(theme, amount.clone(), instructions),
            )],
            Display::Number { number, instructions } => {
                vec![Box::new(NumericalInput::display_number(theme, number.clone(), instructions))]
            }
            Display::List { label, items, instructions, .. } => {
                let items = items.iter_mut().map(|i| i.build(theme)).collect::<Vec<_>>();
                vec![Box::new(ListItemSection::new(
                    &theme.clone(), label.clone(), instructions.clone(), items,
                ))]
            }
            Display::QRCode { data, instructions } => vec![
                Box::new(QRCode::new(theme, data)),
                Box::new(Text::new(
                    theme, instructions, TextSize::Md, TextStyle::Secondary, Align::Center, None,
                )),
            ],
            Display::Avatar { content, purpose } => {
                let (style, outline) = match purpose {
                    AvatarPurpose::None => (None, false),
                    AvatarPurpose::Custom { icon, style } => (Some((*icon, *style)), true),
                };

                vec![Box::new(Avatar::new(
                    theme, content.clone(), style, outline, AvatarSize::Xxl, None,
                ))]
            }
            Display::Actions { actions, as_icons } => match as_icons {
                true => {
                    let items = actions
                        .iter_mut()
                        .map(|ActionItem(action, _, _, icon)| {
                            let action: Box<dyn Callback> = action.get();
                            (*icon, action)
                        })
                        .collect::<Vec<_>>();
                    vec![Box::new(IconButtonGroup::new(theme, items))]
                }
                false => {
                    vec![Box::new(QuickActions::new(
                        theme, Offset::Start,
                        actions
                            .iter_mut()
                            .map(|ActionItem(action, label, _, icon)| {
                                let on_click: Box<dyn Callback> = action.get();
                                ActionData {
                                    label: label.to_string(),
                                    active: None,
                                    icon: *icon,
                                    on_click,
                                }
                            })
                            .collect::<Vec<ActionData>>(),
                    ))]
                }
            },
            Display::Messages { messages } => vec![Box::new(messages.clone())],
            _ => return None,
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
        self.avatar == other.avatar
            && self.title == other.title
            && self.subtitle == other.subtitle
            && self.secondary == other.secondary
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
        let ListItem { avatar, title, subtitle, secondary, flow } = self.clone();
        let has_flow = flow.is_some();
        let closure = Box::new(move |ctx: &mut Context, theme: &Theme| {
            if let Some(mut f) = flow.clone() { (f.build(ctx, theme))(ctx, theme); }
        });

        let avatar = avatar.map(|content| {
            Avatar::new(theme, content, None, false, AvatarSize::Md, None)
        });
        PelicanListItem::new(
            theme, avatar, ListItemInfoLeft::new(&title, Some(&subtitle), None, None),
            secondary.as_ref().map(|s| TitleSubtitle::new(s, Some("Details"))), None,
            has_flow.then_some(Icons::Forward), closure,
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
    Paste { id: Uuid },
    Copy { data: String },
    Message { name: Name },
    ChooseSearch { name: Name },
    Block { name: Name },
    Unblock { name: Name },
    PickPhoto,
}

impl PartialEq for Action {
    fn eq(&self, _: &Self) -> bool { true }
}

impl Action {
    pub fn choose_search(name: Name) -> Self { Action::ChooseSearch { name } }

    pub fn share(data: &str) -> Self { Action::Share { data: data.to_string() } }

    pub fn copy(data: &str) -> Self { Action::Copy { data: data.to_string() } }

    pub fn select_image() -> Self { Action::SelectImage }

    pub fn scan_qr(instructions: &str, id: Uuid) -> Self {
        let on_find = Box::new(move |ctx: &mut Context, value: String| {
            ctx.emit(TextInputEvent::Set { id, value });
        });
        Action::Flow {
            flow: Flow::new(vec![Page::Static(Box::new(crate::PageType::scan_qr(
                instructions, None, on_find,
            )))]),
        }
    }

    pub fn custom(action: impl Callback + 'static) -> Self {
        Action::Custom { action: Box::new(action) }
    }

    pub fn flow(flow: Flow) -> Self { Action::Flow { flow } }

    pub fn block(profile: Instance<Profile>) -> Self {
        let profile = profile.pending();
        let avatar = profile.avatar.clone();
        let username = profile.username.clone();
        let name = profile.name.unwrap();
        Action::flow(Flow::action_target(
            ("block", Action::Block { name }),
            "blocked", &username, avatar, AvatarPurpose::new(Icons::Block, AvatarIconStyle::Danger),
        ))
    }

    pub fn unblock(profile: Instance<Profile>) -> Self {
        let profile = profile.pending();
        let avatar = profile.avatar.clone();
        let username = profile.username.clone();
        let name = profile.name.unwrap();
        Action::flow(Flow::action_target(
            ("unblock", Action::Unblock { name }),
            "unblocked", &username, avatar,
            AvatarPurpose::new(Icons::Unblock, AvatarIconStyle::Success),
        ))
    }

    pub fn message(name: Name) -> Self { Action::Message { name } }

    pub fn get(&self) -> Box<dyn Callback> {
        match self {
            Action::Share { data } => {
                let data = data.clone();
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    ctx.hardware.share(&Data::Text(data.clone()))
                })
            }

            Action::SelectImage => {
                Box::new(move |_ctx: &mut Context, _: &Theme| println!("Selecting image"))
            }

            Action::Custom { action } => {
                let mut action = action.clone();
                Box::new(move |ctx: &mut Context, theme: &Theme| (action)(ctx, theme))
            }

            Action::Flow { flow } => {
                let mut flow = flow.clone();
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    flow.build(ctx, theme)(ctx, theme);
                })
            }

            Action::Paste { id } => {
                let id = *id;
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    let text = match ctx.hardware.get_clipboard() {
                        Some(Data::Text(text)) => text, _ => String::new(),
                    };
                    ctx.emit(TextInputEvent::Set { id, value: text });
                })
            }

            Action::Copy { data } => {
                let data = data.to_string();
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    ctx.hardware.set_clipboard(&Data::Text(data.clone()))
                })
            }

            Action::Message { name } => {
                let recipient = *name;
                Box::new(move |ctx: &mut Context, theme: &Theme| {
                    let instance = ChatRoom::create(ctx, vec![recipient]);
                    let mut flow = Flow::new(vec![Page::messaging(ctx, &mut instance.clone())]);
                    flow.build(ctx, theme)(ctx, theme);
                })
            }

            Action::ChooseSearch { name } => {
                let name = *name;
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    ctx.emit(SearchbarEvent::Select(name))
                })
            }

            Action::Block { name } => {
                let name = *name;
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    let mut block = Block::create(ctx, name);
                    block.send(BlockAction::Block);
                })
            }

            Action::Unblock { name } => {
                let name = *name;
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    let mut block = Block::create(ctx, name);
                    block.send(BlockAction::Unblock);
                })
            }

            _ => Box::new(move |_ctx: &mut Context, _: &Theme| {}),
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
pub struct TableItem {
    title: String,
    data: String,
}

impl TableItem {
    pub fn new(title: &str, data: &str) -> Self {
        TableItem { title: title.to_string(), data: data.to_string() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumItem {
    title: String,
    data: String,
}
impl EnumItem {
    pub fn new(title: &str, data: &str) -> Self {
        EnumItem { title: title.to_string(), data: data.to_string() }
    }
    fn get(&self) -> (&str, &str, Box<dyn Callback>) {
        (&self.title, &self.data, Box::new(|_, _| {}))
    }
}

#[derive(Debug, Clone)]
pub enum AvatarPurpose { None, Custom { icon: Icons, style: AvatarIconStyle } }

impl AvatarPurpose {
    pub fn new(icon: Icons, style: AvatarIconStyle) -> Self {
        AvatarPurpose::Custom { icon, style }
    }
}

#[derive(Clone, Component)]
pub struct AvatarInput {
    layout: Stack,
    avatar: Avatar,
    #[skip]
    id: Id,
    #[skip]
    picked: Option<Picked<Image>>,
    #[skip]
    form: Option<Uuid>,
}

impl AvatarInput {
    fn new(theme: &Theme, content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>) -> Self {
        let id = Id::random();
        let on_click = action.map(|action| match action {
            Action::PickPhoto => {
                Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(PickPhoto(id)))
                    as Box<dyn Callback>
            }
            action => action.get(),
        });
        Self {
            layout: Stack::default(),
            avatar: Avatar::new(theme, content, flair, false, AvatarSize::Xxl, on_click),
            id,
            picked: None,
            form: None,
        }
    }
    pub fn content(&self) -> &AvatarContent { self.avatar.content() }
}

impl std::fmt::Debug for AvatarInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AvatarInput").field("avatar", &self.avatar).finish()
    }
}

#[on_event]
impl OnEvent<PickPhoto> for AvatarInput {
    fn on(&mut self, ctx: &mut Context, event: PickPhoto) -> Vec<Box<dyn DynEvent>> {
        if event.0 == self.id { self.picked = Some(ctx.hardware.pick::<Image>()); }
        vec![]
    }
}

#[on_change(async {
    match &mut self.picked {
        Some(picked) => picked.next().await,
        None => std::future::pending().await,
    }
})]
impl OnChange<prism::hardware::PickResult<Image>> for AvatarInput {
    fn on_change(&mut self, ctx: &mut Context, result: prism::hardware::PickResult<Image>) {
        self.picked = None;
        if let Ok(image) = result {
            self.avatar.set_content(AvatarContent::Image(image));
            if let Some(form) = self.form { ctx.emit(FormEvent::Refresh(form)); }
        }
    }
}

#[on_event]
impl OnEvent<FormEvent> for AvatarInput {
    fn on(&mut self, _ctx: &mut Context, event: FormEvent) -> Vec<Box<dyn DynEvent>> {
        if let FormEvent::Bind(id) = event { self.form = Some(id); }
        prism::events![event]
    }
}

#[derive(Debug, Clone)]
struct PickPhoto(Id);
impl Event for PickPhoto {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children { *child.event = Some(self.clone()); }
    }
}
