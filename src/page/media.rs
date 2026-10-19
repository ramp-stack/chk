use std::sync::Arc;

use pelican_ui::{Context, Task, colors, image};
use pelican_ui::components::{Icon, Photo, Rectangle};
use pelican_ui::components::text::{Text, TextSize, TextStyle};
use pelican_ui::interface::general::Header;
use pelican_ui::theme::{Color, Icons, Theme};
use prism::drawable::{Area, Component, Layout, SizeRange, text::Align};
use prism::events::{DynEvent, OnEvent, TickEvent, on_event};
use prism::hardware::Image;
use ptsd::display::Bin;
use ptsd::layout::{Column, Offset, Padding, Size, Stack};
use ptsd::navigation::AppPage;

use crate::FlowBuilder;

#[derive(Debug, Component, Clone)]
pub struct PhotoPage(Column, Bin<Stack, Header>, FittedPhoto);
impl AppPage for PhotoPage {}

impl PhotoPage {
    pub fn new(ctx: &mut Context, theme: &Theme, photo: Photo, header: Option<(Icons, Box<dyn FlowBuilder>)>) -> Self {
        let icon = header.map(|(icon, mut flow)| (icon, (flow)(ctx, theme).build(ctx, theme)));
        let header = Bin(
            Stack(Offset::Center, Offset::Center, Size::Fill, Size::Fit, Padding(24.0, 0.0, 24.0, 0.0)),
            Header::stack(theme, "Photo", icon),
        );
        Self(
            Column::new(12.0, Offset::Center, Size::Fill, Padding::default(), None),
            header, FittedPhoto::new(theme, &photo),
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct Fit((u32, u32));
impl Fit {
    pub fn new(size: (u32, u32)) -> Self {Self(size)}
}

impl Layout for Fit {
    fn size_range(&self, _children: Vec<SizeRange>) -> SizeRange {
        SizeRange {min_width: 0.0, max_width: f32::MAX, min_height: 0.0, max_height: f32::MAX}
    }

    fn layout(&self, available: (f32, f32), children: Vec<SizeRange>) -> Vec<Area> {
        let intrinsic = (self.0.0.max(1) as f32, self.0.1.max(1) as f32);
        let scale = (available.0 / intrinsic.0).min(available.1 / intrinsic.1).max(0.0);
        let size = (intrinsic.0 * scale, intrinsic.1 * scale);
        let offset = ((available.0 - size.0) / 2.0, (available.1 - size.1) / 2.0);
        children.into_iter().map(|_| Area {offset, size}).collect()
    }
}

#[doc(hidden)]
#[derive(Debug, Component, Clone)]
pub struct FittedPhoto(Fit, Option<Image>, Option<Rectangle>, #[skip] Option<Task<Option<Image>>>);
impl FittedPhoto {
    pub fn new(theme: &Theme, photo: &Photo) -> Self {
        let size = photo.size();
        let photo = photo.clone();
        let decoded = Task::spawn(move || {
            let image = image::load_from_memory(photo.jpeg()).ok()?;
            let image = match image.width().max(image.height()) > 2048 {
                true => image.resize(2048, 2048, image::imageops::FilterType::Triangle),
                false => image,
            };
            Some(Arc::new(image.into_rgba8()))
        });
        let placeholder = Rectangle::new(theme.colors().get(colors::Background::Secondary), 0.0, None);
        Self(Fit::new(size), None, Some(placeholder), Some(decoded))
    }
}

#[on_event]
impl OnEvent<TickEvent> for FittedPhoto {
    fn on(&mut self, _ctx: &mut Context, event: TickEvent) -> Vec<Box<dyn DynEvent>> {
        if let Some(decoded) = self.3.as_ref().and_then(Task::take) {
            self.3 = None;
            if let Some(image) = decoded {
                self.0 = Fit::new(image.dimensions());
                self.1 = Some(image);
                self.2 = None;
            }
        }
        prism::events![event]
    }
}

#[doc(hidden)]
#[derive(Debug, Component, Clone)]
pub struct Badge(Stack, Rectangle, Option<Icon>, Option<Text>);
impl Badge {
    pub fn play(theme: &Theme) -> Self {
        Self(
            Stack(Offset::Center, Offset::Center, Size::Static(72.0), Size::Static(72.0), Padding::default()),
            Rectangle::new(Color::from_hex("000000", 140), 36.0, None),
            Some(Icon::new(theme, Icons::Play, Some(Color::WHITE), 40.0)), None,
        )
    }

    pub fn label(theme: &Theme, text: &str) -> Self {
        let around_text = |margin: f32| Size::custom(move |children: Vec<(f32, f32)>| {
            let size = children.last().map(|c| c.0).unwrap_or_default() + 2.0 * margin;
            (size, size)
        });
        Self(
            Stack(Offset::Center, Offset::Center, around_text(12.0), around_text(6.0), Padding::default()),
            Rectangle::new(Color::from_hex("000000", 140), 14.0, None), None,
            Some(Text::new(theme, text, TextSize::Md, TextStyle::Label(Color::WHITE), Align::Left, None)),
        )
    }
}
