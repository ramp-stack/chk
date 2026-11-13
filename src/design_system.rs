//! Design-independent presentation data and constructor mappings.
//! An application supplies its design system; CHK keeps its controllers and flows.
pub use prism::Context;
pub use ptsd::{colors, Color, TextSize, image};
pub use ptsd::utils::{Assets, Timestamp, ValidationFn};
pub use ptsd::icons::Icons;
pub use ptsd::avatar::{AvatarContent, AvatarIconStyle, AvatarValue};
pub use ptsd::number::Number;
pub use ptsd::media::{
    Photo, Video, VideoClip, Sticker, StickerPack, Gif, MessageContent, SystemMessage,
    SendEvent, AttachmentEvent,
};
pub use ptsd::task::Task;
pub use ptsd::content::{Content, FormEvent};
pub use ptsd::input::{TextInputEvent, MessageBarEvent};
use prism::air::Name;
use prism::drawable::{Drawable, text::Align};
use ptsd::layout::Offset;
use ptsd::Validate;
use uuid::Uuid;
use std::{any::Any, fmt::Debug, future::Future, pin::Pin, rc::Rc, sync::Arc};
pub use crate::design::{PageView, AvatarView, CameraView, VideoView};
pub use crate::Selector;

pub type GifBytes = Pin<Box<dyn Future<Output = Result<Arc<[u8]>, String>>>>;
pub type ListItemView = Box<dyn Drawable>;
pub type MessageGroups = Box<dyn Drawable>;
pub type TypingIndicator = Box<dyn Drawable>;
pub type PhotoPageView = Box<dyn Drawable>;

#[derive(Clone, Copy, Debug)]
pub enum TextStyle {Heading, Primary, Secondary, Error, Keyboard, Label(Color)}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrubPhase {Start, Move, End}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackAction {Toggle, Scrub(f32, ScrubPhase)}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackBadge {Loading, Unplayable, Play, None}

macro_rules! callback {
    ($name:ident, $($signature:tt)+) => {
        pub trait $name: $($signature)+ + 'static {
            fn clone_box(&self) -> Box<dyn $name>;
        }
        impl<F: $($signature)+ + Clone + 'static> $name for F {
            fn clone_box(&self) -> Box<dyn $name> {Box::new(self.clone())}
        }
        impl Clone for Box<dyn $name> {
            fn clone(&self) -> Self {self.as_ref().clone_box()}
        }
        impl PartialEq for dyn $name {
            fn eq(&self, _: &Self) -> bool {true}
        }
        impl Debug for dyn $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(stringify!($name))
            }
        }
    };
}

callback!(Callback, FnMut(&mut Context, &Theme));
callback!(QrCodeFound, FnMut(&mut Context, String));
callback!(PlaybackCallback, FnMut(&mut Context, PlaybackAction));
callback!(SendFn, FnMut(&mut Context, &mut String));

pub trait ValidFn: for<'a> Validate<&'a mut String, bool> {
    fn clone_box(&self) -> Box<dyn ValidFn>;
}
impl<V: for<'a> Validate<&'a mut String, bool> + Clone> ValidFn for V {
    fn clone_box(&self) -> Box<dyn ValidFn> {Box::new(self.clone())}
}
impl Clone for Box<dyn ValidFn> {
    fn clone(&self) -> Self {self.as_ref().clone_box()}
}
impl PartialEq for dyn ValidFn {
    fn eq(&self, _: &Self) -> bool {true}
}
impl Debug for dyn ValidFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {f.write_str("ValidFn")}
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileView {
    pub name: Name,
    pub username: String,
    pub pfp: AvatarContent,
}

#[derive(Debug, Clone)]
pub struct MessageView {
    pub body: MessageContent,
    pub timestamp: Timestamp,
    pub author: ProfileView,
    pub on_click: Box<dyn Callback>,
}

#[derive(Debug, Clone)]
pub enum Header {
    Stack {title: String, action: Option<(Icons, Box<dyn Callback>)>},
    End {title: String},
    Home {title: String, action: Option<(Icons, Box<dyn Callback>)>},
    Leading {title: String, action: Option<(Icons, Box<dyn Callback>)>},
    Messaging {
        title: String, profiles: Vec<ProfileView>, avatar: Option<AvatarContent>,
        steps: usize, info: Option<Box<dyn Callback>>,
    },
}

impl Header {
    pub fn stack(title: &str, action: Option<(Icons, Box<dyn Callback>)>) -> Self {
        Header::Stack {title: title.into(), action}
    }
    pub fn stack_end(title: &str) -> Self {Header::End {title: title.into()}}
    pub fn home(title: &str, action: Option<(Icons, Box<dyn Callback>)>) -> Self {
        Header::Home {title: title.into(), action}
    }
    pub fn home_leading(title: &str, action: Option<(Icons, Box<dyn Callback>)>) -> Self {
        Header::Leading {title: title.into(), action}
    }
    pub fn messaging(title: String, profiles: Vec<ProfileView>, avatar: Option<AvatarContent>,
        steps: usize, info: Option<Box<dyn Callback>>) -> Self {
        Header::Messaging {title, profiles, avatar, steps, info}
    }
}

#[derive(Debug, Clone)]
pub enum Footer {
    Stack {
        label: Option<String>, action: Box<dyn Callback>,
        secondary: Option<(String, Box<dyn Callback>)>,
    },
    End {steps: Option<usize>},
    Home {first: Option<(String, Box<dyn Callback>)>, second: Option<(String, Box<dyn Callback>)>},
    Alert {notice: String},
    Input {placeholder: String, on_send: Box<dyn SendFn>, valid: Box<dyn ValidFn>},
}

impl Footer {
    pub fn stack(label: Option<&str>, action: Box<dyn Callback>,
        secondary: Option<(String, Box<dyn Callback>)>) -> Self {
        Footer::Stack {label: label.map(str::to_owned), action, secondary}
    }
    pub fn stack_end(steps: Option<usize>) -> Self {Footer::End {steps}}
    pub fn home(first: Option<(String, Box<dyn Callback>)>, second: Option<(String, Box<dyn Callback>)>) -> Self {
        Footer::Home {first, second}
    }
    pub fn alert(notice: &str) -> Self {Footer::Alert {notice: notice.into()}}
    pub fn input(placeholder: &str, on_send: impl SendFn, valid: impl ValidFn) -> Self {
        Footer::Input {placeholder: placeholder.into(), on_send: Box::new(on_send), valid: Box::new(valid)}
    }
}

#[derive(Debug, Clone)]
pub struct Theme(Rc<dyn DesignSystem>);

impl Theme {
    pub fn new(backend: impl DesignSystem) -> Self {Self(Rc::new(backend))}
    pub fn image(&self, name: &str) -> Option<prism::hardware::Image> {self.0.image(name)}
    pub fn with_image(&self, name: &str, image: prism::hardware::Image) -> Self {
        Self(Rc::from(self.0.with_image(name, image)))
    }
    pub fn downcast_ref<D: DesignSystem>(&self) -> Option<&D> {
        (self.0.as_ref() as &dyn Any).downcast_ref()
    }
    pub fn colors(&self) -> &ptsd::ColorResources {self.0.colors()}
    pub fn fonts(&self) -> &ptsd::FontResources {self.0.fonts()}
    pub fn configure_gifs(&self, key: &str) {self.0.configure_gifs(key);}
    pub fn gif_bytes(&self, gif: &Gif) -> GifBytes {self.0.gif_bytes(gif)}
}

macro_rules! constructors {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> $result:ty;)+) => {
        pub trait DesignSystem: Debug + Any {
            fn new(config: crate::ChkTheme, assets: Assets) -> Self where Self: Sized;
            fn colors(&self) -> &ptsd::ColorResources;
            fn image(&self, name: &str) -> Option<prism::hardware::Image>;
            fn with_image(&self, name: &str, image: prism::hardware::Image) -> Box<dyn DesignSystem>;
            fn fonts(&self) -> &ptsd::FontResources;
            fn configure_gifs(&self, key: &str);
            fn gif_bytes(&self, gif: &Gif) -> GifBytes;
            $(fn $name(&self, theme: &Theme, $($arg: $ty),*) -> $result;)+
        }

        impl Theme {
            $(pub fn $name(&self, $($arg: $ty),*) -> $result {
                self.0.$name(self, $($arg),*)
            })+
        }
    };
}

constructors! {
    fn interface(ctx: &mut Context, roots: Vec<crate::RootInfo>, keyboard: Option<Box<dyn Drawable>>) -> Box<dyn Drawable>;
    fn page(header: Header, content: Content, footer: Option<Footer>) -> PageView;
    fn slider_input(start: f32, label: Option<&str>, description: Option<&str>) -> Box<dyn Drawable>;
    fn text_input(id: Uuid, preset: Option<&str>, label: Option<&str>, placeholder: &str) -> Box<dyn Drawable>;
    fn choice_input(index: usize, items: Vec<(&str, &str, Box<dyn Callback>)>) -> Box<dyn Drawable>;
    fn currency_input(instructions: &str) -> Box<dyn Drawable>;
    fn number_input(instructions: &str) -> Box<dyn Drawable>;
    fn date_input(instructions: &str) -> Box<dyn Drawable>;
    fn time_input(instructions: &str) -> Box<dyn Drawable>;
    fn keypad() -> Box<dyn Drawable>;
    fn selection_input(selector: Selector) -> Box<dyn Drawable>;
    fn search_input(id: Uuid, label: &str, items: Vec<(crate::ListItem, Name)>, actions: Vec<crate::ActionPresentation>) -> Box<dyn Drawable>;
    fn scanner_input(on_find: Box<dyn QrCodeFound>) -> Box<dyn Drawable>;
    fn text_view(text: &str, size: TextSize, style: TextStyle, align: Align) -> Box<dyn Drawable>;
    fn alert_view(text: &str) -> Box<dyn Drawable>;
    fn icon_view(icon: Icons, size: f32) -> Box<dyn Drawable>;
    fn image_view(name: &str, width: f32) -> Box<dyn Drawable>;
    fn data_text(label: &str, data: Option<&str>, instructions: &str, actions: Option<Vec<crate::ActionPresentation>>) -> Box<dyn Drawable>;
    fn data_table(label: &str, items: Vec<(String, String)>, actions: Option<Vec<crate::ActionPresentation>>) -> Box<dyn Drawable>;
    fn data_list(label: &str, instructions: Option<&str>, items: Vec<crate::ListItem>, actions: Option<Vec<crate::ActionPresentation>>) -> Box<dyn Drawable>;
    fn currency_view(amount: Number, instructions: &str) -> Box<dyn Drawable>;
    fn number_view(number: Number, instructions: &str) -> Box<dyn Drawable>;
    fn list_section(label: Option<String>, instructions: Option<String>, items: Vec<crate::ListItem>) -> Box<dyn Drawable>;
    fn qr_code(data: &str) -> Box<dyn Drawable>;
    fn avatar_display(content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>) -> Box<dyn Drawable>;
    fn quick_actions(offset: Offset, actions: Vec<crate::ActionPresentation>) -> Box<dyn Drawable>;
    fn icon_actions(actions: Vec<(Icons, Box<dyn Callback>)>) -> Box<dyn Drawable>;
    fn list_item(item: &crate::ListItem) -> ListItemView;
    fn avatar_input(content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Box<dyn Callback>>) -> AvatarView;
    fn photo_page(photo: Photo, header: Option<(Icons, Box<dyn Callback>)>) -> PhotoPageView;
    fn video_poster(photo: &Photo, header: Option<(Icons, Box<dyn Callback>)>, action: Box<dyn PlaybackCallback>) -> VideoView;
    fn video_player(title: &str, player: prism::hardware::Player, size: (u32, u32), action: Box<dyn PlaybackCallback>) -> VideoView;
    fn camera_view(capture: Box<dyn Callback>) -> CameraView;
    fn message_group(ctx: &mut Context, messages: Vec<MessageView>, group: bool) -> MessageGroups;
    fn typing_indicator(name: Option<&str>) -> TypingIndicator;
}
