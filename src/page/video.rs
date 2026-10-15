use ramp::prism;

use pelican_ui::Context;
use pelican_ui::event::{self, OnEvent, Event, TickEvent, MouseEvent, MouseState, MouseButton};
use pelican_ui::drawable::{Component, SizedTree};
use pelican_ui::layout::{Area, Column, Row, Stack, Offset, Size, Padding};
use pelican_ui::canvas::{Align, Image, Shape};
use pelican_ui::display::Bin;
use pelican_ui::emitters;
use pelican_ui::interface::general::Header;
use pelican_ui::navigation::AppPage;
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::colors;
use pelican_ui::components::{Rectangle, Circle, Icon, Video, VideoClip};
use pelican_ui::components::text::{Text, TextSize, TextStyle};

use crate::FlowBuilder;
use crate::player::Player;
use super::media::{layout, page_header, FittedPhoto, Badge};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// While the scrubber is dragged, it goes to where it's dragged at most this
/// often (each time starts decoding again from a keyframe).
const SCRUB_EVERY: Duration = Duration::from_millis(100);

/// A video from a chat, played right in the page: its picture as big as
/// fits, with play/pause and a bar to move through it below. It starts
/// playing as soon as its clip is here; tapping the picture pauses and plays
/// it.
#[derive(Debug, Component, Clone)]
pub struct VideoPage {
    layout: Column,
    header: Bin<Stack, Header>,
    screen: emitters::Button<VideoScreen>,
    controls: Controls,
    #[skip] video: Video,
    #[skip] theme: Theme,
    #[skip] token: u64,
    #[skip] state: State,
    /// What the badge, button and time last showed, so they're only remade
    /// when that changes.
    #[skip] shown: Shown,
}

/// Where the page is with its video.
#[derive(Debug, Clone)]
enum State {
    /// Its clip hasn't arrived yet.
    Waiting,
    Ready { player: Rc<RefCell<Player>>, scrubbing: Option<Scrubbing> },
    /// It can't be played here (it isn't H.264, or this is the web).
    Unplayable,
}

#[derive(Debug, Clone, Copy)]
struct Scrubbing { was_playing: bool, last_seek: Instant }

#[derive(Debug, Clone, Default, PartialEq)]
struct Shown { badge: Option<BadgeKind>, playing: Option<bool>, second: Option<u64>, duration: Option<u64> }

#[derive(Debug, Clone, Copy, PartialEq)]
enum BadgeKind { Loading, Unplayable, Play, None }

impl AppPage for VideoPage {}

impl VideoPage {
    pub fn new(ctx: &mut Context, theme: &Theme, video: Video, header: Option<(Icons, Box<dyn FlowBuilder>)>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let token = NEXT.fetch_add(1, Ordering::Relaxed);
        VideoPage {
            layout: layout(),
            header: page_header(ctx, theme, "Video", header),
            screen: emitters::Button::new(VideoScreen::new(theme, &video, token)),
            controls: Controls::new(theme, token),
            video,
            theme: theme.clone(),
            token,
            state: State::Waiting,
            shown: Shown::default(),
        }
    }

    /// Moves playing on to now, and shows where it's at.
    fn update(&mut self, ctx: &mut Context) {
        if let State::Waiting = self.state && let Some(mp4) = VideoClip::find(ctx, self.video.clip) {
            self.state = match Player::new(mp4) {
                Ok(mut player) => {
                    player.play();
                    State::Ready { player: Rc::new(RefCell::new(player)), scrubbing: None }
                },
                Err(_) => State::Unplayable,
            };
        }

        let (badge, playing, position, duration) = match &self.state {
            State::Waiting => (BadgeKind::Loading, false, 0.0, self.video.duration_ms as f64 / 1000.0),
            State::Unplayable => (BadgeKind::Unplayable, false, 0.0, self.video.duration_ms as f64 / 1000.0),
            State::Ready { player, scrubbing } => {
                let mut player = player.borrow_mut();
                if player.tick() && let Some(frame) = player.frame() {
                    self.screen.1.picture.show(frame);
                }
                let playing = player.is_playing() || scrubbing.is_some_and(|s| s.was_playing);
                let badge = if playing { BadgeKind::None } else { BadgeKind::Play };
                if scrubbing.is_none() && player.duration() > 0.0 {
                    self.controls.scrubber.value = (player.position() / player.duration()) as f32;
                }
                (badge, playing, player.position(), player.duration())
            },
        };

        let shown = Shown { badge: Some(badge), playing: Some(playing), second: Some(position as u64), duration: Some(duration.round() as u64) };
        if shown.badge != self.shown.badge {
            self.screen.1.badge = match badge {
                BadgeKind::Loading => Some(Badge::label(&self.theme, "Loading\u{2026}")),
                BadgeKind::Unplayable => Some(Badge::label(&self.theme, "This video can't be played here")),
                BadgeKind::Play => Some(Badge::play(&self.theme)),
                BadgeKind::None => None,
            };
        }
        if shown.playing != self.shown.playing { self.controls.toggle.1.show(&self.theme, playing); }
        if shown.second != self.shown.second { self.controls.position = time_text(&self.theme, position); }
        if shown.duration != self.shown.duration { self.controls.duration = time_text(&self.theme, duration.round()); }
        self.shown = shown;
    }

    fn act(&mut self, action: Action) {
        let State::Ready { player, scrubbing } = &mut self.state else { return };
        let mut player = player.borrow_mut();
        match action {
            Action::Toggle if player.is_playing() => player.pause(),
            Action::Toggle => player.play(),
            Action::Scrub(value, phase) => {
                let to = value as f64 * player.duration();
                match phase {
                    Phase::Start => {
                        *scrubbing = Some(Scrubbing { was_playing: player.is_playing(), last_seek: Instant::now() });
                        player.pause();
                        player.seek(to);
                    },
                    Phase::Move => if let Some(scrub) = scrubbing && scrub.last_seek.elapsed() >= SCRUB_EVERY {
                        scrub.last_seek = Instant::now();
                        player.seek(to);
                    },
                    Phase::End => {
                        player.seek(to);
                        if scrubbing.take().is_some_and(|s| s.was_playing) { player.play(); }
                    },
                }
            },
        }
    }
}

impl OnEvent for VideoPage {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if event.downcast_ref::<TickEvent>().is_some() {
            self.update(ctx);
        } else if let Some(VideoEvent(token, action)) = event.downcast_ref::<VideoEvent>() && *token == self.token {
            self.act(*action);
            return vec![];
        }
        vec![event]
    }
}

/// Something done to the page's video (the one with this number).
#[derive(Debug, Clone, Copy)]
struct VideoEvent(u64, Action);

#[derive(Debug, Clone, Copy)]
enum Action {
    /// Play if it's paused, pause if it's playing.
    Toggle,
    /// The scrubber's at this far along (0 to 1).
    Scrub(f32, Phase),
}

#[derive(Debug, Clone, Copy)]
enum Phase { Start, Move, End }

impl Event for VideoEvent {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}

/// The video's picture, as big as fits (its still until it plays), with a
/// badge over it when it isn't playing.
#[derive(Debug, Component, Clone)]
struct VideoScreen {
    layout: Stack,
    picture: FittedPhoto,
    badge: Option<Badge>,
    #[skip] token: u64,
}

impl VideoScreen {
    fn new(theme: &Theme, video: &Video, token: u64) -> Self {
        VideoScreen { layout: Stack::fill(), picture: FittedPhoto::new(theme, &video.poster), badge: Some(Badge::label(theme, "Loading\u{2026}")), token }
    }
}

impl OnEvent for VideoScreen {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        // Released after a tap (scrolling or dragging away doesn't count).
        if let Some(event::Button::Pressed(false)) = event.downcast_ref::<event::Button>() {
            ctx.emit(VideoEvent(self.token, Action::Toggle));
        }
        vec![event]
    }
}

/// Play/pause, how far along it is, the bar to move through it, and how long
/// it is.
#[derive(Debug, Component, Clone)]
struct Controls {
    layout: Row,
    toggle: emitters::Button<Toggle>,
    position: Text,
    scrubber: Scrubber,
    duration: Text,
}
impl OnEvent for Controls {}

impl Controls {
    fn new(theme: &Theme, token: u64) -> Self {
        Controls {
            layout: Row::new(12.0, Offset::Center, Size::Fit, Padding(16.0, 4.0, 24.0, 16.0)),
            toggle: emitters::Button::new(Toggle::new(theme, token)),
            position: time_text(theme, 0.0),
            scrubber: Scrubber::new(theme, token),
            duration: time_text(theme, 0.0),
        }
    }
}

fn time_text(theme: &Theme, seconds: f64) -> Text {
    Text::new(theme, &clock_text(seconds), TextSize::Sm, TextStyle::Secondary, Align::Left, None)
}

/// "0:42", "12:05", "1:02:03".
fn clock_text(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    match h {
        0 => format!("{m}:{s:02}"),
        _ => format!("{h}:{m:02}:{s:02}"),
    }
}

/// The play/pause button: a play triangle while paused, two bars while
/// playing.
#[derive(Debug, Component, Clone)]
struct Toggle(Stack, Option<Image>, Option<PauseBars>, #[skip] u64);

impl Toggle {
    fn new(theme: &Theme, token: u64) -> Self {
        let mut toggle = Toggle(Stack(Offset::Center, Offset::Center, Size::Static(40.0), Size::Static(40.0), Padding::default()), None, None, token);
        toggle.show(theme, false);
        toggle
    }

    fn show(&mut self, theme: &Theme, playing: bool) {
        let color = theme.colors().get(colors::Text::Heading);
        self.1 = (!playing).then(|| Icon::new(theme, Icons::Play, Some(color), 28.0));
        self.2 = playing.then(|| PauseBars::new(theme));
    }
}

impl OnEvent for Toggle {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(event::Button::Pressed(false)) = event.downcast_ref::<event::Button>() {
            ctx.emit(VideoEvent(self.3, Action::Toggle));
        }
        vec![event]
    }
}

#[derive(Debug, Component, Clone)]
struct PauseBars(Row, Bin<Stack, Rectangle>, Bin<Stack, Rectangle>);
impl OnEvent for PauseBars {}

impl PauseBars {
    fn new(theme: &Theme) -> Self {
        let color = theme.colors().get(colors::Text::Heading);
        let bar = || Bin(Stack(Offset::Center, Offset::Center, Size::Static(5.0), Size::Static(18.0), Padding::default()), Rectangle::new(color, 1.5, None));
        PauseBars(Row::center(5.0), bar(), bar())
    }
}

/// How far along it is, as a bar. Tapping or dragging it goes there.
#[derive(Debug, Component, Clone)]
struct Scrubber {
    layout: Stack,
    track: Bin<Stack, Rectangle>,
    played: Bin<Stack, Rectangle>,
    handle: Bin<Stack, Shape>,
    /// How far along (0 to 1).
    #[skip] value: f32,
    #[skip] dragging: bool,
    #[skip] token: u64,
}

impl Scrubber {
    /// How tall it is to touch (the bar itself is thinner).
    const HEIGHT: f32 = 32.0;
    const BAR: f32 = 4.0;
    const HANDLE: f32 = 14.0;

    fn new(theme: &Theme, token: u64) -> Self {
        let colors = theme.colors();
        let bar = |width: Size| Stack(Offset::Start, Offset::Center, width, Size::Static(Self::BAR), Padding::default());
        Scrubber {
            layout: Stack(Offset::Start, Offset::Center, Size::Fill, Size::Static(Self::HEIGHT), Padding::default()),
            track: Bin(bar(Size::Fill), Rectangle::new(colors.get(colors::Outline::Primary), Self::BAR / 2.0, None)),
            played: Bin(bar(Size::Static(0.0)), Rectangle::new(colors.get(colors::Brand), Self::BAR / 2.0, None)),
            handle: Bin(Stack(Offset::Static(0.0), Offset::Center, Size::Fit, Size::Fit, Padding::default()), Circle::new(Self::HANDLE, colors.get(colors::Brand), false)),
            value: 0.0,
            dragging: false,
            token,
        }
    }
}

impl OnEvent for Scrubber {
    fn on_event(&mut self, ctx: &mut Context, sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        let width = sized.0.0.max(1.0);
        let at = |x: f32| (x / width).clamp(0.0, 1.0);
        if let Some(MouseEvent { state, position }) = event.downcast_ref::<MouseEvent>() {
            let phase = match (state, position) {
                (MouseState::Pressed(MouseButton::Left), Some(_)) => { self.dragging = true; Some(Phase::Start) },
                (MouseState::Moved | MouseState::Scroll(..), Some(_)) if self.dragging => Some(Phase::Move),
                (MouseState::Released(MouseButton::Left), _) if self.dragging => { self.dragging = false; Some(Phase::End) },
                _ => None,
            };
            if let Some(phase) = phase {
                if let Some((x, _)) = position { self.value = at(*x); }
                ctx.emit(VideoEvent(self.token, Action::Scrub(self.value, phase)));
            }
        } else if event.downcast_ref::<TickEvent>().is_some() {
            let x = width * self.value.clamp(0.0, 1.0);
            self.played.get_layout().2 = Size::Static(x);
            self.handle.get_layout().0 = Offset::Static((x - Self::HANDLE / 2.0).clamp(0.0, (width - Self::HANDLE).max(0.0)));
        }
        vec![event]
    }
}
