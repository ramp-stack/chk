use pelican_ui::{Context, theme::{Theme, Icons}, navigation::NavigationEvent};
use prism::air::{Instance, Name};
use ptsd::layout::Offset;
use crate::{Action, ActionItem, AvatarPurpose, Bumper, Display, Flow, Form, FormAction,
    FormComplete, FormItem, FormSubmit, FormValidState, ListItem, Page, PageType, State, Watch};
use crate::messages::{ChatRoom, ChatRoomAction};
use crate::profiles::Profile;

pub struct GroupMessageInfoPage;
impl GroupMessageInfoPage {
    pub fn page(room: Instance<ChatRoom>) -> Page {
        Page::watching((Watch::contract(room), Watch::list::<Profile>()),
            |ctx: &mut Context, _: &Theme, (room, _): (Instance<ChatRoom>, Vec<Instance<Profile>>)| {
                Self::build(ctx, room)
            })
    }

    pub fn build(ctx: &mut Context, room: Instance<ChatRoom>) -> PageType {
        let data = room.pending().clone();
        let details = data.group_details();
        let me = ctx.air.me();
        let active = data.is_group() && data.members.contains(&me);
        let items = data.members.iter().map(|name| {
            let mut profile = Profile::from_name(ctx, *name);
            let data = profile.pending().clone();
            let title = if *name == me {format!("{} (You)", data.username)} else {data.username.clone()};
            let flow = Flow::new(vec![Page::profile(ctx, &mut profile)]);
            ListItem::avatar(data.avatar.clone(), &title, &data.name(), None, Some(flow))
        }).collect();
        let mut display = vec![
            Display::avatar(details.photo.clone(), AvatarPurpose::None),
        ];
        if active {
            let add_room = room.clone();
            let leave_room = room.clone();
            display.push(Display::data_list("Group members", Some(&format!("This group has {} members.", data.members.len())), items, Some(vec![
                ActionItem::new(Action::custom(move |ctx: &mut Context, theme: &Theme| {
                    let form = Self::add_members(ctx, theme, add_room.clone());
                    (Flow::from_form(form).build(ctx, theme))(ctx, theme);
                }), "Add Members", Icons::Add),
            ])));
            display.push(Display::cta("Leave group", None, "Remove yourself from this group.", vec![
                ActionItem::new(Action::custom(move |ctx: &mut Context, theme: &Theme| {
                    (Flow::new(vec![Page::Static(Box::new(Self::leave(leave_room.clone())))])
                        .build(ctx, theme))(ctx, theme);
                }), "Leave Group", Icons::Block),
            ]));
        } else {
            display.push(Display::instructions("You are no longer a member of this group."));
            display.push(Display::data_list("Members", None, items, None));
        }
        let edit = active.then(|| (Icons::Edit, Box::new(move |_: &mut Context, _: &Theme| {
            Flow::new(vec![Page::Static(Box::new(Self::edit(room.clone())))])
        }) as Box<dyn crate::closure::FlowBuilder>));
        PageType::display("Group info", display, edit, Bumper::None, Offset::Start)
    }

    pub fn edit(mut room: Instance<ChatRoom>) -> PageType {
        let original = room.pending().group_details();
        let photo = original.photo.clone();
        let name = original.name.clone();
        let save = Box::new(move |ctx: &mut Context, values: &Vec<State>| {
            if !room.pending().members.contains(&ctx.air.me()) {return FormComplete::None;}
            if let Some(State::Avatar(photo)) = values.first() && *photo != original.photo {
                room.send(ChatRoomAction::SetGroupPhoto(photo.clone()));
            }
            if let Some(State::Text(name)) = values.get(1) && name.trim() != original.name {
                room.send(ChatRoomAction::RenameGroup(name.clone()));
            }
            ctx.emit(NavigationEvent::Pop);
            FormComplete::None
        }) as Box<dyn FormSubmit>;
        PageType::edit_and_display("Edit group", None, vec![
            FormItem::avatar_with_preset("Group photo", photo, Action::PickPhoto,
                |_: &mut Context, _: String| FormValidState::Valid),
            FormItem::text_with_preset("Group name", &name, None, |_: &mut Context, name: String| {
                if name.trim().is_empty() {FormValidState::InvalidWithData("Enter a group name.".into())}
                else if name.trim().chars().count() > 80 {FormValidState::InvalidWithData("Use 80 characters or fewer.".into())}
                else {FormValidState::Valid}
            }),
        ], vec![], save)
    }

    pub fn add_members(ctx: &mut Context, theme: &Theme, mut room: Instance<ChatRoom>) -> Form {
        let members = room.pending().members.clone();
        let mut items: Vec<_> = ctx.air.instances::<Profile>().list().into_values().filter_map(|profile| {
            let profile = profile.pending();
            let name = profile.name?;
            if members.contains(&name) {return None;}
            Some((ListItem::avatar(profile.avatar.clone(), &profile.username, &profile.name(), None, None), name))
        }).collect();
        items.sort_by_key(|(_, name)| *name);
        items.dedup_by_key(|(_, name)| *name);
        let new_contact = Self::new_contact(theme, members);
        let submit = Box::new(move |ctx: &mut Context, values: &Vec<State>| {
            if let Some(State::Search(names)) = values.first() {
                ChatRoom::add_members(ctx, &mut room, names.clone());
            }
            ctx.emit(NavigationEvent::Pop);
            FormComplete::None
        }) as Box<dyn FormSubmit>;
        Form::flow(theme, vec![FormItem::search("Add members", "profile name", items,
            Some(vec![FormAction::Custom("New contact".into(), None, Icons::Add,
                Action::flow(Flow::from_form(new_contact)))]))], None, None, submit)
    }

    fn new_contact(theme: &Theme, members: Vec<Name>) -> Form {
        let submit = Box::new(|ctx: &mut Context, values: &Vec<State>| {
            let Some(State::Text(value)) = values.first() else {return FormComplete::None;};
            let Ok(name) = value.trim().parse::<Name>() else {return FormComplete::None;};
            Profile::from_name(ctx, name);
            FormComplete::Return(Box::new(move |ctx: &mut Context, theme: &Theme| {
                (Action::choose_search(name).get())(ctx, theme)
            }))
        }) as Box<dyn FormSubmit>;
        Form::flow(theme, vec![FormItem::text("New contact", "orange name", Some(vec![
            FormAction::Paste, FormAction::scan_qr_code("Scan a profile QR code"),
        ]), move |_: &mut Context, value: String| {
            match value.trim().parse::<Name>() {
                Ok(name) if members.contains(&name) => FormValidState::InvalidWithData("Already in this group.".into()),
                Ok(_) => FormValidState::Valid,
                Err(_) => FormValidState::InvalidWithData("Enter a valid Orange name.".into()),
            }
        })], None, None, submit)
    }

    pub fn leave(mut room: Instance<ChatRoom>) -> PageType {
        let details = room.pending().group_details();
        PageType::display("Leave group", vec![
            Display::avatar(details.photo, AvatarPurpose::None),
            Display::confirmation_message(&format!("Leave {}?", details.name)),
            Display::instructions("The group will leave your message list. A member can add you again."),
        ], None, Bumper::double("Leave group", Action::custom(move |ctx: &mut Context, _: &Theme| {
            room.send(ChatRoomAction::LeaveGroup);
            room.notify(false);
            ctx.emit(NavigationEvent::Root(None));
        }), "Cancel", Action::None), Offset::Center)
    }
}
