#[derive(thiserror::Error,Debug)]
pub enum Error {
    #[error("neither API_KEY environment variable nor --key parameter supplied")]
    NoAPIKey,
    #[error("file {0} not found")]
    NotFound(String),
    #[error("path {0} exists and is not a directory")]
    PathExists(String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("convert process failed, code {0}")]
    Failure(i32),
    #[error(transparent)]
    HTTPError(#[from] hyper::http::Error),
    #[error(transparent)]
    HyperError(#[from] hyper::Error),
    #[error("failed to obtain icon, code returned {0}")]
    FetchFailure(hyper::StatusCode),
    #[error("failed to parse resolution")]
    ResolutionParseError(#[from] std::num::ParseIntError),
    #[error("fail is too large")]
    FileIsTooLarge(),
    #[error(transparent)]
    PersistError(#[from] tempfile::PersistError),
    #[error(transparent)]
    ModeError(#[from] file_mode::ModeError),
    #[error("file is not modified")]
    NotModified(),
}
