#![doc(html_logo_url = "https://raw.githubusercontent.com/ramp-stack/chk/main/logo.png")]

pub mod design_system;

#[cfg(feature = "pelican_ui")]
pub mod pelican_ui;
#[cfg(feature = "pelican_ui")]
pub use pelican_ui::PelicanUi;

pub use prism::Context;
pub use ptsd::utils::{Timestamp, Assets};
pub use crate::design_system::{DesignSystem, Theme, Color, Icons, image};
pub use prism::events::{DynEvent, Event, OnEvent};
pub use ptsd::layout::Offset;
pub use ptsd::{Validate, Validator, Watched};

pub mod invitations;
pub use invitations::{Invitations, InvitationLinks, ContactInvitations};

pub mod listener;
pub use listener::*;
pub mod closure;
pub use closure::*;
pub mod flow;
pub use flow::*;
pub mod form;
pub use form::*;
pub mod items;
pub use items::*;
pub mod page;
pub use page::*;
pub mod air;
pub use air::*;
#[path = "typing.rs"]
mod typing_ui;

extern crate self as chk;

mod pending;

mod app;
pub use app::*;

pub mod design;
pub use design::{Slider, InputControl, TextField, NumberField, ChoiceField, SelectionField, Scanner, Control, ContentSurface, PageSurface, PageView, AvatarSurface, AvatarView, CameraSurface, CameraView, PlaybackSurface, VideoView};
