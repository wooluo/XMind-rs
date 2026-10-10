pub mod col;
pub mod export;
pub mod fmt;
pub mod geo;
pub mod id;
pub mod layout;
pub mod marker;
pub mod model;
pub mod scene;
pub mod templates;
pub mod text;
pub mod theme;

pub use col::Col;
pub use model::{Doc, Marker, Sheet, Structure, Topic};

pub type XRResult<T> = Result<T, XRErr>;

#[derive(Debug, thiserror::Error)]
pub enum XRErr {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("xml: {0}")]
    Xml(String),
    #[error("{0}")]
    Other(String),
}

impl From<quick_xml::Error> for XRErr {
    fn from(e: quick_xml::Error) -> Self {
        XRErr::Xml(e.to_string())
    }
}

pub fn err<T>(msg: impl Into<String>) -> XRResult<T> {
    Err(XRErr::Other(msg.into()))
}
