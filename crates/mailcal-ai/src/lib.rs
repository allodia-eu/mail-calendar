//! `mailcal-ai`: writing style and drafted replies.
//!
//! Learns how a person writes from their own sent mail and drafts replies in that voice. Three
//! layers carry the voice: a [`StyleGuide`] describing the stable traits per language, a handful
//! of [`Exemplars`] quoting the person's own passages, and at draft time the last messages they
//! sent to the same recipient. Nothing is trained.
//!
//! **Every request passes the jurisdiction gate.** The only thing that dispatches is a
//! [`GatedBackend`], and every function here that sends takes one. See [`gated`] for why that is
//! a type rather than a rule.
//!
//! **No socket.** The host implements [`HttpTransport`]; this crate shapes requests and reads
//! answers. It depends on nothing of the app's or the engine's: a caller hands it plain values,
//! and a backend for another destination implements [`AiBackend`] outside this crate.

mod backend;
mod catalog;
mod checklist;
mod closing;
pub mod corpus;
mod draft;
mod draft_instructions;
mod endpoint;
pub mod gated;
pub mod language;
mod lean;
mod learn;
mod prompt;
pub mod report;
mod style;
mod tool;
mod transport;
pub mod wire;

#[cfg(test)]
mod test_support;

pub use backend::{AiBackend, AiError};
pub use checklist::{DraftTask, TaskKind};
pub use draft::{Draft, DraftRequest, THREAD_CHARS, ThreadMessage, draft_reply};
pub use endpoint::{EndpointError, OwnEndpoint, endpoint_errors};
pub use gated::{GatedBackend, ModeSource, admit};
pub use language::LanguageCode;
pub use lean::lean;
pub use learn::{
    LearnError, LearnOptions, LearnProgress, Learned, REQUEST_BUDGET_TOKENS, learn_style,
};
pub use mailcal_jurisdiction::{Class, Destination, Mode, Refused};
pub use style::{Exemplars, Habit, LanguageStyle, Provenance, SCHEMA_VERSION, StyleGuide};
pub use transport::{HttpRequest, HttpResponse, HttpTransport, TransportFailed};
