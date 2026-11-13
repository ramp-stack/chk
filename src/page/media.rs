use prism::Context;
use prism::drawable::Component;
use prism::hardware::Image;
use ptsd::layout::Stack;
use ptsd::navigation::AppPage;

use crate::design_system::Photo;
use crate::design_system::Theme;

#[derive(Debug, Component, Clone)]
pub struct PhotoPage(Stack, crate::design_system::PhotoPageView);
impl AppPage for PhotoPage {}
impl PhotoPage {
    pub fn new(_ctx: &mut Context, theme: &Theme, photo: Photo, header: Option<(crate::Icons, crate::Action)>) -> Self {
        Self(Stack::fill(), theme.photo_page(photo, header.map(|(icon, action)| (icon, action.get()))))
    }
}

pub trait PhotoCaptured: dyn_clone::DynClone {
    fn captured(&mut self, ctx: &mut Context, photo: Photo);
}
impl<F: FnMut(&mut Context, Photo) + Clone> PhotoCaptured for F {
    fn captured(&mut self, ctx: &mut Context, photo: Photo) {self(ctx, photo);}
}
dyn_clone::clone_trait_object!(PhotoCaptured);
impl std::fmt::Debug for dyn PhotoCaptured {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {f.write_str("PhotoCaptured")}
}

/// Live camera capture; the caller chooses what to do with the resulting photo.
#[derive(Debug, Component, Clone)]
pub struct CameraPage {
    layout: Stack,
    page: crate::design_system::CameraView,
    #[skip] camera: Option<prism::hardware::Camera>,
    #[skip] latest: Option<Image>,
    #[skip] encoding: Option<crate::design_system::Task<Option<Photo>>>,
    #[skip] start: prism::events::Timer,
    #[skip] token: uuid::Uuid,
    #[skip] theme: Theme,
    #[skip] on_capture: Box<dyn PhotoCaptured>,
    #[skip] failed: bool,
    #[skip] focused: bool,
    #[skip] finished: bool,
}
impl AppPage for CameraPage {}

#[derive(Debug, Clone, Copy)]
struct CapturePhoto(uuid::Uuid);
impl prism::events::Event for CapturePhoto {
    fn pass(&self, _: &mut Context, children: &mut [prism::events::ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(*self);}
    }
}

impl CameraPage {
    pub fn new(theme: &Theme, on_capture: Box<dyn PhotoCaptured>) -> Self {
        let token = uuid::Uuid::new_v4();
        let mut page = Self {
            layout: Stack::fill(),
            page: theme.camera_view(Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(CapturePhoto(token)))),
            camera: None, latest: None,
            encoding: None, start: prism::events::Timer::new(std::time::Duration::ZERO),
            token, theme: theme.clone(), on_capture, failed: false, focused: true, finished: false,
        };
        page.page.disable(true);
        page
    }

    pub fn open(&mut self, ctx: &mut Context) {
        self.failed = false;
        self.latest = None;
        self.page.reset(&self.theme);
        match ctx.hardware.start_camera() {
            Ok(camera) => self.camera = Some(camera),
            Err(error) => self.error(&error),
        }
    }

    pub fn error(&mut self, message: &str) {
        self.camera = None;
        self.latest = None;
        self.failed = true;
        self.page.error(&self.theme, message, !self.focused);
    }
}

#[prism::events::on_change(self.start.next())]
impl prism::events::OnChange<()> for CameraPage {
    fn on_change(&mut self, ctx: &mut Context, _: ()) {
        if self.focused && !self.finished && self.encoding.is_none() {self.open(ctx);}
    }
}

#[prism::events::on_change(async {
    use std::{future::Future, pin::pin, task::{Context, Poll, Waker}};
    let Some(camera) = &mut self.camera else {return std::future::pending().await};
    let mut frame = camera.next().await?;
    // Use recent frames without letting a stalled preview monopolize the UI.
    for _ in 0..8 {
        let mut next = pin!(camera.next());
        match next.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(Some(next)) => frame = next, _ => break,
        }
    }
    Some(frame)
})]
impl prism::events::OnChange<Option<prism::hardware::VideoFrame>> for CameraPage {
    fn on_change(&mut self, _: &mut Context, frame: Option<prism::hardware::VideoFrame>) {
        if !self.focused || self.finished || self.encoding.is_some() {return;}
        match frame {
            Some(frame) if !frame.corrupted && frame.image.dimensions.0 > 0 && frame.image.dimensions.1 > 0 => {
                self.page.preview(frame.image.clone());
                self.latest = Some(frame.image);
                self.page.disable(false);
            }
            Some(_) => {},
            None => {
                let error = self.camera.as_ref().and_then(prism::hardware::Camera::error)
                    .unwrap_or_else(|| "The camera stopped. Please try again.".into());
                self.error(&error);
            }
        }
    }
}

#[prism::events::on_event]
impl prism::events::OnEvent<CapturePhoto> for CameraPage {
    fn on(&mut self, ctx: &mut Context, event: CapturePhoto) -> Vec<Box<dyn prism::events::DynEvent>> {
        if event.0 != self.token || !self.focused || self.finished || self.encoding.is_some() {return vec![];}
        if self.failed {self.open(ctx);}
        else if let Some(image) = self.latest.take() {
            self.camera = None;
            self.page.disable(true);
            self.encoding = Some(crate::design_system::Task::spawn(move || Photo::from_image(&image)));
        }
        vec![]
    }
}

#[prism::events::on_change(async {
    match &mut self.encoding {
        Some(task) => task.next().await.flatten(),
        None => std::future::pending().await,
    }
})]
impl prism::events::OnChange<Option<Photo>> for CameraPage {
    fn on_change(&mut self, ctx: &mut Context, photo: Option<Photo>) {
        self.encoding = None;
        if self.finished {return;}
        match photo {
            Some(photo) => {
                self.finished = true;
                let mut on_capture = self.on_capture.clone();
                // Restore the conversation before delivering the attachment.
                ctx.emit(ptsd::navigation::NavigationEvent::reset_with_fn(move |ctx: &mut Context| {
                    on_capture.captured(ctx, photo.clone());
                }));
            }
            None => self.error("This photo could not be captured. Please try again."),
        }
    }
}

#[prism::events::on_event]
impl prism::events::OnEvent<prism::events::WindowEvent> for CameraPage {
    fn on(&mut self, _: &mut Context, event: prism::events::WindowEvent) -> Vec<Box<dyn prism::events::DynEvent>> {
        if let prism::events::WindowEvent::Focused(focused) = event {
            self.focused = focused;
            if !focused {
                self.camera = None;
                self.latest = None;
                self.start.cancel();
                self.page.disable(true);
            } else if !self.finished && self.encoding.is_none() && self.camera.is_none() {
                self.start.reset(std::time::Duration::ZERO);
            }
        }
        prism::events![event]
    }
}

#[prism::events::on_event]
impl prism::events::OnEvent<ptsd::navigation::NavigationEvent> for CameraPage {
    fn on(&mut self, _: &mut Context, event: ptsd::navigation::NavigationEvent) -> Vec<Box<dyn prism::events::DynEvent>> {
        if matches!(event, ptsd::navigation::NavigationEvent::Pop | ptsd::navigation::NavigationEvent::Root(_)
            | ptsd::navigation::NavigationEvent::Reset | ptsd::navigation::NavigationEvent::ResetWithFunction(_)) {
            self.camera = None;
            self.encoding = None;
            self.start.cancel();
            self.finished = true;
        }
        prism::events![event]
    }
}
