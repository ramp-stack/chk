use pelican_ui::Context;
use pelican_ui::components::{Icon, Photo, Rectangle};
use pelican_ui::components::text::{Text, TextSize, TextStyle};
use pelican_ui::interface::general::Header;
use pelican_ui::theme::{Color, Icons, Theme};
use prism::drawable::{Area, Component, Layout, text::Align};
use prism::hardware::Image;
use ptsd::layout::{Column, Offset, Padding, Size, Stack};
use ptsd::navigation::AppPage;

use crate::FlowBuilder;

#[derive(Debug, Component, Clone)]
pub struct PhotoPage(Column, Header, FittedPhoto);
impl AppPage for PhotoPage {}

impl PhotoPage {
    pub fn new(ctx: &mut Context, theme: &Theme, photo: Photo, header: Option<(Icons, Box<dyn FlowBuilder>)>) -> Self {
        let icon = header.map(|(icon, mut flow)| (icon, (flow)(ctx, theme).build(ctx, theme)));
        let header = Header::stack(theme, "Photo", icon);
        Self(
            Column::new(12.0, Offset::Center, Size::Fill, Padding::default(), None), header,
            FittedPhoto::new(&photo),
        )
    }
}

/// Public layout type used by the photo component’s Component implementation.
#[derive(Debug, Clone)]
pub struct Fit((u32, u32), usize);
impl Fit {
    pub fn new(size: (u32, u32)) -> Self {Self(size, 0)}
}

impl Layout for Fit {
    fn request_width(&mut self, children: Vec<(f32, f32)>) -> (f32, f32) {
        self.1 = children.len();
        (0.0, f32::MAX)
    }

    fn assign_width(&mut self, width: f32) -> Vec<f32> {vec![width; self.1]}
    fn request_height(&mut self, _width: f32, _children: Vec<(f32, f32)>) -> (f32, f32) {
        (0.0, f32::MAX)
    }

    fn layout(&mut self, available: (f32, f32)) -> Vec<Area> {
        let intrinsic = (self.0.0.max(1) as f32, self.0.1.max(1) as f32);
        let scale = (available.0 / intrinsic.0).min(available.1 / intrinsic.1).max(0.0);
        let size = (intrinsic.0 * scale, intrinsic.1 * scale);
        let offset = ((available.0 - size.0) / 2.0, (available.1 - size.1) / 2.0);
        vec![Area {offset, size}; self.1]
    }
}

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
