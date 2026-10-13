//! An offline observer, never a financial writer. Agreement between exports
//! does not establish that either export contains the correct accounting.
#![forbid(unsafe_code)]
pub mod compare;
pub mod control;
pub mod encoding;
pub mod explanations;
pub mod model;
pub mod money;
pub mod parse;
pub mod relationships;
pub mod snapshot;
pub mod strict_json;
pub mod validate;
pub mod work;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SEMANTICS: &str = "siediff.sie4c-core.v2";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Error {
    pub code: String,
    pub line: Option<usize>,
    pub message: String,
}
impl Error {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            line: None,
            message: message.into(),
        }
    }
    pub fn at(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {:?}: {}", self.code, self.line, self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn digest<T: Serialize>(v: &T) -> Result<String> {
    Ok(sha256(&serde_json::to_vec(v).map_err(|_| {
        Error::new("SERIALIZE", "Cannot serialize typed result")
    })?))
}
pub fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
