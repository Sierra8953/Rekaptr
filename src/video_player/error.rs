use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("MPV Error: {0}")]
    Mpv(String),
    #[error("OpenGL Error: {0}")]
    OpenGL(String),
    #[error("Interop Error: {0}")]
    Interop(String),
    #[error("failed to lock internal sync primitive")]
    Lock,
    #[error("Generic Bus Error")]
    Bus,
}
