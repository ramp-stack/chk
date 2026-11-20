use prism::Context;
use prism::air::{Metadata, Contract, Id, Name, Instance};
use ptsd::layout::Offset;
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::closure::FormSubmit;
use crate::design_system::{AvatarContent, Header, Icons, ProfileView, Theme};
use crate::form::{FormItem, FormValues};
use crate::items::{Action, Display, Item};
use crate::{ActionItem, Bumper, ContactInvitations, Flow, FormComplete, FormValidState, Page, PageType};

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum ProfileAction {
    ChangeUsername(String), ChangeNotes(String), ChangeAvatar(AvatarContent),
    SetUsernameInit(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Profile {
    pub name: Option<Name>,
    pub username: String,
    pub notes: String,
    pub avatar: AvatarContent,
    init: bool,
}

impl Contract for Profile {
    type Message = ProfileAction;
    type Init = Name;
    type Result = ();

    fn id() -> Id {Id::hash("Profile")}

    fn init(init: Name, _metadata: Metadata) -> Self {
        Profile {
            name: Some(init), username: "Orange Profile".to_string(), notes: String::new(),
            avatar: AvatarContent::default(), init: false,
        }
    }

    fn apply(&mut self, message: ProfileAction, _metadata: Metadata) {
        match message {
            ProfileAction::ChangeUsername(content) => self.username = content.to_string(),
            ProfileAction::ChangeNotes(content) => self.notes = content.to_string(),
            ProfileAction::ChangeAvatar(content) => self.avatar = content.clone(),
            ProfileAction::SetUsernameInit(content) => {
                if !self.init {
                    self.init = true;
                    self.username = content.to_string();
                }
            }
        }
    }
}

impl Profile {
    pub fn create(ctx: &mut Context, name: Name) -> Instance<Profile> {
        let mut profile = ctx.air.instances::<Profile>().create(name);
        profile.send(ProfileAction::SetUsernameInit(Username::generate()));
        profile
    }

    pub fn me(ctx: &mut Context) -> Instance<Profile> {
        let my_name = ctx.air.me();
        Profile::from_name(ctx, my_name)
    }

    pub fn from_name(ctx: &mut Context, name: Name) -> Instance<Profile> {
        Profile::try_from_name(ctx, name).unwrap_or_else(|| Profile::create(ctx, name))
    }

    pub fn try_from_name(ctx: &mut Context, name: Name) -> Option<Instance<Profile>> {
        ctx.air.instances::<Profile>().list().iter_mut().find_map(|profile| {
            if profile.1.pending().name == Some(name) {Some(profile.1.clone())} else {None}
        })
    }

    pub fn to_view(&self) -> ProfileView {
        ProfileView {
            name: self.name.unwrap(), username: self.username.clone(), pfp: self.avatar.clone(),
        }
    }

    pub fn name(&self) -> String {
        let c: Vec<_> = self.name.unwrap().to_string().chars().collect();
        format!(
            "{}...{}", c[..17].iter().collect::<String>(),
            c[c.len() - 4..].iter().collect::<String>()
        )
    }

    pub fn profile_view(invitations: &ContactInvitations, ctx: &mut Context, profile: Instance<Profile>) -> PageType {
        let name = profile.pending().name.unwrap();
        let title = profile.pending().username.clone();
        let is_me = name == ctx.air.me();
        let items = Display::profile(invitations, ctx, profile.clone())
            .into_iter().map(Item::Display).collect();
        let header = Some((Icons::Edit, Action::deferred_flow(move |ctx: &mut Context, _: &Theme| {
            Flow::new(vec![Page::Static(Box::new(Profile::editing(
                is_me, profile.clone(), ctx.window.is_mobile, None,
            )))])
        })));
        PageType::Content {
            header: Header::stack(
                &title, header.map(|(icon, action)| (icon, action.get())),
            ),
            items, bumper: Bumper::None, offset: Offset::Start,
            flow_len: 1, next: None, lifecycle: None, identity: None, reveal_child: None,
        }
    }

    pub fn editing(is_me: bool, profile: Instance<Profile>, is_mobile: bool, contact_actions: Option<Vec<ActionItem>>) -> PageType {
        let p = (*profile.pending()).clone();
        let mut saved = p.clone();
        let mut profile = profile;
        let closure = Box::new(move |_ctx: &mut Context, objects: &FormValues| {
            if let Some(result) = objects.text("username") && result != saved.username {
                profile.send(ProfileAction::ChangeUsername(result.to_string()));
                saved.username = result.to_string();
            }
            if let Some(result) = objects.text("about") && result != saved.notes {
                profile.send(ProfileAction::ChangeNotes(result.to_string()));
                saved.notes = result.to_string();
            }
            if let Some(result) = objects.avatar("avatar") && *result != saved.avatar {
                profile.send(ProfileAction::ChangeAvatar(result.clone()));
                saved.avatar = result.clone();
            }
            FormComplete::None
        }) as Box<dyn FormSubmit>;

        let my_name = p.name.unwrap();
        let display = if is_me {
            vec![Display::cta(
                "AirName", None, &my_name.to_string(),
                contact_actions.unwrap_or_else(|| vec![
                    ActionItem::new_with_active(
                        Action::copy(&my_name.to_string()), "Copy", "Copied", Icons::Copy,
                    ),
                    ActionItem::new(
                        Action::flow(Flow::new(vec![Page::Static(Box::new(PageType::display_qr_code(
                            "Share AirName", &my_name.to_string(), "Scan to share AirName.",
                            is_mobile,
                        )))])), "Display QR Code", Icons::QrCode,
                    ),
                ]),
            )]
        } else {
            vec![]
        };

        PageType::Edit {
            title: if is_me {"My profile"} else {"Edit profile"}.into(),
            header: None,
            items: vec![
                FormItem::avatar_with_preset(
                    "avatar", "Avatar", p.avatar.clone(), Action::PickPhoto,
                    |_ctx: &mut Context, _: String| FormValidState::Valid,
                ),
                FormItem::text_with_preset(
                    "username", Uuid::new_v4(), "Username", &p.username, None,
                    |_ctx: &mut Context, value: String| {
                        if value.trim().is_empty() {
                            FormValidState::InvalidWithData("Username cannot be empty".to_string())
                        } else {
                            FormValidState::Valid
                        }
                    },
                ),
                FormItem::text_with_preset(
                    "about", Uuid::new_v4(), "About me", &p.notes, None,
                    |_ctx: &mut Context, _: String| FormValidState::Valid,
                ),
            ],
            display, on_save: closure,
        }
    }
}

pub struct Username;
impl Username {
    pub fn generate() -> String {
        let parse = |s: &str| {
            s.lines().map(str::trim).filter(|l| !l.is_empty()).map(|l| l.to_string())
                .collect::<Vec<_>>()
        };
        let mut rng = rand::thread_rng();

        let animals = parse(include_str!("../../usernames/animals.txt"));
        let adjectives = parse(include_str!("../../usernames/adjectives.txt"));
        let foods = parse(include_str!("../../usernames/foods.txt"));
        let noun = if rng.gen_bool(0.5) {&animals} else {&foods};

        let f = |s: &str| {
            let mut o = String::with_capacity(s.len());
            let mut cap = true;
            for c in s.chars() {
                if c == ' ' || c == '-' {
                    cap = true;
                    continue;
                }
                if cap {
                    for u in c.to_uppercase() {o.push(u);}
                    cap = false;
                } else {o.push(c);}
            }
            o
        };

        format!("{}{}", f(adjectives.choose(&mut rng).unwrap()), f(noun.choose(&mut rng).unwrap()))
    }
}
