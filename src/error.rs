use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error `{0}`")]
    IoError(io::Error),
    #[error("Parse Error `{0}`")]
    ParseError(serde_json::Error),
}
