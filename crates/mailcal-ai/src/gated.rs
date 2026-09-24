//! The gated backend: the one door an AI request leaves through.
//!
//! Every function in this crate that dispatches takes a [`GatedBackend`], and the app holds one of
//! those rather than any [`AiBackend`]. Its [`chat`](GatedBackend::chat) asks the jurisdiction
//! gate first and refuses before the inner backend is reached, so the check sits in process, on
//! the path, where it cannot be routed around (`AGENTS.md`, "Non-negotiables").

use std::{fmt, sync::Arc};

use mailcal_jurisdiction::{Destination, Mode, Refused, classify, gate};

use crate::{
    AiBackend, AiError,
    endpoint::{OpenAiCompatibleBackend, OwnEndpoint},
    transport::HttpTransport,
    wire::{ChatRequest, ChatResponse},
};

/// Reads the mode in force at the moment of a request, so a change of preference applies to the
/// next request without rebuilding the backend.
pub type ModeSource = Arc<dyn Fn() -> Mode + Send + Sync>;

/// Asks the gate whether a dispatch to `destination` may leave under the mode in force now: the
/// check [`GatedBackend::chat`] makes, for a dispatch that is not a chat request.
///
/// # Errors
///
/// Returns the [`Refused`] the dispatch meets.
pub fn admit(destination: Destination, mode: &ModeSource) -> Result<(), Refused> {
    gate(mode(), classify(&destination))
}

/// A backend behind the jurisdiction gate.
pub struct GatedBackend {
    inner: Box<dyn AiBackend>,
    destination: Destination,
    mode: ModeSource,
    /// The own endpoint's model name; `None` for any other backend.
    model: Option<String>,
}

impl GatedBackend {
    /// Puts `inner`, which sends to `destination`, behind the gate.
    ///
    /// The destination is the caller's statement of where `inner` sends, and the gate is only as
    /// true as it: the relay's constructor passes [`Destination::AllodiaRelay`] and nothing else
    /// may.
    #[must_use]
    pub fn new(inner: Box<dyn AiBackend>, destination: Destination, mode: ModeSource) -> Self {
        Self {
            inner,
            destination,
            mode,
            model: None,
        }
    }

    /// An own endpoint behind the gate, classed by the person's declaration.
    #[must_use]
    pub fn own_endpoint(
        endpoint: OwnEndpoint,
        transport: Box<dyn HttpTransport>,
        mode: ModeSource,
    ) -> Self {
        let destination = Destination::OwnEndpoint {
            declared: endpoint.declared(),
        };
        let model = endpoint.model().to_owned();
        Self {
            model: Some(model),
            ..Self::new(
                Box::new(OpenAiCompatibleBackend::new(endpoint, transport)),
                destination,
                mode,
            )
        }
    }

    /// Where requests go.
    #[must_use]
    pub fn destination(&self) -> Destination {
        self.destination
    }

    /// What a draft's record calls the model: the own endpoint's model name, or `allodia` for the
    /// relay, whose gateway picks its own.
    #[must_use]
    pub fn model_label(&self) -> &str {
        match (&self.model, self.destination) {
            (Some(model), _) => model,
            (None, Destination::AllodiaRelay) => "allodia",
            (None, Destination::OwnEndpoint { .. }) => "",
        }
    }

    /// Whether a request would pass the gate now, so a client can explain a refusal before the
    /// person asks for anything.
    ///
    /// # Errors
    ///
    /// Returns the [`Refused`] a request would meet.
    pub fn check(&self) -> Result<(), Refused> {
        admit(self.destination, &self.mode)
    }

    /// Asks the gate, then sends `request` if it passed.
    ///
    /// # Errors
    ///
    /// Returns [`AiError::Refused`] when the gate stopped it, in which case nothing left the
    /// device, and otherwise whatever the inner backend returned.
    pub fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AiError> {
        if let Err(refused) = self.check() {
            log::info!("ai: dispatch refused: {refused}");
            return Err(AiError::Refused(refused));
        }
        self.inner.chat(request)
    }
}

impl fmt::Debug for GatedBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatedBackend")
            .field("destination", &self.destination)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "gated_tests.rs"]
mod tests;
