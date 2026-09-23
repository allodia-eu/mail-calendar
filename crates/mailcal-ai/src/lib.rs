//! `mailcal-ai`: writing style and drafted replies (`docs/ai.md`).
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
//! answers. It depends on neither `mailcal-app` nor `allodia-license`: the app hands it plain
//! values, and Allodia's relay implements [`AiBackend`] from the other side.

mod backend;
mod catalog;
pub mod corpus;
mod draft;
mod endpoint;
pub mod gated;
pub mod language;
mod learn;
mod observe;
mod prompt;
mod style;
mod tool;
mod transport;
pub mod wire;

#[cfg(test)]
mod test_support;

pub use backend::{AiBackend, AiError};
pub use draft::{Draft, DraftRequest, ThreadMessage, draft_reply};
pub use endpoint::{EndpointError, OwnEndpoint};
pub use gated::{GatedBackend, ModeSource};
pub use learn::{
    LearnError, LearnOptions, LearnProgress, Learned, REQUEST_BUDGET_TOKENS, learn_style,
    pick_passages,
};
pub use mailcal_jurisdiction::{Class, Destination, Mode, Refused};
pub use observe::{Correction, correction};
pub use style::{Exemplars, Habit, LanguageStyle, Provenance, SCHEMA_VERSION, StyleGuide};
pub use transport::{HttpRequest, HttpResponse, HttpTransport, TransportFailed};
