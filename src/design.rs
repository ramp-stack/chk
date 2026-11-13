use prism::{Context, drawable::{Drawable, LayoutTree, Instruction, Rect}};
use prism::events::{DynEvent, ListenFuture, ListenCallback};
use uuid::Uuid;
use crate::{State, FormValidState};
use crate::design_system::{Theme, PlaybackBadge, AvatarContent};
use ptsd::content::Content;
use prism::hardware::{Image, Player};

pub trait Slider: Drawable {fn value(&self) -> f32;}

pub trait InputControl: Drawable {
    fn state(&self) -> State;
    fn validation_value(&self) -> Option<String> {None}
    fn feedback(&mut self, _result: &FormValidState) {}
}
dyn_clone::clone_trait_object!(InputControl);

pub trait TextField: InputControl {
    fn value(&self) -> String;
    fn id(&self) -> Uuid;
    fn focused(&self) -> bool;
}
pub trait NumberField: InputControl {fn value(&self) -> String;}
pub trait ChoiceField: InputControl {fn value(&self) -> usize;}
pub trait SelectionField: InputControl {fn value(&self) -> Vec<String>;}
pub trait Scanner: InputControl {fn value(&self) -> Option<String>;}

#[derive(Clone, Debug)]
pub struct Control(pub Box<dyn InputControl>);
impl Control {
    pub fn new(control: impl InputControl + 'static) -> Self {Self(Box::new(control))}
    pub fn boxed(control: impl InputControl + 'static) -> Box<dyn Drawable> {Box::new(Self(Box::new(control)))}
    pub fn state(&self) -> State {self.0.state()}
}

macro_rules! delegate_drawable {
    ($($view:ident),+ $(,)?) => {$(
        impl Drawable for $view {
            fn request_width(&mut self) -> (f32, f32) {self.0.request_width()}
            fn request_height(&mut self, width: f32) -> (f32, f32) {self.0.request_height(width)}
            fn layout(&mut self, size: (f32, f32)) -> LayoutTree {self.0.layout(size)}
            fn draw(&self, layout: &LayoutTree, offset: (f32, f32), bound: Rect) -> Vec<Instruction> {self.0.draw(layout, offset, bound)}
            fn event(&mut self, ctx: &mut Context, layout: &LayoutTree, event: Box<dyn DynEvent>) {self.0.event(ctx, layout, event);}
            fn changed(&mut self) -> ListenFuture<'_> {
                Box::pin(async move {
                    let callback = self.0.changed().await;
                    Box::new(move |target: &mut dyn std::any::Any, ctx: &mut Context| {
                        let target = target.downcast_mut::<Self>().unwrap();
                        callback(target.0.as_mut().as_any_mut(), ctx);
                    }) as ListenCallback
                })
            }
        }

    )+};
}

pub trait ContentSurface: Drawable + Clone {
    fn id(&self) -> Uuid;
    fn children(&self) -> &Vec<Box<dyn Drawable>>;
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Drawable>>;
    fn scroll(&self) -> f32;
    fn set_scroll(&mut self, value: f32);
}

impl ContentSurface for Content {
    fn id(&self) -> Uuid {std::ops::Deref::deref(self).id()}
    fn children(&self) -> &Vec<Box<dyn Drawable>> {std::ops::Deref::deref(self).children()}
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Drawable>> {std::ops::DerefMut::deref_mut(self).children_mut()}
    fn scroll(&self) -> f32 {self.children.scroll()}
    fn set_scroll(&mut self, value: f32) {self.children.set_scroll(value);}
}

pub trait PageSurface: Drawable + dyn_clone::DynClone {
    fn content(&self) -> &Content;
    fn content_mut(&mut self) -> &mut Content;
    fn replace_content(&mut self, content: Content);
    fn disable_actions(&mut self, disabled: bool);
    fn on_primary(&mut self, action: Box<dyn ptsd::utils::Callback>);
    fn composer_state(&mut self) -> Option<(Uuid, bool)>;
    fn take_footer_from(&mut self, old: &mut dyn PageSurface);
}
dyn_clone::clone_trait_object!(PageSurface);

#[derive(Clone, Debug)]
pub struct PageView(pub Box<dyn PageSurface>);
impl PageView {
    pub fn new(widget: impl PageSurface + 'static) -> Self {Self(Box::new(widget))}
    pub fn content(&self) -> &Content {self.0.content()}
    pub fn content_mut(&mut self) -> &mut Content {self.0.content_mut()}
    pub fn replace_content(&mut self, content: Content) {self.0.replace_content(content);}
    pub fn disable_actions(&mut self, disabled: bool) {self.0.disable_actions(disabled);}
    pub fn on_primary(&mut self, action: Box<dyn ptsd::utils::Callback>) {self.0.on_primary(action);}
    pub fn composer_state(&mut self) -> Option<(Uuid, bool)> {self.0.composer_state()}
    pub fn take_footer_from(&mut self, old: &mut Self) {self.0.take_footer_from(old.0.as_mut());}
}

pub trait AvatarSurface: Drawable + dyn_clone::DynClone {
    fn content(&self) -> &AvatarContent;
    fn set_content(&mut self, content: AvatarContent);
}
dyn_clone::clone_trait_object!(AvatarSurface);

#[derive(Clone, Debug)]
pub struct AvatarView(pub Box<dyn AvatarSurface>);
impl AvatarView {
    pub fn new(widget: impl AvatarSurface + 'static) -> Self {Self(Box::new(widget))}
    pub fn content(&self) -> &AvatarContent {self.0.content()}
    pub fn set_content(&mut self, content: AvatarContent) {self.0.set_content(content);}
}

pub trait CameraSurface: Drawable + dyn_clone::DynClone {
    fn reset(&mut self, theme: &Theme);
    fn error(&mut self, theme: &Theme, message: &str, disabled: bool);
    fn disable(&mut self, disabled: bool);
    fn preview(&mut self, image: Image);
}
dyn_clone::clone_trait_object!(CameraSurface);

#[derive(Clone, Debug)]
pub struct CameraView(pub Box<dyn CameraSurface>);
impl CameraView {
    pub fn new(widget: impl CameraSurface + 'static) -> Self {Self(Box::new(widget))}
    pub fn reset(&mut self, theme: &Theme) {self.0.reset(theme);}
    pub fn error(&mut self, theme: &Theme, message: &str, disabled: bool) {self.0.error(theme, message, disabled);}
    pub fn disable(&mut self, disabled: bool) {self.0.disable(disabled);}
    pub fn preview(&mut self, image: Image) {self.0.preview(image);}
}

pub trait PlaybackSurface: Drawable + dyn_clone::DynClone {
    fn set_player(&mut self, player: Player, size: (u32, u32));
    fn progress(&mut self, value: f32);
    fn badge(&mut self, theme: &Theme, badge: PlaybackBadge);
    fn playing(&mut self, theme: &Theme, playing: bool);
    fn position(&mut self, theme: &Theme, seconds: u64);
    fn duration(&mut self, theme: &Theme, seconds: u64);
}
dyn_clone::clone_trait_object!(PlaybackSurface);

#[derive(Clone, Debug)]
pub struct VideoView(pub Box<dyn PlaybackSurface>);
impl VideoView {
    pub fn new(widget: impl PlaybackSurface + 'static) -> Self {Self(Box::new(widget))}
    pub fn set_player(&mut self, player: Player, size: (u32, u32)) {self.0.set_player(player, size);}
    pub fn progress(&mut self, value: f32) {self.0.progress(value);}
    pub fn badge(&mut self, theme: &Theme, badge: PlaybackBadge) {self.0.badge(theme, badge);}
    pub fn playing(&mut self, theme: &Theme, playing: bool) {self.0.playing(theme, playing);}
    pub fn position(&mut self, theme: &Theme, seconds: u64) {self.0.position(theme, seconds);}
    pub fn duration(&mut self, theme: &Theme, seconds: u64) {self.0.duration(theme, seconds);}
}

delegate_drawable!(Control, PageView, AvatarView, CameraView, VideoView);
