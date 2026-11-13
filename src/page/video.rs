use crate::design_system::Context;
use crate::design_system::{Video, VideoClip};
use crate::design_system::{Icons, Theme};
use prism::drawable::Component;
use prism::events::{ChildEvent, DynEvent, Event};
use prism::hardware::{Media, Frame, Player};
use prism::air::{Instance, Instances};
use prism::events::{on_change, on_event, OnChange, OnEvent};
use ptsd::layout::Stack;
use ptsd::navigation::AppPage;
use uuid::Uuid;

use std::time::{Duration, Instant};


#[derive(Debug, Component, Clone)]
pub struct VideoPage {
    layout: Stack,
    page: crate::design_system::VideoView,
    #[skip]
    video: Option<Video>,
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
    pub fn new(ctx: &mut Context, theme: &Theme, video: Video, header: Option<(Icons, crate::Action)>) -> Self {
        let token = Uuid::new_v4();
        let mut page = Self {
            layout: Stack::fill(),
            page: theme.video_poster(&video.poster, header.map(|(icon, action)| (icon, action.get())),
                Box::new(move |ctx: &mut Context, action: Action| ctx.emit(VideoEvent(token, action)))), video: Some(video), theme: theme.clone(), token,
            state: State::Waiting, shown: None, clips: Some(ctx.air.instances::<VideoClip>()),
        };
        page.load(ctx);
        page.update(None);
        page
    }

    /// Reuse the standard controls without encoding or uploading a derived video.
    pub fn from_player(theme: &Theme, title: &str, player: Player, size: (u32, u32)) -> Self {
        let token = Uuid::new_v4();
        let mut page = Self {
            layout: Stack::fill(),
            page: theme.video_player(title, player.clone(), size,
                Box::new(move |ctx: &mut Context, action: Action| ctx.emit(VideoEvent(token, action)))), video: None, clips: None,
            theme: theme.clone(), token, state: State::Ready { player: player.clone(), scrubbing: None },
            shown: None,
        };
        player.start();
        page.update(None);
        page
    }

    fn load(&mut self, ctx: &mut Context) {
        if let State::Waiting = self.state
            && let Some(video) = &self.video
            && let Some(bytes) = VideoClip::find(ctx, video.clip)
        {
            let player = Media::read(std::io::Cursor::new(bytes)).and_then(|media| {
                let size = media.video.dimensions().ok_or("Media has no video track")?;
                ctx.hardware.player(media.player()).map(|player| (player, size))
            });
            self.clips = None;
            self.state = match player {
                Ok((player, size)) => {
                    player.start();
                    self.page.set_player(player.clone(), size);
                    State::Ready { player, scrubbing: None }
                }
                Err(_) => State::Unplayable,
            };
        }
    }

    fn update(&mut self, playback: Option<Frame>) {
        let duration = Duration::from_millis(self.video.as_ref().map_or(0, |video| video.duration_ms));
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
                    self.page.progress(if duration.is_zero() {0.0} else {
                        (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
                    });
                }
                (badge, playing, position, duration)
            }
        };

        let shown = Shown {
            badge, playing, second: position.as_secs(),
            duration: duration.as_secs_f64().round() as u64,
        };
        if self.shown.as_ref().is_none_or(|old| old.badge != shown.badge) {
            self.page.badge(&self.theme, badge);
        }
        if self.shown.as_ref().is_none_or(|old| old.playing != shown.playing) {
            self.page.playing(&self.theme, playing);
        }
        if self.shown.as_ref().is_none_or(|old| old.second != shown.second) {
            self.page.position(&self.theme, shown.second);
        }
        if self.shown.as_ref().is_none_or(|old| old.duration != shown.duration) {
            self.page.duration(&self.theme, shown.duration);
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
        if self.video.as_ref().is_some_and(|video| clip.pending().id == video.clip) {
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

use crate::design_system::{PlaybackBadge as BadgeKind, PlaybackAction as Action, ScrubPhase as Phase};

#[derive(Debug, Clone, Copy)]
pub(crate) struct VideoEvent(pub(crate) Uuid, pub(crate) Action);
impl Event for VideoEvent {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children { *child.event = Some(*self); }
    }
}
