use super::*;
use crate::{Form, PageType};
use crate::messages::RoomNotificationState;

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

    pub fn group_info(ctx: &mut Context, room: Instance<ChatRoom>) -> Vec<Self> {
        let data = room.pending().clone();
        let details = data.group_details();
        let me = ctx.air.me();
        let active = data.is_group() && data.members.contains(&me);
        let items = data.members.iter().map(|name| {
            let mut profile = Profile::from_name(ctx, *name);
            let data = profile.pending().clone();
            let title = if *name == me {format!("{} (You)", data.username)}
                else {data.username.clone()};
            let flow = Flow::new(vec![Page::profile(ctx, &mut profile)]);
            ListItem::avatar(data.avatar.clone(), &title, &data.name(), None, Some(Action::flow(flow)))
        }).collect();
        let mut display = vec![Display::Avatar {
            content: details.photo.clone(), purpose: AvatarPurpose::None,
        }];
        if active {
            let muted = RoomNotificationState::open(ctx, room.id()).pending().is_muted();
            display.push(Display::Cta {
                label: "Notifications".into(), data: None,
                instructions: if muted {"Notifications are muted for this group."}
                    else {"Notifications are on for this group."}.into(),
                actions: vec![ActionItem::mute(ctx, room.clone())],
            });
            let add_room = room.clone();
            let leave_room = room;
            display.push(Display::DataList {
                label: "Group members".into(),
                instructions: Some(format!("This group has {} members.", data.members.len())),
                items,
                actions: Some(vec![ActionItem::new(Action::custom(move |ctx: &mut Context, theme: &Theme| {
                    let form = Form::add_members(ctx, theme, add_room.clone());
                    (Flow::from_form(form).build(ctx, theme))(ctx, theme);
                }), "Add Members", Icons::Add)]),
            });
            display.push(Display::Cta {
                label: "Leave group".into(), data: None,
                instructions: "Remove yourself from this group.".into(),
                actions: vec![ActionItem::new(Action::custom(move |ctx: &mut Context, theme: &Theme| {
                    (Flow::new(vec![Page::Static(Box::new(PageType::leave(leave_room.clone())))])
                        .build(ctx, theme))(ctx, theme);
                }), "Leave Group", Icons::Block)],
            });
        } else {
            display.push(Display::Text {
                text: "You are no longer a member of this group.".into(),
                size: TextSize::Md, style: TextStyle::Secondary, align: Align::Center,
            });
            display.push(Display::DataList {label: "Members".into(), instructions: None, items, actions: None});
        }
        display
    }

    pub fn profile(ctx: &mut Context, profile: Instance<Profile>) -> Vec<Self> {
        let instance = profile.clone();
        let profile = profile.pending().clone();
        let name = profile.name.unwrap();
        let is_me = name == ctx.air.me();
        let about = if profile.notes.is_empty() {"No bio yet."} else {&profile.notes};
        let blocked = !is_me && Block::did_i_block(ctx, name);
        let blocked_by_them = !is_me && Block::am_i_blocked(ctx, name);
        let mute = ActionItem::mute_profile(ctx, &instance);
        let block = if blocked {
            ActionItem::new(Action::unblock(instance), "Unblock", Icons::Unblock)
        } else {
            ActionItem::new(Action::block(instance), "Block", Icons::Block)
        };
        let actions = vec![ActionItem::new(Action::message(name), "Message", Icons::Messages), block, mute];
        let mut display = Vec::new();

        let notice = if blocked_by_them {Some("This user blocked you.")}
            else if blocked {Some("You blocked this user.")}
            else {None};

        display.extend([
            Display::Avatar {content: profile.avatar.clone(), purpose: AvatarPurpose::None},
            Display::Actions {actions, as_icons: true},
        ]);
        if let Some(text) = notice {display.push(Display::Alert {text: text.into()});}
        display.extend([
            Display::Cta {label: "About me".into(), data: None, instructions: about.into(), actions: vec![]},
            Display::Cta {
                label: "AirName".into(), data: None, instructions: name.to_string(),
                actions: vec![
                    ActionItem::new_with_active(Action::copy(&name.to_string()), "Copy", "Copied", Icons::Copy),
                    ActionItem::new(Action::flow(Flow::new(vec![Page::Static(Box::new(
                        PageType::display_qr_code(
                            "Share AirName", &name.to_string(), "Scan to share AirName.", ctx.window.is_mobile,
                        ),
                    ))])), "Display QR Code", Icons::QrCode),
                ],
            },
        ]);
        display
    }

    pub fn build(&mut self, theme: &Theme) -> Option<Vec<Box<dyn Drawable>>> {
        Some(match self {
            Display::Alert {text} => vec![theme.alert_view(text)],
            Display::Icon {icon} => vec![theme.icon_view(*icon, 128.0)],
            Display::Image {image, size} => vec![theme.image_view(image, size.0)],
            Display::Text {text, size, style, align} if !text.is_empty() => vec![theme.text_view(text, *size, *style, *align)],
            Display::Cta {label, data, instructions, actions} => vec![theme.data_text(label, data.as_deref(), instructions,
                Some(actions.iter().map(ActionItem::presentation).collect()))],
            Display::Table {label, items, actions} => vec![theme.data_table(label,
                items.iter().map(|item| (item.title.clone(), item.data.clone())).collect(),
                actions.as_ref().map(|actions| actions.iter().map(ActionItem::presentation).collect()))],
            Display::DataList {label, instructions, items, actions} => vec![theme.data_list(label, instructions.as_deref(),
                items.clone(),
                actions.as_ref().map(|actions| actions.iter().map(ActionItem::presentation).collect()))],
            Display::Currency {amount, instructions} => vec![theme.currency_view(amount.clone(), instructions)],
            Display::Number {number, instructions} => vec![theme.number_view(number.clone(), instructions)],
            Display::List {label, items, instructions} => vec![theme.list_section(label.clone(), instructions.clone(),
                items.clone())],
            Display::QRCode {data, instructions} => vec![theme.qr_code(data),
                theme.text_view(instructions, TextSize::Md, TextStyle::Secondary, Align::Center)],
            Display::Avatar {content, purpose} => {
                let flair = match purpose {AvatarPurpose::None => None, AvatarPurpose::Custom {icon, style} => Some((*icon, *style))};
                vec![theme.avatar_display(content.clone(), flair)]
            }
            Display::Actions {actions, as_icons} if *as_icons => {
                vec![theme.icon_actions(actions.iter().map(ActionItem::icon_action).collect())]
            }
            Display::Actions {actions, ..} => vec![theme.quick_actions(Offset::Start,
                actions.iter().map(|action| {let mut item = action.presentation(); item.active = None; item}).collect())],
            Display::Messages {messages} => vec![messages.clone()],
            _ => return None,
        })
    }
}
