use prism::{Context, drawable::{Drawable, LayoutTree, Instruction, Rect}};
use prism::events::{ChangeHandler, DynEvent, ListenFuture, ListenCallback, Timer, WindowEvent};
use prism::hardware::{Image, Player};
use ptsd::content::Content;
use uuid::Uuid;

use std::{
    collections::BTreeMap, future::{Future, ready}, ops::Range, pin::Pin, rc::Rc,
    task::{Context as TaskContext, Poll, Waker}, time::Duration,
};

use crate::{State, FormValidState, FormValues, Watcher};
use crate::design_system::{Theme, PlaybackBadge, AvatarContent, PageEvent};

type PageTask = crate::pending::Pending<Rc<dyn Fn(&mut Context, &mut PageView)>>;

pub trait Slider: Drawable {fn value(&self) -> f32;}

pub trait TextField: InputControl {
    fn value(&self) -> String;
    fn id(&self) -> Uuid;
    fn focused(&self) -> bool;
}
pub trait NumberField: InputControl {fn value(&self) -> String;}
pub trait ChoiceField: InputControl {fn value(&self) -> usize;}
pub trait SelectionField: InputControl {fn value(&self) -> Vec<String>;}
pub trait Scanner: InputControl {fn value(&self) -> Option<String>;}

pub trait InputControl: Drawable {
    fn state(&self) -> State;
    fn validation_value(&self) -> Option<String> {None}
    fn feedback(&mut self, _result: &FormValidState) {}
}
dyn_clone::clone_trait_object!(InputControl);

pub trait PageSurface: Drawable + dyn_clone::DynClone {
    fn content(&self) -> &Content;
    fn content_mut(&mut self) -> &mut Content;
    fn replace_content(&mut self, content: Content);
    fn disable_actions(&mut self, disabled: bool);
    fn composer_state(&mut self) -> Option<(Uuid, bool)>;
    fn take_footer_from(&mut self, old: &mut dyn PageSurface);
}
dyn_clone::clone_trait_object!(PageSurface);

pub trait AvatarSurface: Drawable + dyn_clone::DynClone {
    fn content(&self) -> &AvatarContent;
    fn set_content(&mut self, content: AvatarContent);
}
dyn_clone::clone_trait_object!(AvatarSurface);

pub trait CameraSurface: Drawable + dyn_clone::DynClone {
    fn reset(&mut self, theme: &Theme);
    fn error(&mut self, theme: &Theme, message: &str, disabled: bool);
    fn disable(&mut self, disabled: bool);
    fn preview(&mut self, image: Image);
}
dyn_clone::clone_trait_object!(CameraSurface);

pub trait PlaybackSurface: Drawable + dyn_clone::DynClone {
    fn set_player(&mut self, player: Player, size: (u32, u32));
    fn progress(&mut self, value: f32);
    fn badge(&mut self, theme: &Theme, badge: PlaybackBadge);
    fn playing(&mut self, theme: &Theme, playing: bool);
    fn position(&mut self, theme: &Theme, seconds: u64);
    fn duration(&mut self, theme: &Theme, seconds: u64);
}
dyn_clone::clone_trait_object!(PlaybackSurface);

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
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Drawable>> {
        std::ops::DerefMut::deref_mut(self).children_mut()
    }
    fn scroll(&self) -> f32 {self.children.scroll()}
    fn set_scroll(&mut self, value: f32) {self.children.set_scroll(value);}
}

macro_rules! delegate_drawable {
    ($($view:ident),+ $(,)?) => {$(
        impl Drawable for $view {
            fn request_width(&mut self) -> (f32, f32) {self.0.request_width()}
            fn request_height(&mut self, width: f32) -> (f32, f32) {self.0.request_height(width)}
            fn layout(&mut self, size: (f32, f32)) -> LayoutTree {self.0.layout(size)}
            fn draw(&self, layout: &LayoutTree, offset: (f32, f32), bound: Rect) -> Vec<Instruction> {
                self.0.draw(layout, offset, bound)
            }
            fn event(&mut self, ctx: &mut Context, layout: &LayoutTree, event: Box<dyn DynEvent>) {
                self.0.event(ctx, layout, event);
            }
            fn changed(&mut self) -> ListenFuture<'_> {
                Box::pin(async move {
                    let callback = self.0.changed().await;
                    Box::new(move |target: &mut dyn std::any::Any, ctx: &mut Context| {
                        let target = target.downcast_mut::<$view>().unwrap();
                        callback(target.0.as_mut().as_any_mut(), ctx);
                    }) as ListenCallback
                })
            }
        }

    )+};
}

#[derive(Clone, Debug)]
pub struct Control(pub Box<dyn InputControl>);
impl Control {
    pub fn new(control: impl InputControl + 'static) -> Self {Self(Box::new(control))}
    pub fn boxed(control: impl InputControl + 'static) -> Box<dyn Drawable> {
        Box::new(Self(Box::new(control)))
    }
    pub fn state(&self) -> State {self.0.state()}
}

delegate_drawable!(Control);

#[derive(Clone, Debug)]
pub struct PageView {
    surface: Box<dyn PageSurface>,
    identity: Option<String>,
    events: Option<Box<dyn PageEvent>>,
    activation: Option<Box<dyn ptsd::utils::Callback>>,
    activate: bool,
    focused: bool,
    timeout: Option<(Timer, Box<dyn ptsd::utils::Callback>)>,
    fields: Vec<(String, Range<usize>)>,
    tasks: BTreeMap<Uuid, PageTask>,
}
impl PageView {
    /// Runs `on_create` once after installing the page state and handlers; clones do not rerun it.
    #[allow(clippy::too_many_arguments, reason = "Page setup is supplied directly at construction.")]
    pub fn new(
        ctx: &mut Context, mut widget: impl PageSurface + 'static,
        fields: Vec<(String, Range<usize>)>, disabled: bool, identity: Option<String>,
        on_event: Option<Box<dyn PageEvent>>, on_show: Option<Box<dyn ptsd::utils::Callback>>,
        on_create: Option<ListenCallback<PageView>>,
    ) -> Self {
        widget.disable_actions(disabled);
        let mut page = Self {
            surface: Box::new(widget), identity, events: on_event, activate: on_show.is_some(),
            activation: on_show, focused: true, timeout: None, fields, tasks: BTreeMap::new(),
        };
        if let Some(on_create) = on_create {on_create(&mut page, ctx);}
        page
    }
    pub fn content(&self) -> &Content {self.surface.content()}
    pub fn content_mut(&mut self) -> &mut Content {self.surface.content_mut()}
    pub fn replace_content(&mut self, content: Content) {self.surface.replace_content(content);}
    pub fn disable_actions(&mut self, disabled: bool) {self.surface.disable_actions(disabled);}
    pub fn composer_state(&mut self) -> Option<(Uuid, bool)> {self.surface.composer_state()}
    pub fn take_footer_from(&mut self, old: &mut Self) {
        self.surface.take_footer_from(old.surface.as_mut());
    }
    pub fn focused(&self) -> bool {self.focused}
    pub fn values(&self) -> FormValues {FormValues::from_content(self.content(), &self.fields)}

    /// Cloned pages share the operation's result; each page delivers its completion once.
    /// Canceling or dropping a page releases only that page's ownership.
    pub fn spawn<T: Clone + 'static>(
        &mut self, future: impl Future<Output = T> + 'static,
        complete: impl Fn(&mut Context, &mut PageView, T) + 'static,
    ) -> Uuid {
        let id = Uuid::new_v4();
        self.tasks.insert(id, crate::pending::Pending::new(async move {
            let result = future.await;
            Rc::new(move |ctx: &mut Context, page: &mut PageView| complete(ctx, page, result.clone()))
                as Rc<dyn Fn(&mut Context, &mut PageView)>
        }));
        id
    }
    pub fn pending(&self, id: Uuid) -> bool {self.tasks.contains_key(&id)}
    pub fn cancel(&mut self, id: Uuid) {self.tasks.remove(&id);}

    pub fn watch<W: Watcher>(&mut self, ctx: &mut Context, mut watcher: W, update: impl Fn(&mut Context, &mut PageView, W::Output) + Clone + 'static) {
        watcher.get(ctx);
        let mut waiting = watcher.clone();
        // Establish the subscription before taking the final snapshot, including while hidden.
        let mut future = Box::pin(async move {(waiting.next().await, waiting)});
        let future: Pin<Box<dyn Future<Output = (Option<()>, W)>>> =
            match future.as_mut().poll(&mut TaskContext::from_waker(Waker::noop())) {
                Poll::Ready(result) => Box::pin(ready(result)), Poll::Pending => future,
            };
        let next = update.clone();
        self.spawn(future, move |ctx, page, (changed, watcher)| {
            if changed.is_some() {page.watch(ctx, watcher, next.clone());}
        });
        let value = watcher.get(ctx);
        update(ctx, self, value);
    }
    pub fn handle_event(&mut self, ctx: &mut Context, event: &dyn DynEvent) -> bool {
        if let Some(WindowEvent::Focused(focused)) = event.downcast_ref() {
            self.focused = *focused;
            self.activate = *focused && self.activation.is_some();
        } else if event.downcast_ref::<ptsd::navigation::NavigationChanged>().is_some() {
            self.focused = true;
            self.activate = self.activation.is_some();
        }
        let Some(mut handler) = self.events.take() else {return false;};
        let consumed = handler(ctx, self, event);
        self.events = Some(handler);
        consumed
    }
    pub fn after(&mut self, duration: Duration, callback: impl ptsd::utils::Callback) {
        self.timeout = Some((Timer::new(duration), Box::new(callback)));
    }

    pub fn retain(&mut self, ctx: &mut Context, old: &mut Self) {
        if self.identity.is_none() || self.identity != old.identity {return;}
        self.focused = old.focused;
        self.content_mut().retain_scroll_from(old.content());
        if self.composer_state().is_some() && old.composer_state().is_some() {
            self.take_footer_from(old);
            self.events = old.events.take();
            self.timeout = old.timeout.take();
            self.tasks = std::mem::take(&mut old.tasks);
        } else if let Some(mut handler) = old.events.take() {
            old.focused = false;
            handler(ctx, old, &WindowEvent::Focused(false));
        }
    }
}

impl Drawable for PageView {
    fn request_width(&mut self) -> (f32, f32) {self.surface.request_width()}
    fn request_height(&mut self, width: f32) -> (f32, f32) {self.surface.request_height(width)}
    fn layout(&mut self, size: (f32, f32)) -> LayoutTree {self.surface.layout(size)}
    fn draw(&self, layout: &LayoutTree, offset: (f32, f32), bound: Rect) -> Vec<Instruction> {
        self.surface.draw(layout, offset, bound)
    }
    fn event(&mut self, ctx: &mut Context, layout: &LayoutTree, event: Box<dyn DynEvent>) {
        if self.handle_event(ctx, event.as_ref()) {return;}
        self.surface.event(ctx, layout, event);
    }
    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            if self.activate && self.focused {
                return Box::new(|target: &mut dyn std::any::Any, ctx: &mut Context| {
                    let target = target.downcast_mut::<PageView>().unwrap();
                    target.activate = false;
                    if let Some(callback) = &mut target.activation {callback(ctx);}
                }) as ListenCallback;
            }
            let view: ListenFuture<'_, PageView> = Box::pin(async {
                let callback = self.surface.changed().await;
                Box::new(move |target: &mut PageView, ctx: &mut Context| {
                    callback(target.surface.as_mut().as_any_mut(), ctx);
                }) as ListenCallback<PageView>
            });
            let timeout: ListenFuture<'_, PageView> = Box::pin(async {
                match &mut self.timeout {
                    Some((timer, _)) => timer.next().await,
                    None => std::future::pending().await,
                }
                Box::new(|target: &mut PageView, ctx: &mut Context| {
                    if let Some((_, mut callback)) = target.timeout.take() {callback(ctx);}
                }) as ListenCallback<PageView>
            });
            let tasks: ListenFuture<'_, PageView> = Box::pin(async {
                let waits = self.tasks.iter_mut().map(|(id, task)| {
                    let id = *id;
                    Box::pin(async move {
                        let complete = task.await;
                        Box::new(move |page: &mut PageView, ctx: &mut Context| {
                            if page.tasks.remove(&id).is_some() {complete(ctx, page);}
                        }) as ListenCallback<PageView>
                    }) as ListenFuture<'_, PageView>
                }).collect();
                ChangeHandler::select(waits).await
            });
            let callback = ChangeHandler::select(vec![view, timeout, tasks]).await;
            Box::new(move |target: &mut dyn std::any::Any, ctx: &mut Context| {
                callback(target.downcast_mut::<PageView>().unwrap(), ctx);
            }) as ListenCallback
        })
    }
}

#[derive(Clone, Debug)]
pub struct AvatarView(pub Box<dyn AvatarSurface>);
impl AvatarView {
    pub fn new(widget: impl AvatarSurface + 'static) -> Self {Self(Box::new(widget))}
    pub fn content(&self) -> &AvatarContent {self.0.content()}
    pub fn set_content(&mut self, content: AvatarContent) {self.0.set_content(content);}
}

delegate_drawable!(AvatarView);

#[derive(Clone, Debug)]
pub struct CameraView(pub Box<dyn CameraSurface>);
impl CameraView {
    pub fn new(widget: impl CameraSurface + 'static) -> Self {Self(Box::new(widget))}
    pub fn reset(&mut self, theme: &Theme) {self.0.reset(theme);}
    pub fn error(&mut self, theme: &Theme, message: &str, disabled: bool) {
        self.0.error(theme, message, disabled);
    }
    pub fn disable(&mut self, disabled: bool) {self.0.disable(disabled);}
    pub fn preview(&mut self, image: Image) {self.0.preview(image);}
}

delegate_drawable!(CameraView);

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
delegate_drawable!(VideoView);
