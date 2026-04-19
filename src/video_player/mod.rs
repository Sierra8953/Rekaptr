#[allow(dead_code)]
mod element;
#[allow(dead_code)]
mod error;
#[allow(dead_code)]
mod video;
#[allow(dead_code)]
mod window_helper;

pub use element::video;
pub use error::Error;
pub use video::{SendHandle, Video, VideoOptions};
