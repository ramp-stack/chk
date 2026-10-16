use ramp::prism;

use pelican_ui::Context;
use pelican_ui::event::{OnEvent, Event, TickEvent};
use pelican_ui::drawable::{Component, Drawable, SizedTree, RequestTree, Rect};
use pelican_ui::layout::{Column, Stack, Offset, Size, Padding, SizeRequest};
use pelican_ui::canvas::{self, Align, Area, Image, Instruction, Item, Shape, ShapeType};
use pelican_ui::display::Bin;
use pelican_ui::interface::general::Header;
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Color, Icons};
use pelican_ui::{Task, colors};
use pelican_ui::components::{Rectangle, Icon, Photo};
use pelican_ui::components::text::{Text, TextSize, TextStyle};
use pelican_ui::image::{self, RgbaImage, imageops::FilterType};

use crate::FlowBuilder;

use std::sync::Arc;

/// The most pixels a photo is shown with on its longest side. Photos are
/// sent at most 1600, and every platform can show a picture this big.
const MAX_SIDE: u32 = 2048;

/// A photo from a chat, as big as the screen allows.
#[derive(Debug, Component, Clone)]
pub struct PhotoPage(Column, Bin<Stack, Header>, FittedPhoto);
impl OnEvent for PhotoPage {}
impl AppPage for PhotoPage {}

impl PhotoPage {
    pub fn new(ctx: &mut Context, theme: &Theme, photo: Photo, header: Option<(Icons, Box<dyn FlowBuilder>)>) -> Self {
        PhotoPage(layout(), page_header(ctx, theme, "Photo", header), FittedPhoto::new(theme, &photo))
    }
}

/// The header on top, and the photo or video taking up the room below it,
/// edge to edge.
pub(super) fn layout() -> Column {
    Column::new(12.0, Offset::Center, Size::Fill, Padding::default(), None)
}

/// A back button, the title, and an optional icon on the right that opens a
/// flow (spaced from the sides like any other page's header).
pub(super) fn page_header(ctx: &mut Context, theme: &Theme, title: &str, header: Option<(Icons, Box<dyn FlowBuilder>)>) -> Bin<Stack, Header> {
    let icon = header.map(|(icon, mut flow)| (icon, (flow)(ctx, theme).build(ctx, theme)));
    let layout = Stack(Offset::Center, Offset::Center, Size::Fill, Size::Fit, Padding(24.0, 0.0, 24.0, 0.0));
    Bin(layout, Header::stack(theme, title, icon))
}

/// A photo as big as its space allows, in its own shape, in the middle. It's
/// decoded at full size away from the interface; until then it's a plain box.
#[derive(Debug, Component, Clone)]
pub(super) struct FittedPhoto(Stack, Fitted, #[skip] Option<Task<Option<Arc<RgbaImage>>>>);

impl FittedPhoto {
    pub(super) fn new(theme: &Theme, photo: &Photo) -> Self {
        FittedPhoto(Stack::fill(), Fitted::new(theme, photo.size()), Some(decode(photo)))
    }

    /// Shows `image` in its place (from now on, instead of the photo).
    pub(super) fn show(&mut self, image: Arc<RgbaImage>) {
        self.2 = None;
        self.1.show(image);
    }
}

impl OnEvent for FittedPhoto {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if event.downcast_ref::<TickEvent>().is_some() && let Some(decoded) = self.2.as_ref().and_then(|task| task.take()) {
            self.2 = None;
            if let Some(image) = decoded { self.1.show(image); }
        }
        vec![event]
    }
}

/// `photo`'s pixels, at most [`MAX_SIDE`] on its longest side.
fn decode(photo: &Photo) -> Task<Option<Arc<RgbaImage>>> {
    let photo = photo.clone();
    Task::spawn(move || {
        let image = image::load_from_memory(photo.jpeg()).ok()?;
        let image = match image.width().max(image.height()) > MAX_SIDE {
            true => image.resize(MAX_SIDE, MAX_SIDE, FilterType::Triangle),
            false => image,
        };
        Some(Arc::new(image.into_rgba8()))
    })
}

/// Takes all the room it's given, and draws its image as big as fits in it,
/// keeping its shape, in the middle (a box of that shape until there's an
/// image).
#[derive(Debug, Clone)]
struct Fitted {
    image: Option<Arc<RgbaImage>>,
    /// Its width and height, for its shape.
    size: (f32, f32),
    placeholder: canvas::Color,
}

impl Fitted {
    fn new(theme: &Theme, (width, height): (u32, u32)) -> Self {
        let placeholder = theme.colors().get(colors::Background::Secondary).into();
        Fitted { image: None, size: (width.max(1) as f32, height.max(1) as f32), placeholder }
    }

    fn show(&mut self, image: Arc<RgbaImage>) {
        let (width, height) = image.dimensions();
        self.size = (width.max(1) as f32, height.max(1) as f32);
        self.image = Some(image);
    }
}

impl Drawable for Fitted {
    fn request_size(&self) -> RequestTree { RequestTree(SizeRequest::fill(), vec![]) }

    fn draw(&self, sized: &SizedTree, offset: (f32, f32), bound: Rect) -> Vec<Instruction> {
        let (width, height) = sized.0;
        let scale = (width / self.size.0).min(height / self.size.1);
        if !(scale.is_finite() && scale > 0.0) { return vec![]; }

        let size = (self.size.0 * scale, self.size.1 * scale);
        let at = (offset.0 + (width - size.0) / 2.0, offset.1 + (height - size.1) / 2.0);
        let shape = ShapeType::Rectangle(0.0, size, 0.0);
        let item = match &self.image {
            Some(image) => Item::Image(Image { shape, image: image.clone(), color: None }),
            None => Item::Shape(Shape { shape, color: self.placeholder }),
        };
        vec![Instruction(Area { offset: at, bounds: Some(bound) }, item)]
    }
}

/// A dark, see-through badge over the video: the play button, or a label.
#[derive(Debug, Component, Clone)]
pub(super) struct Badge(Stack, Rectangle, Option<Image>, Option<Text>);
impl OnEvent for Badge {}

impl Badge {
    pub(super) fn play(theme: &Theme) -> Self {
        Badge(
            Stack(Offset::Center, Offset::Center, Size::Static(72.0), Size::Static(72.0), Padding::default()),
            Rectangle::new(Color::from_hex("000000", 140), 36.0, None),
            Some(Icon::new(theme, Icons::Play, Some(Color::WHITE), 40.0)),
            None,
        )
    }

    pub(super) fn label(theme: &Theme, text: &str) -> Self {
        // Its text and a margin around it (the background alone would take
        // all the room there is).
        let around_text = |margin: f32| Size::custom(move |children: Vec<(f32, f32)>| {
            let text = children.last().map(|c| c.0).unwrap_or_default() + 2.0 * margin;
            (text, text)
        });
        Badge(
            Stack(Offset::Center, Offset::Center, around_text(12.0), around_text(6.0), Padding::default()),
            Rectangle::new(Color::from_hex("000000", 140), 14.0, None),
            None,
            Some(Text::new(theme, text, TextSize::Md, TextStyle::Label(Color::WHITE), Align::Left, None)),
        )
    }
}
