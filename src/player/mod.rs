//! Plays videos inside the app: H.264 pictures (decoded with OpenH264) and
//! AAC sound (decoded with Symphonia, played through the device's output with
//! cpal). The web has neither, so there a video can't be played.

#[cfg(not(target_arch = "wasm32"))]
mod video;
#[cfg(not(target_arch = "wasm32"))]
mod audio;
#[cfg(not(target_arch = "wasm32"))]
mod engine;
#[cfg(not(target_arch = "wasm32"))]
pub use engine::Player;

#[cfg(target_arch = "wasm32")]
mod unsupported;
#[cfg(target_arch = "wasm32")]
pub use unsupported::Player;
