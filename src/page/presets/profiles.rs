use pelican_ui::Context;
use ptsd::layout::Offset;
use pelican_ui::theme::{Theme, Icons};
use prism::air::Instance;

use crate::profiles::{Profile, ProfileAction};
use crate::blocking::Block;
use crate::{Page, ActionItem, PageType, FormItem, Bumper};
use crate::flow::Flow;
use crate::form::{State, FormValidState, FormComplete};
use crate::items::{Action, Display, AvatarPurpose};
use crate::closure::FormSubmit;

pub struct ProfilePage;
impl ProfilePage {
    pub fn editing(is_me: bool, profile: Instance<Profile>, is_mobile: bool) -> PageType {
        let mut p = profile.clone();
        let closure = Box::new(move |_ctx: &mut Context, objects: &Vec<State>| {
            if let Some(State::Text(result)) = objects.get(1) {
                p.send(ProfileAction::ChangeUsername(result.to_string()));
            }
            if let Some(State::Text(result)) = objects.get(2) {
                p.send(ProfileAction::ChangeNotes(result.to_string()));
            }
            if let Some(State::Avatar(result)) = objects.first() {
                p.send(ProfileAction::ChangeAvatar(result.clone()));
            }
            FormComplete::None
        }) as Box<dyn FormSubmit>;

        let p = (*profile.pending()).clone();
        let my_name = p.name.unwrap();
        let title = if is_me { "My profile" } else { "Edit profile" };
        let display = if is_me {
            vec![Display::cta(
                "Orange name", None, &my_name.to_string(),
                vec![
                    ActionItem::new_with_active(
                        Action::copy(&my_name.to_string()), "Copy", "Copied", Icons::Copy,
                    ),
                    ActionItem::new(
                        Action::flow(Flow::new(vec![Page::Static(Box::new(PageType::display_qr_code(
                            "Share name", &my_name.to_string(), "Scan to share orange name.",
                            is_mobile,
                        )))])), "Display QR Code", Icons::QrCode,
                    ),
                ],
            )]
        } else {
            vec![]
        };

        PageType::edit_and_display(
            title, None,
            vec![
                FormItem::avatar_with_preset(
                    "Avatar", p.avatar.clone(), Action::PickPhoto,
                    |_ctx: &mut Context, _: String| FormValidState::Valid,
                ),
                FormItem::text_with_preset(
                    "Username", &p.username, None,
                    |_ctx: &mut Context, value: String| {
                        if value.trim().is_empty() {
                            FormValidState::InvalidWithData("Username cannot be empty".to_string())
                        } else {
                            FormValidState::Valid
                        }
                    },
                ),
                FormItem::text_with_preset(
                    "About me", &p.notes, None,
                    |_ctx: &mut Context, _: String| FormValidState::Valid,
                ),
            ], display, closure,
        )
    }
}

pub struct ProfileView;
impl ProfileView {
    pub fn build(ctx: &mut Context, profile: Instance<Profile>) -> PageType {
        let p = profile.clone();
        let profile = profile.pending().clone();
        let profile_name = profile.name.unwrap();
        let my_name = ctx.air.me();
        let is_me = profile_name == my_name;
        let about_me = if profile.notes.is_empty() { "No bio yet." } else { &profile.notes };

        let is_blocked = !is_me && Block::did_i_block(ctx, profile.name.unwrap());

        let block_action = match is_blocked {
            true => ActionItem::new(Action::unblock(p.clone()), "Unblock", Icons::Unblock),
            false => ActionItem::new(Action::block(p.clone()), "Block", Icons::Block),
        };

        PageType::display(
            &profile.username,
            vec![
                Display::avatar(profile.avatar.clone(), AvatarPurpose::None),
                Display::actions(
                    vec![
                        ActionItem::new(Action::message(profile_name), "Message", Icons::Messages),
                        block_action,
                    ], true,
                ), Display::cta("About me", None, about_me, vec![]),
                Display::cta(
                    "Orange name", None, &profile_name.to_string(),
                    vec![
                        ActionItem::new_with_active(
                            Action::copy(&profile_name.to_string()), "Copy", "Copied", Icons::Copy,
                        ),
                        ActionItem::new(
                            Action::flow(Flow::new(vec![Page::Static(Box::new(PageType::display_qr_code(
                                "Share name", &profile_name.to_string(),
                                "Scan to share orange name.", ctx.window.is_mobile,
                            )))])), "Display QR Code", Icons::QrCode,
                        ),
                    ],
                ),
            ],
            Some((
                Icons::Edit,
                Box::new(move |ctx: &mut Context, _: &Theme| {
                    Flow::new(vec![Page::Static(Box::new(ProfilePage::editing(
                        is_me, p.clone(), ctx.window.is_mobile,
                    )))])
                }),
            )), Bumper::None, Offset::Start,
        )
    }
}
