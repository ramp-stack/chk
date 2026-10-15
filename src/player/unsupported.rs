//! The web can't decode videos here: every video is unplayable.

use pelican_ui::image::RgbaImage;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unplayable { Unsupported }

#[derive(Debug)]
pub struct Player;

impl Player {
    pub fn new(_mp4: Arc<[u8]>) -> Result<Self, Unplayable> { Err(Unplayable::Unsupported) }
    pub fn duration(&self) -> f64 { 0.0 }
    pub fn position(&self) -> f64 { 0.0 }
    pub fn is_playing(&self) -> bool { false }
    pub fn frame(&self) -> Option<Arc<RgbaImage>> { None }
    pub fn play(&mut self) {}
    pub fn pause(&mut self) {}
    pub fn seek(&mut self, _seconds: f64) {}
    pub fn tick(&mut self) -> bool { false }
}
