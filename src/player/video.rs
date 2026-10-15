//! A video's picture: its H.264 track read out of the MP4, decoded a few
//! frames ahead on a thread of its own.

use pelican_ui::image::RgbaImage;
use pelican_ui::image::imageops;
use mp4::{Mp4Reader, TrackType, MediaType};
use openh264::decoder::{Decoder, DecoderConfig, DecodedYUV, Flush};
use openh264::formats::YUVSource;
use openh264::OpenH264API;

use std::collections::BinaryHeap;
use std::cmp::Reverse;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread;

/// Frames decoded ahead of the one showing, at most.
const AHEAD: usize = 4;

/// A frame of the video, and when it shows (in seconds from the start).
#[derive(Clone)]
pub struct Frame {
    pub time: f64,
    pub image: Arc<RgbaImage>,
}

/// What's needed to decode a video's picture, found once when it's opened.
#[derive(Debug)]
pub struct VideoTrack {
    track: u32,
    timescale: f64,
    /// Where the video starts in the track (from its edit list): the time
    /// in the track's timescale that shows at `delay` seconds.
    shift: i64,
    delay: f64,
    /// Bytes before each NAL unit that give its length.
    length_size: usize,
    /// The SPS and PPS, ready to give the decoder.
    headers: Vec<u8>,
    /// Every sample, in decoding order.
    samples: Vec<SampleInfo>,
    /// Clockwise turns of 90° that make it the right way up.
    turns: u8,
    /// How long it is, in seconds.
    pub duration: f64,
}

#[derive(Debug, Clone, Copy)]
struct SampleInfo {
    /// When it shows, in the track's timescale.
    time: i64,
    is_sync: bool,
}

/// Why a video can't be played.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unplayable {
    /// It isn't an MP4 that can be read.
    Unreadable,
    /// It has no H.264 picture (it may be HEVC, which isn't supported).
    Unsupported,
}

impl VideoTrack {
    pub fn open(mp4: &[u8]) -> Result<Self, Unplayable> {
        let mut reader = Mp4Reader::read_header(Cursor::new(mp4), mp4.len() as u64).map_err(|_| Unplayable::Unreadable)?;
        let track = reader.tracks().values()
            .find(|t| t.track_type().ok() == Some(TrackType::Video))
            .ok_or(Unplayable::Unsupported)?;
        if track.media_type().ok() != Some(MediaType::H264) { return Err(Unplayable::Unsupported); }

        let avc1 = track.trak.mdia.minf.stbl.stsd.avc1.as_ref().ok_or(Unplayable::Unsupported)?;
        let mut headers = Vec::new();
        for nal in avc1.avcc.sequence_parameter_sets.iter().chain(&avc1.avcc.picture_parameter_sets) {
            headers.extend_from_slice(&[0, 0, 0, 1]);
            headers.extend_from_slice(&nal.bytes);
        }
        let length_size = avc1.avcc.length_size_minus_one as usize % 4 + 1;
        let id = track.track_id();
        let timescale = track.timescale().max(1) as f64;
        let matrix = &track.trak.tkhd.matrix;
        let turns = turns(matrix.a, matrix.b);

        let (shift, delay) = edits(track, reader.timescale().max(1) as f64);
        let count = reader.sample_count(id).map_err(|_| Unplayable::Unreadable)?;
        let mut samples = Vec::with_capacity(count as usize);
        let mut end = 0;
        for i in 1..=count {
            let Ok(Some(sample)) = reader.read_sample(id, i) else { continue };
            let time = sample.start_time as i64 + sample.rendering_offset as i64;
            end = end.max(time + sample.duration as i64);
            samples.push(SampleInfo { time, is_sync: sample.is_sync });
        }
        if samples.is_empty() { return Err(Unplayable::Unreadable); }

        let mut track = VideoTrack { track: id, timescale, shift, delay, length_size, headers, samples, turns, duration: 0.0 };
        track.duration = track.seconds(end);
        Ok(track)
    }

    /// A time in the track's timescale, in seconds from the video's start.
    fn seconds(&self, time: i64) -> f64 {
        (time - self.shift) as f64 / self.timescale + self.delay
    }

    /// Seconds from the video's start, in the track's timescale.
    fn time(&self, seconds: f64) -> i64 {
        ((seconds - self.delay) * self.timescale).round() as i64 + self.shift
    }

    /// When the first frame there is shows, in seconds.
    fn start(&self) -> f64 {
        self.seconds(self.samples.iter().map(|s| s.time).min().unwrap_or(0))
    }
}

/// From a track's edit list: the time in its timescale where it starts
/// showing, and how many seconds it waits before that.
fn edits(track: &mp4::Mp4Track, movie_timescale: f64) -> (i64, f64) {
    let Some(entries) = track.trak.edts.as_ref().and_then(|e| e.elst.as_ref()).map(|l| &l.entries) else { return (0, 0.0) };
    let mut delay = 0.0;
    for entry in entries {
        // An empty edit (its media time is -1) is a wait.
        if entry.media_time == u32::MAX as u64 || entry.media_time == u64::MAX {
            delay += entry.segment_duration as f64 / movie_timescale;
        } else {
            return (entry.media_time as i64, delay);
        }
    }
    (0, delay)
}

/// How many clockwise quarter turns a track's matrix asks for, from its
/// first row (`a`, `b`).
fn turns(a: i32, b: i32) -> u8 {
    let degrees = (b as f64).atan2(a as f64).to_degrees();
    (((degrees / 90.0).round() as i64).rem_euclid(4)) as u8
}

/// Frames decoded on a thread of their own, a few ahead of the one showing.
/// Dropping it stops the thread.
pub struct Frames {
    frames: Receiver<Frame>,
    stop: Arc<AtomicBool>,
    /// The frame received but not yet due.
    next: Option<Frame>,
    done: bool,
}

impl Frames {
    /// Frames from `from` seconds in (starting at the keyframe before it,
    /// whose frames before `from` aren't kept).
    pub fn start(mp4: Arc<[u8]>, track: Arc<VideoTrack>, from: f64) -> Self {
        let (send, frames) = mpsc::sync_channel(AHEAD);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        thread::Builder::new().name("chk video".into()).spawn(move || decode(&mp4, &track, from, &send, &stopping)).ok();
        Frames { frames, stop, next: None, done: false }
    }

    /// The next frame, without taking it.
    pub fn peek(&mut self) -> Option<&Frame> {
        if self.next.is_none() && !self.done {
            match self.frames.try_recv() {
                Ok(frame) => self.next = Some(frame),
                Err(TryRecvError::Disconnected) => self.done = true,
                Err(TryRecvError::Empty) => {},
            }
        }
        self.next.as_ref()
    }

    pub fn take(&mut self) -> Option<Frame> {
        self.peek();
        self.next.take()
    }

    /// Every frame has been taken.
    pub fn is_done(&mut self) -> bool {
        self.peek().is_none() && self.done
    }
}

impl Drop for Frames {
    fn drop(&mut self) { self.stop.store(true, Ordering::Relaxed); }
}

/// Decodes the frames from `from` and sends them, until they're all sent or
/// it's told to stop.
fn decode(mp4: &[u8], track: &VideoTrack, from: f64, send: &SyncSender<Frame>, stop: &AtomicBool) {
    let Ok(mut reader) = Mp4Reader::read_header(Cursor::new(mp4), mp4.len() as u64) else { return };
    let config = DecoderConfig::new().flush_after_decode(Flush::NoFlush);
    let Ok(mut decoder) = Decoder::with_api_config(OpenH264API::from_source(), config) else { return };
    let _ = decoder.decode(&track.headers);

    // Frames before `from` are decoded (to decode the ones after) but not
    // kept, except the last of them if nothing at or after `from` comes.
    let from = from.max(track.start());
    let target = track.time(from);
    let first = track.samples.iter()
        .rposition(|s| s.is_sync && s.time <= target)
        .or_else(|| track.samples.iter().position(|s| s.is_sync))
        .unwrap_or(0);

    // The decoder gives frames in the order they show: each takes the
    // earliest time of those given to it and not yet out.
    let mut waiting: BinaryHeap<Reverse<i64>> = BinaryHeap::new();
    let mut held: Option<Frame> = None;
    let out = |time: i64, image: RgbaImage, held: &mut Option<Frame>| -> bool {
        let frame = Frame { time: track.seconds(time), image: Arc::new(turn(image, track.turns)) };
        if time < target {
            *held = Some(frame);
            return true;
        }
        if let Some(earlier) = held.take() && time > target && send.send(earlier).is_err() { return false; }
        send.send(frame).is_ok()
    };

    let mut annex_b = Vec::new();
    for (i, info) in track.samples.iter().enumerate().skip(first) {
        if stop.load(Ordering::Relaxed) { return; }
        let Ok(Some(sample)) = reader.read_sample(track.track, i as u32 + 1) else { continue };
        to_annex_b(&sample.bytes, track.length_size, &mut annex_b);
        waiting.push(Reverse(info.time));
        match decoder.decode(&annex_b) {
            Ok(Some(yuv)) => {
                let Some(Reverse(time)) = waiting.pop() else { continue };
                if !out(time, rgba(&yuv), &mut held) { return; }
            },
            Ok(None) => {},
            // A frame that can't be decoded never comes out.
            Err(_) => { remove(&mut waiting, info.time); },
        }
    }
    if let Ok(rest) = decoder.flush_remaining() {
        for yuv in rest {
            if stop.load(Ordering::Relaxed) { return; }
            let Some(Reverse(time)) = waiting.pop() else { break };
            if !out(time, rgba(&yuv), &mut held) { return; }
        }
    }
    if let Some(last) = held { let _ = send.send(last); }
}

fn remove(waiting: &mut BinaryHeap<Reverse<i64>>, time: i64) {
    let mut all = std::mem::take(waiting).into_vec();
    if let Some(i) = all.iter().position(|t| t.0 == time) { all.swap_remove(i); }
    *waiting = all.into();
}

/// MP4's length-prefixed NAL units as the start-code-prefixed stream the
/// decoder reads.
fn to_annex_b(sample: &[u8], length_size: usize, annex_b: &mut Vec<u8>) {
    annex_b.clear();
    let mut rest = sample;
    while rest.len() >= length_size {
        let length = rest[..length_size].iter().fold(0usize, |n, b| n << 8 | *b as usize);
        rest = &rest[length_size..];
        let length = length.min(rest.len());
        annex_b.extend_from_slice(&[0, 0, 0, 1]);
        annex_b.extend_from_slice(&rest[..length]);
        rest = &rest[length..];
    }
}

fn rgba(yuv: &DecodedYUV<'_>) -> RgbaImage {
    let (width, height) = yuv.dimensions();
    let mut pixels = vec![0; width * height * 4];
    yuv.write_rgba8(&mut pixels);
    RgbaImage::from_raw(width as u32, height as u32, pixels).unwrap_or_default()
}

fn turn(image: RgbaImage, turns: u8) -> RgbaImage {
    match turns {
        1 => imageops::rotate90(&image),
        2 => imageops::rotate180(&image),
        3 => imageops::rotate270(&image),
        _ => image,
    }
}
