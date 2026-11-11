use pelican_ui::Context;
use pelican_ui::components::{Icon, Photo, Rectangle};
use pelican_ui::components::text::{Text, TextSize, TextStyle};
use pelican_ui::interface::general::{Header, FullScreen};
use pelican_ui::theme::{Color, Icons, Theme};
use prism::drawable::{Component, text::Align};
use prism::hardware::Image;
use ptsd::layout::{Column, Offset, Padding, Size, Stack};
use ptsd::navigation::AppPage;

use crate::Action;

#[derive(Debug, Component, Clone)]
pub struct PhotoPage(Stack, FullScreen<FittedPhoto>);
impl AppPage for PhotoPage {}

impl PhotoPage {
    pub fn new(_ctx: &mut Context, theme: &Theme, photo: Photo, header: Option<(Icons, Action)>) -> Self {
        let icon = header.map(|(icon, action)| (icon, action.get()));
        let header = Header::stack(theme, "Photo", icon);
        Self(Stack::fill(), FullScreen::without_footer(theme, header, FittedPhoto::new(&photo)))
    }
}

pub use pelican_ui::components::AspectFit as Fit;

#[derive(Debug, Component, Clone)]
pub struct FittedPhoto(Fit, Image);
impl FittedPhoto {
    pub fn new(photo: &Photo) -> Self {Self(Fit::new(photo.size()), photo.image())}
}

/// Shared photo/video overlay used by public viewer components.
#[derive(Debug, Component, Clone)]
pub struct Badge(Stack, Rectangle, Option<Icon>, Option<Text>);
impl Badge {
    pub fn play(theme: &Theme) -> Self {
        Self(
            Stack::new(
                Offset::Center, Offset::Center, Size::Static(72.0), Size::Static(72.0),
                Padding::default(),
            ), Rectangle::new(Color::from_hex("000000", 67), 36.0, None),
            Some(Icon::new(theme, Icons::Play, Some(Color::white()), 40.0)), None,
        )
    }

    pub fn label(theme: &Theme, text: &str) -> Self {
        let around_text = |margin: f32| {
            Size::custom(move |children: Vec<(f32, f32)>| {
                let size = children.last().map(|c| c.1).unwrap_or_default() + 2.0 * margin;
                (size, size)
            })
        };
        Self(
            Stack::new(
                Offset::Center, Offset::Center, around_text(12.0), around_text(6.0),
                Padding::default(),
            ), Rectangle::new(Color::from_hex("000000", 67), 14.0, None), None,
            Some(Text::new(
                theme, text, TextSize::Md, TextStyle::Label(Color::white()), Align::Left, None,
            ).fit()),
        )
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
    page: FullScreen<ptsd::display::Bin<Fit, Option<Image>>, CaptureControls>,
    #[skip] camera: Option<prism::hardware::Camera>,
    #[skip] latest: Option<Image>,
    #[skip] encoding: Option<pelican_ui::Task<Option<Photo>>>,
    #[skip] start: prism::events::Timer,
    #[skip] token: uuid::Uuid,
    #[skip] theme: Theme,
    #[skip] on_capture: Box<dyn PhotoCaptured>,
    #[skip] failed: bool,
    #[skip] focused: bool,
    #[skip] finished: bool,
}
#[derive(Debug, Component, Clone)]
struct CaptureControls {
    layout: Column,
    notice: Option<Text>,
    capture: pelican_ui::components::button::GhostIconButton,
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
            page: FullScreen::new(theme, Header::stack(theme, "Camera", None),
                ptsd::display::Bin(Fit::new((1, 1)), None), CaptureControls {
                    layout: Column::new(12.0, Offset::Center, Size::Fill, Padding::new(16.0), None),
                    notice: None, capture: Self::button(theme, token),
                }),
            camera: None, latest: None,
            encoding: None, start: prism::events::Timer::new(std::time::Duration::ZERO),
            token, theme: theme.clone(), on_capture, failed: false, focused: true, finished: false,
        };
        page.page.footer.content.capture.1.disable(true);
        page
    }

    fn button(theme: &Theme, token: uuid::Uuid) -> pelican_ui::components::button::GhostIconButton {
        pelican_ui::components::button::GhostIconButton::large(theme, Icons::Capture,
            Stack::center(), move |ctx, _| ctx.emit(CapturePhoto(token)))
    }

    fn open(&mut self, ctx: &mut Context) {
        self.failed = false;
        self.latest = None;
        self.page.content.1 = None;
        self.page.footer.content.notice = None;
        self.page.footer.content.capture = Self::button(&self.theme, self.token);
        self.page.footer.content.capture.1.disable(true);
        match ctx.hardware.start_camera() {
            Ok(camera) => self.camera = Some(camera),
            Err(error) => self.error(&error),
        }
    }

    fn error(&mut self, message: &str) {
        self.camera = None;
        self.latest = None;
        self.failed = true;
        self.page.footer.content.notice = Some(Text::new(&self.theme, message, TextSize::Sm, TextStyle::Secondary, Align::Center, None));
        self.page.footer.content.capture = Self::button(&self.theme, self.token);
        self.page.footer.content.capture.1.disable(!self.focused);
    }
}

#[prism::events::on_change(self.start.next())]
impl prism::events::OnChange<()> for CameraPage {
    fn on_change(&mut self, ctx: &mut Context, _: ()) {
        if self.focused && !self.finished && self.encoding.is_none() {self.open(ctx);}
    }
}

#[prism::events::on_change(async {
    use futures::FutureExt;
    let Some(camera) = &mut self.camera else {return std::future::pending().await};
    let mut frame = camera.next().await?;
    // Use recent frames without letting a stalled preview monopolize the UI.
    for _ in 0..8 {
        match camera.next().now_or_never() {Some(Some(next)) => frame = next, _ => break}
    }
    Some(frame)
})]
impl prism::events::OnChange<Option<prism::hardware::VideoFrame>> for CameraPage {
    fn on_change(&mut self, _: &mut Context, frame: Option<prism::hardware::VideoFrame>) {
        if !self.focused || self.finished || self.encoding.is_some() {return;}
        match frame {
            Some(frame) if !frame.corrupted && frame.image.dimensions.0 > 0 && frame.image.dimensions.1 > 0 => {
                self.page.content.0 = Fit::new(frame.image.dimensions);
                self.page.content.1 = Some(frame.image.clone());
                self.latest = Some(frame.image);
                self.page.footer.content.capture.1.disable(false);
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
            self.page.footer.content.capture.1.disable(true);
            self.encoding = Some(pelican_ui::Task::spawn(move || Photo::from_image(&image)));
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
                self.page.footer.content.capture.1.disable(true);
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
