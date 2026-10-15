use pelican_ui::image::RgbaImage;

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::audio::{self, AudioOut};
use super::video::{Frame, Frames, VideoTrack};
pub use super::video::Unplayable;

/// Gaps between draws longer than this mean the page wasn't showing (it was
/// left for another, or the app was in the background), so it pauses.
const AWAY: Duration = Duration::from_secs(1);
/// The most the clock moves in one draw when there's no sound to follow.
const MAX_STEP: f64 = 0.1;

/// Plays an MP4's H.264 picture and AAC sound inside the app: call [`tick`]
/// each time the page draws and show [`frame`].
///
/// [`tick`]: Player::tick
/// [`frame`]: Player::frame
pub struct Player {
    mp4: Arc<[u8]>,
    track: Arc<VideoTrack>,
    frames: Frames,
    /// The device's output, if the video has sound and there's an output.
    audio: Option<AudioOut>,
    /// Where the sound now playing started, in seconds.
    sound_from: f64,
    /// Where it's at, in seconds.
    position: f64,
    playing: bool,
    ended: bool,
    showing: Option<Frame>,
    /// The next frame shows whatever its time (it's the first after a seek).
    fresh: bool,
    last_tick: Option<Instant>,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Player({:.2}/{:.2}s, {})", self.position, self.track.duration, if self.playing { "playing" } else { "paused" })
    }
}

impl Player {
    /// A player for `mp4`, paused at the start.
    pub fn new(mp4: Arc<[u8]>) -> Result<Self, Unplayable> {
        let audio = audio::has_sound(&mp4).then(AudioOut::open).flatten();
        Self::with_audio(mp4, audio)
    }

    pub(crate) fn with_audio(mp4: Arc<[u8]>, audio: Option<AudioOut>) -> Result<Self, Unplayable> {
        let track = Arc::new(VideoTrack::open(&mp4)?);
        let frames = Frames::start(mp4.clone(), track.clone(), 0.0);
        Ok(Player { mp4, track, frames, audio, sound_from: 0.0, position: 0.0, playing: false, ended: false, showing: None, fresh: true, last_tick: None })
    }

    /// How long it is, in seconds.
    pub fn duration(&self) -> f64 { self.track.duration }
    /// Where it's at, in seconds.
    pub fn position(&self) -> f64 { self.position }
    pub fn is_playing(&self) -> bool { self.playing }
    /// The picture to show now, once there is one.
    pub fn frame(&self) -> Option<Arc<RgbaImage>> { self.showing.as_ref().map(|f| f.image.clone()) }

    /// Plays from where it's at (from the start again, if it had ended).
    pub fn play(&mut self) {
        if self.playing { return; }
        if self.ended { self.seek(0.0); }
        self.playing = true;
        self.last_tick = None;
        if let Some(audio) = &self.audio {
            self.sound_from = self.position;
            audio.start(self.mp4.clone(), self.position);
            audio.keep_alive();
            audio.set_playing(true);
        }
    }

    pub fn pause(&mut self) {
        if !self.playing { return; }
        self.position = self.clock();
        self.playing = false;
        if let Some(audio) = &self.audio { audio.set_playing(false); }
    }

    /// Goes to `seconds` in (showing the frame there, even while paused).
    pub fn seek(&mut self, seconds: f64) {
        let to = seconds.clamp(0.0, self.track.duration);
        self.position = to;
        self.ended = false;
        self.fresh = true;
        self.frames = Frames::start(self.mp4.clone(), self.track.clone(), to);
        if self.playing && let Some(audio) = &self.audio {
            self.sound_from = to;
            audio.start(self.mp4.clone(), to);
        }
    }

    /// Where it's at while playing: as far as the sound has played, or
    /// moved on by the time since the last draw once there's no sound.
    fn clock(&self) -> f64 {
        match &self.audio {
            Some(audio) if !audio.is_finished() => self.sound_from + audio.played(),
            _ => self.position,
        }
    }

    /// Moves it on to now. Call it each time the page draws; it's true when
    /// there's a new frame to show.
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let since = self.last_tick.map(|last| now - last).unwrap_or_default();
        self.last_tick = Some(now);
        if let Some(audio) = &self.audio { audio.keep_alive(); }
        if since > AWAY { self.pause(); }

        if self.playing {
            let following_sound = matches!(&self.audio, Some(audio) if !audio.is_finished());
            self.position = match following_sound {
                true => self.clock(),
                false => self.position + since.as_secs_f64().min(MAX_STEP),
            }.min(self.track.duration);
        }

        let mut changed = false;
        if self.fresh && let Some(frame) = self.frames.take() {
            self.showing = Some(frame);
            self.fresh = false;
            changed = true;
        }
        // Every frame that's due; the last of them shows (any others were
        // late, and are skipped).
        while !self.fresh && self.frames.peek().is_some_and(|next| next.time <= self.position + 0.001) {
            self.showing = self.frames.take();
            changed = true;
        }

        let sound_done = self.audio.as_ref().is_none_or(|audio| audio.is_finished());
        if self.playing && sound_done && self.frames.is_done() && self.position >= self.showing.as_ref().map(|f| f.time).unwrap_or(0.0) {
            self.playing = false;
            self.ended = true;
            self.position = self.track.duration;
            if let Some(audio) = &self.audio { audio.set_playing(false); }
        }
        changed
    }
}
