//! A credential that redacts itself.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A secret the flow handles: the poll token, or the app password it yields. `Debug` never shows
/// it, so a value holding one can still derive `Debug` without it reaching a log.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    /// Wraps `value`.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The secret itself, for the request that presents it and never for anything else.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}
