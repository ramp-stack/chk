use pelican_ui::{Context, theme::{Theme, Icons}, navigation::NavigationEvent};
use prism::air::{Instance, Name};
use ptsd::layout::Offset;
use crate::{Action, ActionItem, AvatarPurpose, Bumper, Display, Flow, Form, FormAction,
    FormComplete, FormItem, FormSubmit, FormValidState, ListItem, Page, PageType, Selector, FormValues, Watch};
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
            ListItem::avatar(data.avatar.clone(), &title, &data.name(), None, Some(Action::flow(flow)))
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
        let edit = active.then(|| (Icons::Edit, Action::deferred_flow(move |_: &mut Context, _: &Theme| {
            Flow::new(vec![Page::Static(Box::new(Self::edit(room.clone())))])
        })));
        PageType::display("Group info", display, edit, Bumper::None, Offset::Start)
    }

    pub fn edit(mut room: Instance<ChatRoom>) -> PageType {
        let original = room.pending().group_details();
        let photo = original.photo.clone();
        let name = original.name.clone();
        let save = Box::new(move |ctx: &mut Context, values: &FormValues| {
            if !room.pending().members.contains(&ctx.air.me()) {return FormComplete::Error("You are no longer a member of this group.".into());}
            if let Some(photo) = values.avatar("photo") && *photo != original.photo {
                room.send(ChatRoomAction::SetGroupPhoto(photo.clone()));
            }
            if let Some(name) = values.text("name") && name.trim() != original.name {
                room.send(ChatRoomAction::RenameGroup(name.to_owned()));
            }
            FormComplete::Action(Action::custom(|ctx: &mut Context, _: &Theme| ctx.emit(NavigationEvent::Pop)))
        }) as Box<dyn FormSubmit>;
        PageType::edit("Edit group", None, vec![
            FormItem::avatar_with_preset("Group photo", photo, Action::PickPhoto,
                |_: &mut Context, _: String| FormValidState::Valid).named("photo"),
            FormItem::text_with_preset("Group name", &name, None, |_: &mut Context, name: String| {
                if name.trim().is_empty() {FormValidState::InvalidWithData("Enter a group name.".into())}
                else if name.trim().chars().count() > 80 {FormValidState::InvalidWithData("Use 80 characters or fewer.".into())}
                else {FormValidState::Valid}
            }).named("name"),
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
        let selector = Selector::new("contacts", items.into_iter()
            .map(|(item, name)| (item, name.to_string())).collect());
        let new_contact = crate::ChatPresets::default().select_contact(theme, selector.clone(), members);
        let submit = Box::new(move |ctx: &mut Context, values: &FormValues| {
            let Some(keys) = values.selection("members") else {return FormComplete::None;};
            let Ok(names) = keys.iter().map(|key| key.parse::<Name>()).collect::<Result<Vec<_>, _>>()
                else {return FormComplete::None;};
            ChatRoom::add_members(ctx, &mut room, names);
            ctx.emit(NavigationEvent::Pop);
            FormComplete::None
        }) as Box<dyn FormSubmit>;
        Form::flow(theme, vec![FormItem::selection("Add members", selector.with_actions(vec![
            FormAction::Custom(ActionItem::new(Action::flow(Flow::from_form(new_contact)), "New contact", Icons::Add)),
        ])).named("members")], None, None, submit)
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
