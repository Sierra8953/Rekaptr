use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to initialize gstreamer")]
    Init(#[from] glib::Error),
    #[error("failed to create element: {0}")]
    Element(String),
    #[error("MPV Error: {0}")]
    Mpv(String),
    #[error("OpenGL Error: {0}")]
    OpenGL(String),
    #[error("Interop Error: {0}")]
    Interop(String),
    #[error("failed to set pipeline state")]
    StateChange(#[from] gstreamer::StateChangeError),
    #[error("failed to cast element")]
    Cast,
    #[error("failed to get caps from appsink")]
    Caps,
    #[error("failed to get property: {0}")]
    Property(String),
    #[error("failed to parse URI: {0}")]
    Uri(String),
    #[error("app sink error: {0}")]
    AppSink(String),
    #[error("failed to calculate video duration")]
    Duration,
    #[error("failed to sync with playback")]
    Sync,
    #[error("failed to lock internal sync primitive")]
    Lock,
    #[error("invalid framerate: {0}")]
    Framerate(f64),
    #[error("Generic Bus Error")]
    Bus,
}
