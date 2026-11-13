use super::*;

#[derive(Debug, Clone)]
pub enum Display {
    Text { text: String, size: TextSize, style: TextStyle, align: Align },
    Alert { text: String },
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
    pub fn alert(text: &str) -> Self {Display::Alert { text: text.to_string() }}

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

    pub fn messages(ctx: &mut Context, theme: &Theme, messages: Vec<MessageView>) -> Self {
        Display::Messages { messages: theme.message_group(ctx, messages, false) }
    }

    pub fn build(&mut self, theme: &Theme) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Self::Alert {text} => vec![theme.alert_view(text)],
            Self::Icon {icon} => vec![theme.icon_view(*icon, 128.0)],
            Self::Image {image, size} => vec![theme.image_view(image, size.0)],
            Self::Text {text, size, style, align} if !text.is_empty() => vec![theme.text_view(text, *size, *style, *align)],
            Self::Cta {label, data, instructions, actions} => vec![theme.data_text(label, data.as_deref(), instructions,
                Some(actions.iter().map(ActionItem::presentation).collect()))],
            Self::Table {label, items, actions} => vec![theme.data_table(label,
                items.iter().map(|item| (item.title.clone(), item.data.clone())).collect(),
                actions.as_ref().map(|actions| actions.iter().map(ActionItem::presentation).collect()))],
            Self::DataList {label, instructions, items, actions} => vec![theme.data_list(label, instructions.as_deref(),
                items.clone(),
                actions.as_ref().map(|actions| actions.iter().map(ActionItem::presentation).collect()))],
            Self::Currency {amount, instructions} => vec![theme.currency_view(amount.clone(), instructions)],
            Self::Number {number, instructions} => vec![theme.number_view(number.clone(), instructions)],
            Self::List {label, items, instructions} => vec![theme.list_section(label.clone(), instructions.clone(),
                items.clone())],
            Self::QRCode {data, instructions} => vec![theme.qr_code(data),
                theme.text_view(instructions, TextSize::Md, TextStyle::Secondary, Align::Center)],
            Self::Avatar {content, purpose} => {
                let flair = match purpose {AvatarPurpose::None => None, AvatarPurpose::Custom {icon, style} => Some((*icon, *style))};
                vec![theme.avatar_display(content.clone(), flair)]
            }
            Self::Actions {actions, as_icons} if *as_icons => vec![theme.icon_actions(actions.iter().map(|ActionItem(action, _, _, icon)| (*icon, action.get())).collect())],
            Self::Actions {actions, ..} => vec![theme.quick_actions(Offset::Start,
                actions.iter().map(|action| {let mut item = action.presentation(); item.active = None; item}).collect())],
            Self::Messages {messages} => vec![messages.clone()],
            _ => return None,
        })
    }
}
