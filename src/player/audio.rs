//! A video's sound: its AAC track decoded on a thread of its own, into a
//! buffer the device's audio output plays from.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample, SampleFormat, Stream, StreamConfig};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::Time;

use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// How much sound is decoded ahead, at most.
const AHEAD: f64 = 0.5;
/// The sound stops this long after the page last drew (it's been left, or
/// the app's in the background).
const ALIVE: Duration = Duration::from_millis(300);

/// The device's audio output, playing whatever's decoded into it.
pub struct AudioOut {
    shared: Arc<Shared>,
    _stream: Option<Stream>,
    pub rate: u32,
    pub channels: usize,
}

/// What the output and the decoding share.
struct Shared {
    buffer: Mutex<Buffer>,
    playing: AtomicBool,
    /// Until when (in milliseconds since `epoch`) it may play.
    alive_until: AtomicU64,
    epoch: Instant,
}

#[derive(Default)]
struct Buffer {
    /// Which run of decoding the sound is from (each start or seek is a new
    /// one, so sound from an earlier one is never played).
    session: u64,
    /// Sound to play, interleaved, at the output's rate and channels.
    samples: VecDeque<f32>,
    /// Frames played since the session started.
    played: u64,
    /// The session's sound has all been decoded.
    finished: bool,
}

impl Shared {
    fn new() -> Self {
        Shared { buffer: Mutex::new(Buffer::default()), playing: AtomicBool::new(false), alive_until: AtomicU64::new(0), epoch: Instant::now() }
    }

    fn now(&self) -> u64 { self.epoch.elapsed().as_millis() as u64 }

    /// Fills `data` (interleaved, `channels` to a frame) with what's next
    /// to play, or silence.
    fn fill<T: SizedSample + FromSample<f32>>(&self, data: &mut [T], channels: usize) {
        let mut filled = 0;
        let alive = self.playing.load(Ordering::Relaxed) && self.now() <= self.alive_until.load(Ordering::Relaxed);
        // Never waits on the decoding: if it's busy, this bit is silent.
        if alive && let Ok(mut buffer) = self.buffer.try_lock() {
            let frames = (buffer.samples.len() / channels).min(data.len() / channels);
            for (out, sample) in data.iter_mut().zip(buffer.samples.drain(..frames * channels)) {
                *out = T::from_sample(sample);
            }
            buffer.played += frames as u64;
            filled = frames * channels;
        }
        data[filled..].fill(T::EQUILIBRIUM);
    }
}

impl AudioOut {
    /// The default output, if there is one that works.
    pub fn open() -> Option<Self> {
        let device = cpal::default_host().default_output_device()?;
        let supported = device.default_output_config().ok()?;
        let config: StreamConfig = supported.config();
        let shared = Arc::new(Shared::new());
        let channels = config.channels.max(1) as usize;
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, &config, shared.clone()),
            SampleFormat::I16 => build::<i16>(&device, &config, shared.clone()),
            SampleFormat::U16 => build::<u16>(&device, &config, shared.clone()),
            SampleFormat::I32 => build::<i32>(&device, &config, shared.clone()),
            SampleFormat::F64 => build::<f64>(&device, &config, shared.clone()),
            _ => None,
        }?;
        stream.play().ok()?;
        Some(AudioOut { shared, _stream: Some(stream), rate: config.sample_rate, channels })
    }

    pub fn set_playing(&self, playing: bool) { self.shared.playing.store(playing, Ordering::Relaxed); }

    /// Keeps it playing a little longer (it's called each time the page
    /// draws, so the sound stops soon after the page is gone).
    pub fn keep_alive(&self) {
        self.shared.alive_until.store(self.shared.now() + ALIVE.as_millis() as u64, Ordering::Relaxed);
    }

    /// Seconds played since `start` was last called.
    pub fn played(&self) -> f64 {
        self.shared.buffer.lock().map(|b| b.played as f64 / self.rate as f64).unwrap_or(0.0)
    }

    /// Everything decoded since `start` was last called has been played.
    pub fn is_finished(&self) -> bool {
        self.shared.buffer.lock().map(|b| b.finished && b.samples.is_empty()).unwrap_or(true)
    }

    /// Starts playing the sound from `from` seconds into `mp4` (dropping
    /// anything from before), decoding it on a thread of its own.
    pub fn start(&self, mp4: Arc<[u8]>, from: f64) {
        let session = match self.shared.buffer.lock() {
            Ok(mut buffer) => {
                buffer.session += 1;
                buffer.samples.clear();
                buffer.played = 0;
                buffer.finished = false;
                buffer.session
            },
            Err(_) => return,
        };
        let (shared, rate, channels) = (self.shared.clone(), self.rate, self.channels);
        thread::Builder::new().name("chk audio".into()).spawn(move || {
            decode(&mp4, from, rate, channels, &shared, session);
            if let Ok(mut buffer) = shared.buffer.lock() && buffer.session == session { buffer.finished = true; }
        }).ok();
    }
}

impl Drop for AudioOut {
    /// Ends the session, so its decoding stops.
    fn drop(&mut self) {
        if let Ok(mut buffer) = self.shared.buffer.lock() { buffer.session += 1; }
    }
}

fn build<T: SizedSample + FromSample<f32>>(device: &cpal::Device, config: &StreamConfig, shared: Arc<Shared>) -> Option<Stream> {
    let channels = config.channels.max(1) as usize;
    device.build_output_stream(*config, move |data: &mut [T], _| shared.fill(data, channels), |_| {}, None).ok()
}

/// Whether `mp4` has sound that can be played.
pub fn has_sound(mp4: &Arc<[u8]>) -> bool {
    open(mp4).is_some()
}

fn open(mp4: &Arc<[u8]>) -> Option<(Box<dyn FormatReader>, u32)> {
    let source = MediaSourceStream::new(Box::new(Cursor::new(mp4.clone())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp4");
    let reader = symphonia::default::get_probe().probe(&hint, source, FormatOptions::default(), MetadataOptions::default()).ok()?;
    let codecs = symphonia::default::get_codecs();
    let track = reader.tracks().iter().find(|track| match &track.codec_params {
        Some(CodecParameters::Audio(params)) => codecs.get_audio_decoder(params.codec).is_some(),
        _ => false,
    })?.id;
    Some((reader, track))
}

/// Decodes the sound from `from` into the buffer, until it's all decoded or
/// another session starts.
fn decode(mp4: &Arc<[u8]>, from: f64, rate: u32, channels: usize, shared: &Shared, session: u64) {
    let Some((mut reader, track)) = open(mp4) else { return };
    let Some(info) = reader.tracks().iter().find(|t| t.id == track) else { return };
    let time_base = info.time_base;
    let Some(CodecParameters::Audio(params)) = info.codec_params.clone() else { return };
    let Ok(mut decoder) = symphonia::default::get_codecs().make_audio_decoder(&params, &AudioDecoderOptions::default()) else { return };

    // Sound before `from` (the seek lands at the packet before it) isn't kept.
    let mut skipping = from > 0.0;
    if skipping && let Some(time) = Time::try_from_secs_f64(from) && reader.seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(track) }).is_err() {
        return;
    }
    let seconds = |ts: i64| time_base.map(|tb| ts as f64 * tb.numer.get() as f64 / tb.denom.get() as f64).unwrap_or(0.0);

    let mut resampler: Option<Resampler> = None;
    let mut decoded = Vec::new();
    let mut out = Vec::new();
    let current = || shared.buffer.lock().map(|b| b.session == session).unwrap_or(false);
    loop {
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(_) => break,
        };
        if packet.track_id != track { continue; }
        let Ok(audio) = decoder.decode(&packet) else { continue };
        let (in_rate, in_channels) = (audio.spec().rate(), audio.spec().channels().count().max(1));
        audio.copy_to_vec_interleaved::<f32>(&mut decoded);

        if skipping {
            let start = seconds(packet.pts.get());
            let frames = decoded.len() / in_channels;
            let skip = (((from - start) * in_rate as f64).round().max(0.0) as usize).min(frames);
            decoded.drain(..skip * in_channels);
            skipping = skip == frames;
        }

        let resampler = resampler.get_or_insert_with(|| Resampler::new(in_rate, in_channels, rate, channels));
        out.clear();
        resampler.push(&decoded, &mut out);

        // Waits while plenty is decoded ahead, and stops if a new session
        // started (a seek, or the player's gone).
        loop {
            if !current() { return; }
            let ahead = shared.buffer.lock().map(|b| b.samples.len()).unwrap_or(0) as f64 / (rate as usize * channels) as f64;
            if ahead < AHEAD { break; }
            thread::sleep(Duration::from_millis(10));
        }
        match shared.buffer.lock() {
            Ok(mut buffer) if buffer.session == session => buffer.samples.extend(out.iter().copied()),
            _ => return,
        }
    }
}

/// Changes sound's rate and channels to the output's (by straight lines
/// between samples, which is plenty for a video's sound).
struct Resampler {
    in_channels: usize,
    out_channels: usize,
    /// Input frames per output frame.
    step: f64,
    /// Where the next output frame falls, in input frames into `pending`.
    position: f64,
    /// Input not yet used up, interleaved.
    pending: Vec<f32>,
}

impl Resampler {
    fn new(in_rate: u32, in_channels: usize, out_rate: u32, out_channels: usize) -> Self {
        Resampler { in_channels, out_channels, step: in_rate.max(1) as f64 / out_rate.max(1) as f64, position: 0.0, pending: Vec::new() }
    }

    fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        self.pending.extend_from_slice(input);
        let (inc, outc) = (self.in_channels, self.out_channels);
        let frames = self.pending.len() / inc;
        while self.position + 1.0 < frames as f64 {
            let i = self.position as usize;
            let t = (self.position - i as f64) as f32;
            let (a, b) = (&self.pending[i * inc..(i + 1) * inc], &self.pending[(i + 1) * inc..(i + 2) * inc]);
            for c in 0..outc {
                // Mono goes to every speaker; otherwise channels go to their
                // own, and speakers the sound doesn't have stay quiet.
                let from = match (inc, c < inc) { (1, _) => Some(0), (_, true) => Some(c), _ => None };
                out.push(from.map(|f| a[f] + (b[f] - a[f]) * t).unwrap_or(0.0));
            }
            self.position += self.step;
        }
        let used = (self.position as usize).min(frames);
        self.pending.drain(..used * inc);
        self.position -= used as f64;
    }
}
