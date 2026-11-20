use prism::Context;
use prism::drawable::Component;
use prism::events::{
    ChildEvent, DynEvent, Event, OnChange, OnEvent, Timer, WindowEvent, on_change, on_event,
};
use prism::hardware::{Camera, Image, VideoFrame};
use ptsd::layout::Stack;
use ptsd::navigation::NavigationEvent;
use uuid::Uuid;

use std::{future::Future, pin::pin, task::{Context as TaskContext, Poll, Waker}, time::Duration};

use crate::design_system::{CameraView, Photo, Task, Theme};

pub trait PhotoCaptured: dyn_clone::DynClone {
    fn captured(&mut self, ctx: &mut Context, photo: Photo);
}
impl<F: FnMut(&mut Context, Photo) + Clone> PhotoCaptured for F {
    fn captured(&mut self, ctx: &mut Context, photo: Photo) {self(ctx, photo);}
}
dyn_clone::clone_trait_object!(PhotoCaptured);
impl std::fmt::Debug for dyn PhotoCaptured {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PhotoCaptured")
    }
}

/// Live camera capture; the caller chooses what to do with the resulting photo.
#[derive(Debug, Component, Clone)]
pub struct CameraCapture {
    layout: Stack,
    view: CameraView,
    #[skip] camera: Option<Camera>,
    #[skip] latest: Option<Image>,
    #[skip] encoding: Option<Task<Option<Photo>>>,
    #[skip] start: Timer,
    #[skip] token: Uuid,
    #[skip] theme: Theme,
    #[skip] on_capture: Box<dyn PhotoCaptured>,
    #[skip] failed: bool,
    #[skip] focused: bool,
    #[skip] finished: bool,
}

impl CameraCapture {
    pub fn new(theme: &Theme, on_capture: Box<dyn PhotoCaptured>) -> Self {
        let token = Uuid::new_v4();
        CameraCapture {
            layout: Stack::fill(),
            view: theme.camera_view(true, Box::new(move |ctx: &mut Context, _: &Theme| {
                ctx.emit(CapturePhoto(token));
            })),
            camera: None, latest: None,
            encoding: None, start: Timer::new(Duration::ZERO),
            token, theme: theme.clone(), on_capture, failed: false, focused: true, finished: false,
        }
    }

    pub fn open(&mut self, ctx: &mut Context) {
        self.failed = false;
        self.latest = None;
        self.view.reset(&self.theme);
        match ctx.hardware.start_camera() {
            Ok(camera) => self.camera = Some(camera),
            Err(error) => self.error(&error),
        }
    }

    pub fn error(&mut self, message: &str) {
        self.camera = None;
        self.latest = None;
        self.failed = true;
        self.view.error(&self.theme, message, !self.focused);
    }
}

#[on_change(self.start.next())]
impl OnChange<()> for CameraCapture {
    fn on_change(&mut self, ctx: &mut Context, _: ()) {
        if self.focused && !self.finished && self.encoding.is_none() {self.open(ctx);}
    }
}

#[on_change(async {
    let Some(camera) = &mut self.camera else {return std::future::pending().await};
    let mut frame = camera.next().await?;
    // Use recent frames without letting a stalled preview monopolize the UI.
    for _ in 0..8 {
        let mut next = pin!(camera.next());
        match next.as_mut().poll(&mut TaskContext::from_waker(Waker::noop())) {
            Poll::Ready(Some(next)) => frame = next, _ => break,
        }
    }
    Some(frame)
})]
impl OnChange<Option<VideoFrame>> for CameraCapture {
    fn on_change(&mut self, _: &mut Context, frame: Option<VideoFrame>) {
        if !self.focused || self.finished || self.encoding.is_some() {return;}
        match frame {
            Some(frame) if !frame.corrupted
                && frame.image.dimensions.0 > 0 && frame.image.dimensions.1 > 0 => {
                self.view.preview(frame.image.clone());
                self.latest = Some(frame.image);
                self.view.disable(false);
            }
            Some(_) => {},
            None => {
                let error = self.camera.as_ref().and_then(Camera::error)
                    .unwrap_or_else(|| "The camera stopped. Please try again.".into());
                self.error(&error);
            }
        }
    }
}

#[on_event]
impl OnEvent<CapturePhoto> for CameraCapture {
    fn on(&mut self, ctx: &mut Context, event: CapturePhoto) -> Vec<Box<dyn DynEvent>> {
        if event.0 != self.token || !self.focused || self.finished || self.encoding.is_some() {
            return vec![];
        }
        if self.failed {self.open(ctx);}
        else if let Some(image) = self.latest.take() {
            self.camera = None;
            self.view.disable(true);
            self.encoding = Some(Task::spawn(move || Photo::from_image(&image)));
        }
        vec![]
    }
}

#[on_change(async {
    match &mut self.encoding {
        Some(task) => task.next().await.flatten(),
        None => std::future::pending().await,
    }
})]
impl OnChange<Option<Photo>> for CameraCapture {
    fn on_change(&mut self, ctx: &mut Context, photo: Option<Photo>) {
        self.encoding = None;
        if self.finished {return;}
        match photo {
            Some(photo) => {
                self.finished = true;
                self.on_capture.captured(ctx, photo);
            }
            None => self.error("This photo could not be captured. Please try again."),
        }
    }
}

#[on_event]
impl OnEvent<WindowEvent> for CameraCapture {
    fn on(&mut self, _: &mut Context, event: WindowEvent) -> Vec<Box<dyn DynEvent>> {
        if let WindowEvent::Focused(focused) = event {
            self.focused = focused;
            if !focused {
                self.camera = None;
                self.latest = None;
                self.start.cancel();
                self.view.disable(true);
            } else if !self.finished && self.encoding.is_none() && self.camera.is_none() {
                self.start.reset(Duration::ZERO);
            }
        }
        prism::events![event]
    }
}

#[on_event]
impl OnEvent<NavigationEvent> for CameraCapture {
    fn on(&mut self, _: &mut Context, event: NavigationEvent) -> Vec<Box<dyn DynEvent>> {
        if matches!(event, NavigationEvent::Pop | NavigationEvent::Root(_)
            | NavigationEvent::Reset | NavigationEvent::ResetWithFunction(_)) {
            self.camera = None;
            self.encoding = None;
            self.start.cancel();
            self.finished = true;
        }
        prism::events![event]
    }
}

#[derive(Debug, Clone, Copy)]
struct CapturePhoto(Uuid);
impl Event for CapturePhoto {
    fn pass(&self, _: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(*self);}
    }
}
