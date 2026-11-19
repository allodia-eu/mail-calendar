//! Where a request goes (`docs/ai.md` rule 2): an own endpoint when one is set up, otherwise the
//! relay, otherwise nowhere. What a page says about it before anything is pressed and what a
//! request is sent through come from the same function, so the two cannot disagree.

use engine_api::Provider;
use mailcal_ai::{Class, Destination, GatedBackend, OwnEndpoint, admit};

use super::{AiFailure, AiRun, RelayStatus, ai_offered};
use crate::App;

/// Where a request would go now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiRoute {
    /// The person's own endpoint.
    OwnEndpoint {
        /// The host requests go to, which the consent names.
        host: String,
        /// Where the person declared it runs.
        declared: Option<Class>,
    },
    /// Allodia's relay.
    Relay,
}

impl AiRoute {
    /// The destination the gate classes.
    #[must_use]
    pub const fn destination(&self) -> Destination {
        match self {
            Self::OwnEndpoint { declared, .. } => Destination::OwnEndpoint {
                declared: *declared,
            },
            Self::Relay => Destination::AllodiaRelay,
        }
    }
}

impl<P: Provider> App<P> {
    /// The own endpoint as stored, with its key, when one is set up and still makes an endpoint.
    fn own_endpoint(&self) -> Option<OwnEndpoint> {
        let stored = self.ai.settings().endpoint.clone()?;
        let key = self.ai.ports().key.clone();
        OwnEndpoint::new(&stored.base_url, key, &stored.model, stored.declared).ok()
    }

    /// Where a request would go now, or why none can: what Learn and Draft a reply say before
    /// they are pressed, and what [`App::ai_backend`] acts on. Read from memory.
    ///
    /// # Errors
    ///
    /// [`AiFailure::NotEntitled`] while the feature is not offered, [`AiFailure::Refused`] when
    /// the gate refuses the own endpoint as declared, [`AiFailure::RelaySignIn`] when the relay's
    /// sign-in has to be made again, and [`AiFailure::NowhereToSend`] otherwise.
    pub fn ai_route(&self) -> Result<AiRoute, AiFailure> {
        let (offered, relay) = {
            let ports = self.ai.ports();
            (
                ai_offered(ports.build, ports.entitlement),
                ports.relay.clone(),
            )
        };
        if !offered {
            return Err(AiFailure::NotEntitled);
        }
        let relay = relay.map(|relay| relay.status());
        let route = if let Some(endpoint) = self.own_endpoint() {
            AiRoute::OwnEndpoint {
                host: endpoint.host(),
                declared: endpoint.declared(),
            }
        } else {
            match relay {
                Some(RelayStatus::Ready) => AiRoute::Relay,
                Some(RelayStatus::SignInNeeded) => return Err(AiFailure::RelaySignIn),
                Some(RelayStatus::SignedOut) | None => return Err(AiFailure::NowhereToSend),
            }
        };
        admit(route.destination(), &self.ai_mode_source()).map_err(AiFailure::Refused)?;
        Ok(route)
    }

    /// The backend a request of `run` is sent through, behind the jurisdiction gate with the
    /// destination it truly sends to and the mode read at each request. The one way this crate
    /// builds a backend.
    ///
    /// # Errors
    ///
    /// As [`App::ai_route`].
    pub fn ai_backend(&self, run: &AiRun<'_>) -> Result<GatedBackend, AiFailure> {
        let route = self.ai_route()?;
        let transport = run.transport();
        let mode = self.ai_mode_source();
        match route {
            AiRoute::OwnEndpoint { .. } => {
                let endpoint = self.own_endpoint().ok_or(AiFailure::NowhereToSend)?;
                Ok(GatedBackend::own_endpoint(endpoint, transport, mode))
            }
            AiRoute::Relay => {
                let relay = self
                    .ai
                    .ports()
                    .relay
                    .clone()
                    .ok_or(AiFailure::NowhereToSend)?;
                Ok(GatedBackend::new(
                    relay.backend(transport),
                    Destination::AllodiaRelay,
                    mode,
                ))
            }
        }
    }
}
