use compass_core::algorithm::search::search_error::SearchError;
use geo::Coord;

#[derive(thiserror::Error, Debug)]
pub enum PluginError {
    #[error("failed to parse {0} as {1}")]
    ParseError(&'static str, &'static str),
    #[error("missing field {0}")]
    MissingField(&'static str),
    #[error("error with parsing inputs: {0}")]
    InputError(&'static str),
    #[error("error with building plugin")]
    BuildError,
    #[error("nearest vertex not found for coord {0:?}")]
    NearestVertexNotFound(Coord),
    #[error(transparent)]
    FileReadError(#[from] std::io::Error),
    #[error("error with reading file")]
    CsvReadError(#[from] csv::Error),
    #[error("geometry missing for edge id {0}")]
    GeometryMissing(u64),
    #[error("uuid missing for edge id {0}")]
    UUIDMissing(usize),
    #[error("error during search")]
    SearchError(#[from] SearchError),
    #[error("unexpected error {0}")]
    InternalError(String),
}
