use super::*;

#[derive(Debug, Clone)]
pub enum Display {
    Text { text: String, size: TextSize, style: TextStyle, align: Align },
    Icon { icon: Icons },
    Image { image: String, size: (f32, f32) },
    Cta { label: String, data: Option<String>, instructions: String, actions: Vec<ActionItem> },
    Table { label: String, items: Vec<ValueItem>, actions: Option<Vec<ActionItem>> },
    DataList { label: String, instructions: Option<String>, items: Vec<ListItem>, actions: Option<Vec<ActionItem>> },
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

    pub fn table(label: &str, items: Vec<ValueItem>, actions: Option<Vec<ActionItem>>) -> Self {
        Display::Table { label: label.to_string(), items, actions }
    }

    /// List rows inside a data item, with optional instructions and quick actions.
    pub fn data_list(label: &str, instructions: Option<&str>, items: Vec<ListItem>, actions: Option<Vec<ActionItem>>) -> Self {
        Display::DataList {
            label: label.to_string(), instructions: instructions.map(str::to_string), items, actions,
        }
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
                                label: ActionItem::format_label(label),
                                active: active.as_deref().map(ActionItem::format_label),
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
                                label: ActionItem::format_label(label),
                                active: active.as_deref().map(ActionItem::format_label),
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
                        .map(|ValueItem { title, data }| (title.clone(), data.clone()))
                        .collect(),
                    actions,
                ))]
            }
            Display::DataList { label, instructions, items, actions } => {
                let actions = actions.as_ref().map(|actions| actions.iter()
                    .map(|ActionItem(action, label, active, icon)| ActionData {
                        label: ActionItem::format_label(label), active: active.as_deref().map(ActionItem::format_label),
                        icon: *icon, on_click: action.get(),
                    }).collect());
                vec![Box::new(DataItem::list(
                    theme, label, instructions.as_deref(),
                    items.iter().map(|item| item.build(theme)).collect(), actions,
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
                let style = match purpose {
                    AvatarPurpose::None => None,
                    AvatarPurpose::Custom { icon, style } => Some((*icon, *style)),
                };

                vec![Box::new(Avatar::new(
                    theme, content.clone(), style, AvatarSize::Xxl, None,
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
                                    label: ActionItem::format_label(label),
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