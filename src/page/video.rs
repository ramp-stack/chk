use pelican_ui::{Context, colors};
use pelican_ui::components::{Circle, Icon, Rectangle, Video, VideoClip};
use pelican_ui::components::text::{Text, TextSize, TextStyle};
use pelican_ui::interface::general::Header;
use pelican_ui::theme::{Icons, Theme};
use prism::drawable::{Component, Drawable, Instruction, LayoutTree, Rect, Shape, text::Align};
use prism::events::{ChildEvent, DynEvent, Event, MouseButton, MouseEvent, MouseState};
use prism::hardware::{Media, Frame, Player};
use prism::air::{Instance, Instances};
use prism::events::{on_change, on_event, ListenCallback, ListenFuture, OnChange, OnEvent};
use ptsd::{emitters, events};
use ptsd::display::Bin;
use ptsd::layout::{Column, Offset, Padding, Row, Size, Slider as SliderLayout, Stack};
use ptsd::navigation::AppPage;
use uuid::Uuid;

use std::any::Any;
use std::ops::{Deref, DerefMut};
use std::time::{Duration, Instant};

use crate::FlowBuilder;
use super::media::{Badge, Fit, FittedPhoto};

#[derive(Debug, Component, Clone)]
pub struct VideoPage {
    layout: Column,
    header: Header,
    screen: VideoScreen,
    controls: Controls,
    #[skip]
    video: Video,
    #[skip]
    clips: Option<Instances<VideoClip>>,
    #[skip]
    theme: Theme,
    #[skip]
    token: Uuid,
    #[skip]
    state: State,
    #[skip]
    shown: Option<Shown>,
}
impl AppPage for VideoPage {}

impl VideoPage {
    pub fn new(ctx: &mut Context, theme: &Theme, video: Video, header: Option<(Icons, Box<dyn FlowBuilder>)>) -> Self {
        let token = Uuid::new_v4();
        let icon = header.map(|(icon, mut flow)| (icon, (flow)(ctx, theme).build(ctx, theme)));
        let header = Header::stack(theme, "Video", icon);
        let mut page = Self {
            layout: Column::new(12.0, Offset::Center, Size::Fill, Padding::default(), None), header,
            screen: VideoScreen(ScreenBody(
                Stack::fill(), Box::new(FittedPhoto::new(&video.poster)),
                Some(Badge::label(theme, "Loading\u{2026}")),
            )), controls: Controls::new(theme, token), video, theme: theme.clone(), token,
            state: State::Waiting, shown: None, clips: Some(ctx.air.instances::<VideoClip>()),
        };
        page.load(ctx);
        page.update(None);
        page
    }

    fn load(&mut self, ctx: &mut Context) {
        if let State::Waiting = self.state
            && let Some(bytes) = VideoClip::find(ctx, self.video.clip)
        {
            let player = Media::read(std::io::Cursor::new(bytes)).and_then(|media| {
                let size = media.video.dimensions().ok_or("Media has no video track")?;
                ctx.hardware.player(media.player()).map(|player| (player, size))
            });
            self.clips = None;
            self.state = match player {
                Ok((player, size)) => {
                    player.start();
                    self.screen.0.1 = Box::new(Bin(Fit::new(size), player.clone()));
                    State::Ready { player, scrubbing: None }
                }
                Err(_) => State::Unplayable,
            };
        }
    }

    fn update(&mut self, playback: Option<Frame>) {
        let duration = Duration::from_millis(self.video.duration_ms);
        let (badge, playing, position, duration) = match &self.state {
            State::Waiting => (BadgeKind::Loading, false, Duration::ZERO, duration),
            State::Unplayable => (BadgeKind::Unplayable, false, Duration::ZERO, duration),
            State::Ready { player, scrubbing } => {
                let (position, duration, playing) = playback
                    .map(|event| (event.position, event.duration.unwrap(), event.playing))
                    .unwrap_or_else(|| {
                        (player.position(), player.duration().unwrap(), player.is_playing())
                    });
                let error = player.error();
                let playing =
                    error.is_none() && (playing || scrubbing.is_some_and(|s| s.was_playing));
                let badge = if error.is_some() {
                    BadgeKind::Unplayable
                } else if playing {
                    BadgeKind::None
                } else { BadgeKind::Play };
                if scrubbing.is_none() {
                    self.controls.scrubber.value = if duration.is_zero() {
                        0.0
                    } else {
                        (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
                    };
                }
                (badge, playing, position, duration)
            }
        };

        let shown = Shown {
            badge, playing, second: position.as_secs(),
            duration: duration.as_secs_f64().round() as u64,
        };
        if self.shown.as_ref().is_none_or(|old| old.badge != shown.badge) {
            self.screen.0.2 = match badge {
                BadgeKind::Loading => Some(Badge::label(&self.theme, "Loading\u{2026}")),
                BadgeKind::Unplayable => {
                    Some(Badge::label(&self.theme, "This video can't be played here"))
                }
                BadgeKind::Play => Some(Badge::play(&self.theme)), BadgeKind::None => None,
            };
        }
        if self.shown.as_ref().is_none_or(|old| old.playing != shown.playing) {
            self.controls.toggle.1.show(&self.theme, playing);
        }
        if self.shown.as_ref().is_none_or(|old| old.second != shown.second) {
            self.controls.position.1 = Controls::time(&self.theme, shown.second);
        }
        if self.shown.as_ref().is_none_or(|old| old.duration != shown.duration) {
            self.controls.duration.1 = Controls::time(&self.theme, shown.duration);
        }
        self.shown = Some(shown);
    }

    fn act(&mut self, action: Action) {
        let State::Ready { player, scrubbing } = &mut self.state else { return };
        match action {
            Action::Toggle if player.is_playing() => player.stop(),
            Action::Toggle => player.start(),
            Action::Scrub(value, phase) => {
                let duration = player.duration().unwrap();
                let to = duration.mul_f64(value.clamp(0.0, 1.0) as f64);
                match phase {
                    Phase::Start => {
                        *scrubbing = Some(Scrubbing {
                            was_playing: player.is_playing(), last_seek: Instant::now(),
                        });
                        player.stop();
                        player.seek(to);
                    }
                    Phase::Move => {
                        if let Some(scrub) = scrubbing
                            && scrub.last_seek.elapsed() >= Duration::from_millis(100)
                        {
                            scrub.last_seek = Instant::now();
                            player.seek(to);
                        }
                    }
                    Phase::End => {
                        player.seek(to);
                        if scrubbing.take().is_some_and(|s| s.was_playing) { player.start(); }
                    }
                }
            }
        }
    }
}

#[on_event]
impl OnEvent<VideoEvent> for VideoPage {
    fn on(&mut self, _ctx: &mut Context, event: VideoEvent) -> Vec<Box<dyn DynEvent>> {
        if event.0 != self.token {return vec![Box::new(event)]}
        self.act(event.1);
        vec![]
    }
}

#[on_change(async {
    match &mut self.clips {
        Some(clips) => clips.next().await,
        None => std::future::pending().await,
    }
})]
impl OnChange<Instance<VideoClip>> for VideoPage {
    fn on_change(&mut self, ctx: &mut Context, clip: Instance<VideoClip>) {
        if clip.pending().id == self.video.clip {
            self.load(ctx);
            self.update(None);
        }
    }
}

#[on_change(async {
    match &mut self.state {
        State::Ready {player, ..} => player.next().await,
        _ => std::future::pending().await,
    }
})]
impl OnChange<Frame> for VideoPage {
    fn on_change(&mut self, _ctx: &mut Context, playback: Frame) { self.update(Some(playback)); }
}

#[derive(Debug, Clone)]
enum State {
    Waiting,
    Ready { player: Player, scrubbing: Option<Scrubbing> }, Unplayable,
}
#[derive(Debug, Clone, Copy)]
struct Scrubbing {
    was_playing: bool,
    last_seek: Instant,
}

#[derive(Debug, Clone, PartialEq)]
struct Shown {
    badge: BadgeKind,
    playing: bool,
    second: u64,
    duration: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BadgeKind {
    Loading, Unplayable, Play, None,
}

#[derive(Debug, Clone, Copy)]
enum Action {
    Toggle, Scrub(f32, Phase),
}

#[derive(Debug, Clone, Copy)]
enum Phase {
    Start, Move, End,
}

#[derive(Debug, Clone, Copy)]
struct VideoEvent(Uuid, Action);
impl Event for VideoEvent {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children { *child.event = Some(*self); }
    }
}

#[derive(Debug, Clone)]
struct VideoScreen(ScreenBody);
impl Drawable for VideoScreen {
    fn request_width(&mut self) -> (f32, f32) {self.0.request_width()}
    fn request_height(&mut self, width: f32) -> (f32, f32) {self.0.request_height(width)}
    fn layout(&mut self, size: (f32, f32)) -> LayoutTree {Drawable::layout(&mut self.0, size)}
    fn draw(&self, layout: &LayoutTree, offset: (f32, f32), bound: Rect) -> Vec<Instruction> {
        self.0.draw(layout, offset, bound)
    }
    fn event(&mut self, ctx: &mut Context, layout: &LayoutTree, event: Box<dyn DynEvent>) {
        // Badges are visual; pointer input belongs to the fitted player.
        self.0.1.event(ctx, &layout.1[0].1, event);
    }
    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            let callback = Component::listen(&mut self.0).await;
            Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                callback(&mut target.downcast_mut::<Self>().unwrap().0, ctx);
            }) as ListenCallback
        })
    }
}

#[derive(Debug, Component, Clone)]
struct ScreenBody(Stack, Box<dyn Drawable>, Option<Badge>);

#[derive(Debug, Component, Clone)]
struct Controls {
    layout: Row,
    toggle: emitters::Button<Bin<Stack, Toggle>>,
    position: Bin<Stack, Text>,
    scrubber: Bin<Stack, Scrubber>,
    duration: Bin<Stack, Text>,
}
impl Controls {
    fn new(theme: &Theme, token: Uuid) -> Self {
        Self {
            layout: Row::new(0.0, Offset::Center, Size::Fit, Padding(0.0, 0.0, 24.0, 0.0)),
            toggle: emitters::Button::new(Bin(
                Stack::new(
                    Offset::Center, Offset::Center, Size::Fit, Size::Fit,
                    Padding(16.0, 4.0, 6.0, 16.0),
                ), Toggle::new(theme, token),
            )),
            position: Bin(Self::content_layout(Padding(6.0, 4.0, 6.0, 16.0)), Self::time(theme, 0)),
            scrubber: Bin(
                Self::content_layout(Padding(6.0, 4.0, 6.0, 16.0)), Scrubber::new(theme, token),
            ),
            duration: Bin(Self::content_layout(Padding(6.0, 4.0, 0.0, 16.0)), Self::time(theme, 0)),
        }
    }

    fn content_layout(padding: Padding) -> Stack {
        Stack::new(Offset::Center, Offset::Center, Size::Fit, Size::Fit, padding)
    }

    fn time(theme: &Theme, seconds: u64) -> Text {
        let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
        let text = match h {
            0 => format!("{m}:{s:02}"),
            _ => format!("{h}:{m:02}:{s:02}"),
        };
        Text::new(theme, &text, TextSize::Sm, TextStyle::Secondary, Align::Left, None).fit()
    }
}

#[derive(Debug, Component, Clone)]
struct Toggle(Stack, Option<Icon>, Option<PauseBars>, #[skip] Uuid);
impl Toggle {
    fn new(theme: &Theme, token: Uuid) -> Self {
        let layout = Stack::new(
            Offset::Center, Offset::Center, Size::Static(40.0), Size::Static(40.0),
            Padding::default(),
        );
        let mut toggle = Self(layout, None, None, token);
        toggle.show(theme, false);
        toggle
    }

    fn show(&mut self, theme: &Theme, playing: bool) {
        let color = theme.colors().get(colors::Text::Heading);
        self.1 = (!playing).then(|| Icon::new(theme, Icons::Play, Some(color), 28.0));
        self.2 = playing.then(|| PauseBars::new(theme));
    }
}

#[on_event]
impl OnEvent<events::Button> for Toggle {
    fn on(&mut self, ctx: &mut Context, event: events::Button) -> Vec<Box<dyn DynEvent>> {
        if let events::Button::Pressed(false) = event {
            ctx.emit(VideoEvent(self.3, Action::Toggle));
        }
        vec![Box::new(event)]
    }
}

#[derive(Debug, Component, Clone)]
struct PauseBars(Row, Bin<Stack, Rectangle>, Bin<Stack, Rectangle>);
impl PauseBars {
    fn new(theme: &Theme) -> Self {
        let color = theme.colors().get(colors::Text::Heading);
        let bar = || {
            Bin(
                Stack::new(
                    Offset::Center, Offset::Center, Size::Static(5.0), Size::Static(18.0),
                    Padding::default(),
                ), Rectangle::new(color, 1.5, None),
            )
        };
        Self(Row::center(5.0), bar(), bar())
    }
}

#[derive(Debug, Component, Clone)]
struct Scrubber {
    layout: SliderLayout,
    background: Bin<Stack, Rectangle>,
    foreground: Bin<Stack, Rectangle>,
    handle: Bin<Stack, Shape>,
    #[skip] dragging: bool,
    #[skip] token: Uuid,
}
impl Scrubber {
    fn new(theme: &Theme, token: Uuid) -> Self {
        let colors = theme.colors();
        let bar = |width: Size| {
            Stack::new(Offset::Start, Offset::Center, width, Size::Static(4.0), Padding::default())
        };
        Self {
            layout: SliderLayout::new(0.0, Size::Static(32.0)),
            background: Bin(
                bar(Size::Fill), Rectangle::new(colors.get(colors::Outline::Primary), 2.0, None),
            ),
            foreground: Bin(bar(Size::Fill), Rectangle::new(colors.get(colors::Brand), 2.0, None)),
            handle: Circle::new(14.0, colors.get(colors::Brand), false),
            dragging: false, token,
        }
    }
}

impl Deref for Scrubber {
    type Target = SliderLayout;
    fn deref(&self) -> &Self::Target {&self.layout}
}

impl DerefMut for Scrubber {
    fn deref_mut(&mut self) -> &mut Self::Target {&mut self.layout}
}

#[on_event]
impl OnEvent<MouseEvent> for Scrubber {
    fn on(&mut self, ctx: &mut Context, event: MouseEvent) -> Vec<Box<dyn DynEvent>> {
        let phase = match (event.state, event.position) {
            (MouseState::Pressed(MouseButton::Left), Some(_)) => {
                self.dragging = true;
                Some(Phase::Start)
            }
            (MouseState::Moved | MouseState::Scroll(..), Some(_)) if self.dragging => {
                Some(Phase::Move)
            }
            (MouseState::Released(MouseButton::Left), _) if self.dragging => {
                self.dragging = false;
                Some(Phase::End)
            }
            _ => None,
        };
        if let Some(phase) = phase {
            if let Some((x, _)) = event.position {
                self.value = self.layout.position(x).clamp(0.0, 1.0);
            }
            ctx.emit(VideoEvent(self.token, Action::Scrub(self.value, phase)));
        }
        prism::events![event]
    }
}
