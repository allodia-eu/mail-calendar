//! The jurisdiction gate: the in-process check every external dispatch passes **before data
//! leaves the device** (`AGENTS.md`, "Non-negotiables").
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

/// How far a dispatch may reach. [`Mode::EuNative`] unless the person chooses otherwise.
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
    let limit = match mode {
        Mode::All => return Ok(()),
        Mode::EuHosted => Limit::EuHosted,
        Mode::EuNative => Limit::EuNative,
    };
    match (class, limit) {
        (Class::EuNative, _) | (Class::EuHosted, Limit::EuHosted) => Ok(()),
        (Class::EuHosted, Limit::EuNative) => Err(Refused::EuHosted),
        (Class::NonEu, mode) => Err(Refused::NonEu { mode }),
        (Class::Unknown, mode) => Err(Refused::Unknown { mode }),
    }
}

/// A mode that refuses something: every mode but [`Mode::All`], which admits everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Limit {
    /// [`Mode::EuHosted`].
    EuHosted,
    /// [`Mode::EuNative`].
    EuNative,
}

impl Limit {
    /// The mode it is.
    #[must_use]
    pub const fn mode(self) -> Mode {
        match self {
            Self::EuHosted => Mode::EuHosted,
            Self::EuNative => Mode::EuNative,
        }
    }
}

/// A dispatch the gate stopped: the class it did not admit, with the mode in force where more
/// than one mode refuses that class. Only what the gate can refuse is expressible: [`Mode::All`]
/// refuses nothing, and no mode refuses [`Class::EuNative`]. A client words its explanation from
/// this; the core carries no copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// A destination in the EU run by a company based outside it, which only
    /// [`Mode::EuNative`] refuses.
    EuHosted,
    /// A destination outside the EU.
    NonEu {
        /// The mode in force.
        mode: Limit,
    },
    /// A destination nobody has classified.
    Unknown {
        /// The mode in force.
        mode: Limit,
    },
}

impl Refused {
    /// The mode in force.
    #[must_use]
    pub const fn mode(self) -> Mode {
        match self {
            Self::EuHosted => Mode::EuNative,
            Self::NonEu { mode } | Self::Unknown { mode } => mode.mode(),
        }
    }

    /// The class the gate did not admit.
    #[must_use]
    pub const fn class(self) -> Class {
        match self {
            Self::EuHosted => Class::EuHosted,
            Self::NonEu { .. } => Class::NonEu,
            Self::Unknown { .. } => Class::Unknown,
        }
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a {} destination is not admitted under {}",
            self.class().label(),
            self.mode().label()
        )
    }
}

impl std::error::Error for Refused {}

#[cfg(test)]
mod tests;
