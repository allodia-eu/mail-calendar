//! The jurisdiction gate: the in-process check every external dispatch passes **before data
//! leaves the device** (`AGENTS.md`, "Non-negotiables"; `docs/ai.md`, "The gate").
//!
//! A dispatch names where it is going ([`Destination`]). The destination is classified
//! ([`classify`]) and the class is compared with the mode the app runs under ([`Mode`]) by
//! [`gate`]. Nothing here sends anything: the crate decides, and the caller that owns the socket
//! asks it first. `mailcal-ai`'s `GatedBackend` is that caller for AI requests, and the only one
//! there is.
//!
//! **Logging.** A refusal is logged by its two labels ([`Mode::label`], [`Class::label`]) and
//! nothing else. A destination's address is the person's own configuration and never reaches a
//! log (`docs/logging.md`).

use std::fmt;

use serde::{Deserialize, Serialize};

/// How far a dispatch may reach. Stored in the preferences; [`Mode::EuNative`] unless changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Any destination, including one nobody has classified.
    All,
    /// A destination that processes in the EU, whoever operates it.
    EuHosted,
    /// A destination that processes in the EU and is operated from the EU.
    #[default]
    EuNative,
}

impl Mode {
    /// Every mode, for tests and for a picker that lists them.
    pub const ALL: [Self; 3] = [Self::All, Self::EuHosted, Self::EuNative];

    /// The stable label a log line carries.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::EuHosted => "eu-hosted",
            Self::EuNative => "eu-native",
        }
    }
}

/// Where a destination processes data, and from where it is operated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// Processed in the EU by an operator based in the EU.
    EuNative,
    /// Processed in the EU by an operator based outside it.
    EuHosted,
    /// Processed outside the EU.
    NonEu,
    /// Nobody has said.
    Unknown,
}

impl Class {
    /// Every class, for tests and for a picker that lists them.
    pub const ALL: [Self; 4] = [Self::EuNative, Self::EuHosted, Self::NonEu, Self::Unknown];

    /// The stable label a log line carries.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::EuNative => "eu-native",
            Self::EuHosted => "eu-hosted",
            Self::NonEu => "non-eu",
            Self::Unknown => "unknown",
        }
    }
}

/// Where a dispatch is going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// Allodia's relay. It forwards only to providers the gateway classifies as EU-native, so it
    /// is [`Class::EuNative`] by construction.
    AllodiaRelay,
    /// An endpoint the person configured themselves. It is what they declared it to be, and
    /// [`Class::Unknown`] until they declare anything.
    OwnEndpoint {
        /// The person's declaration of where it runs.
        declared: Option<Class>,
    },
}

/// The class of `destination`.
#[must_use]
pub const fn classify(destination: &Destination) -> Class {
    match destination {
        Destination::AllodiaRelay => Class::EuNative,
        Destination::OwnEndpoint {
            declared: Some(class),
        } => *class,
        Destination::OwnEndpoint { declared: None } => Class::Unknown,
    }
}

/// Lets a dispatch to a `class` destination through under `mode`, or refuses it.
///
/// # Errors
///
/// Returns [`Refused`] when the mode does not admit the class. [`Class::Unknown`] passes only
/// under [`Mode::All`]: a destination nobody has classified is not assumed to be anywhere.
pub const fn gate(mode: Mode, class: Class) -> Result<(), Refused> {
    let admitted = match mode {
        Mode::All => true,
        Mode::EuHosted => matches!(class, Class::EuNative | Class::EuHosted),
        Mode::EuNative => matches!(class, Class::EuNative),
    };
    if admitted {
        Ok(())
    } else {
        Err(Refused { mode, class })
    }
}

/// A dispatch the gate stopped, and why: the mode in force and the class it did not admit. A
/// client words its explanation from these two; the core carries no copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused {
    /// The mode in force.
    pub mode: Mode,
    /// The destination's class.
    pub class: Class,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a {} destination is not admitted under {}",
            self.class.label(),
            self.mode.label()
        )
    }
}

impl std::error::Error for Refused {}

#[cfg(test)]
mod tests;
