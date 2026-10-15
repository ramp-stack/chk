use ramp::prism;

use pelican_ui::event::{OnEvent, Event, TickEvent};
use pelican_ui::drawable::{Component, Drawable, SizedTree};
use pelican_ui::{Context, Callback, drawables};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::canvas::Align;
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::AppPage;
use pelican_ui::components::text::{ExpandableText, TextSize, TextStyle};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::components::{MessageGroups, SendStickerEvent, SendGifEvent, Sticker};
use pelican_ui::navigation::{NavigationEvent};
use pelican_ui::utils::Timestamp;
use pelican_ui::components::{SendPhotoEvent, SendVideoEvent};

use crate::messages::{ChatRoom, Message, ChatRoomAction, MessageContent};
use crate::profiles::{Profile, ProfileAction};
use crate::blocking::Block;
use crate::{Page, PageBuilder, ActionItem, PageType, FormItem, Bumper, Listener};
use crate::flow::Flow;
use crate::form::{State, FormValidState, FormComplete};
use crate::items::{Action, Display, AvatarPurpose, TableItem};
use crate::closure::{FormSubmit, NavFn, ReviewItemGetter, SuccessGetter, FlowBuilder};

use maverick_os::air::{Id, Name, Instance};

use std::str::FromStr;
use std::sync::Arc;

pub struct ProfilePage;
impl ProfilePage {
    pub fn editing(theme: &Theme, is_me: bool, mut profile: Instance<Profile>) -> PageType {
        let mut p = profile.clone();
        let closure = Box::new(move |ctx: &mut Context, objects: &Vec<State>| {
            println!("Saving profile");
            if let Some(State::Text(result)) = objects.get(1) {
                p.send(ProfileAction::ChangeUsername(result.to_string()));
            }
            if let Some(State::Text(result)) = objects.get(2) {
                p.send(ProfileAction::ChangeNotes(result.to_string()));
            }
            if let Some(State::Avatar(result)) = objects.get(0) {
                p.send(ProfileAction::ChangeAvatar(result.clone()));
            }
            FormComplete::None
        }) as Box<dyn FormSubmit>;
        
        let mut avatar = profile.clone();
        let mut username = profile.clone();
        let mut notes = profile.clone();
        let p = (*profile.pending()).clone();
        let my_name = p.name.unwrap();
        let title = if is_me {"My profile"} else {"Edit profile"};
        let display = if is_me {vec![
            Display::cta("Orange name", None, &my_name.to_string(), vec![
                ActionItem::new_with_active(Action::copy(&my_name.to_string()), "Copy", "Copied", Icons::Copy),
                ActionItem::new(Action::flow(Flow::new(vec![
                    Page::Static(PageType::display_qr_code("Share name", &my_name.to_string(), "Scan to share orange name."))
                ])), "Display QR Code", Icons::QrCode)
            ]),
        ]} else {vec![]};
        
        PageType::edit_and_display(title, None,
            // Some((Icons::AddUser, Box::new(move |ctx: &mut Context, theme: &Theme| {
            //     let p = profile.clone(); // CREATE NEW
            //     let t = theme.clone();
            //     Flow::new(vec![Page::Static(ProfilePage::editing(&t.clone(), true, p.clone()))])
            // }))),
            vec![
                FormItem::avatar_with_preset("Avatar", p.avatar.clone(), move |ctx: &mut Context, a: String| {
                    let current = avatar.pending().avatar.get_image().unwrap_or_default();
                    match current == a {
                        true => FormValidState::Valid,
                        false => FormValidState::Invalid,
                    }
                }),
                FormItem::text_with_preset("Username", &p.username.clone(), None, move |ctx: &mut Context, a: String| {
                    match a.as_str() {
                        a if a == &username.pending().username => FormValidState::Valid,
                        "" => FormValidState::InvalidWithData("Username cannot be empty".to_string()),
                        _ => FormValidState::Invalid,
                    }
                }),
                FormItem::text_with_preset("About me", &p.notes, None, move |ctx: &mut Context, a: String| {
                    match notes.pending().notes == a {
                        true => FormValidState::Valid,
                        false => FormValidState::Invalid,
                    }
                })
            ], 
            display,
            closure
        )
    }
}

// impl ProfileView {
//     Listener::new(ctx, theme, listener_p, profile, page, |ctx: &mut Context, theme: &Theme, page: PageType| page.build(ctx, theme))
// }

pub struct ProfileView;
impl ProfileView {
    pub fn new(ctx: &mut Context, theme: &Theme, mut profile: Instance<Profile>) -> PageType {
        let p = profile.clone();
        let profile = profile.pending().clone();
        let profile_name = profile.name.unwrap();
        let saved = p.clone();

        let my_profile = Profile::me(ctx);
        let my_profile = my_profile.pending().clone();
        let my_name = ctx.me();
        let is_me = profile_name == my_name;
        let about_me = if profile.notes.is_empty() {"No bio yet."} else {&profile.notes};
        
        let is_blocked = !is_me && Block::did_i_block(ctx, profile.name.unwrap());
        
        let block_action = match is_blocked {
            true => ActionItem::new(Action::unblock(&theme, p.clone()), "Unblock", Icons::Unblock),
            false => ActionItem::new(Action::block(&theme, p.clone()), "Block", Icons::Block),
        };

        PageType::display(&profile.username,
            vec![
                Display::avatar(profile.avatar.clone(), AvatarPurpose::None),
                Display::actions(vec![
                    // ActionItem::new(Action::None, "Bitcoin", Icons::Bitcoin),
                    ActionItem::new(Action::message(profile_name), "Message", Icons::Messages),
                    block_action
                ], true),
                Display::cta("About me", None, about_me, vec![]),
                Display::cta("Orange name", None, &my_name.to_string(), vec![
                    ActionItem::new_with_active(Action::copy(&my_name.to_string()), "Copy", "Copied", Icons::Copy),
                    ActionItem::new(Action::flow(Flow::new(vec![
                        Page::Static(PageType::display_qr_code("Share name", &my_name.to_string(), "Scan to share orange name."))
                    ])), "Display QR Code", Icons::QrCode)
                ]),
            ], 
            Some((Icons::Edit, Box::new(move |ctx: &mut Context, theme: &Theme| {
                let p = p.clone();
                let t = theme.clone();
                Flow::new(vec![Page::Static(ProfilePage::editing(&t.clone(), is_me, p.clone()))])
            }))), 
            Bumper::None, Offset::Start,
        )
    }
}