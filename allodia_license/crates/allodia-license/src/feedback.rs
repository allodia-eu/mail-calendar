//! Feedback on drafted replies, posted to the Mail & Calendar service (`docs/ai.md`, "Feedback"
//! and "Where requests go").
//!
//! The body is the document the app kept in its outbox, sent as it is. The jurisdiction gate is
//! asked before anything leaves, with the relay's destination, because the route is on the relay's
//! service. A 2xx answer is delivery; anything else, or no answer, leaves the item for a later
//! pass.

use std::{fmt, time::Duration};

use mailcal_ai::{Destination, HttpRequest, HttpTransport, ModeSource, Refused, admit};

use crate::{API_BASE_PATH, AccountService};

/// Whether the service has `POST /api/v1/ai/feedback`. **False until the account service ships
/// that route**: a request to a route that does not exist still carries its body, so until then
/// nothing leaves the outbox and no token is minted for it. Turn it on in the change that follows
/// the route's deployment.
pub const AI_FEEDBACK_ROUTE_LIVE: bool = false;

/// How long one post may take.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Why an item was not delivered. It stays in the outbox either way.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FeedbackError {
    /// The jurisdiction gate stopped it; nothing left the device.
    #[error("{0}")]
    Refused(Refused),
    /// No access token could be had.
    #[error("no access token")]
    Unauthorized,
    /// No answer arrived.
    #[error("the service could not be reached")]
    Unreachable,
    /// The service answered with a status other than 2xx.
    #[error("the service answered {0}")]
    Status(u16),
}

/// Posts feedback to the service.
pub struct FeedbackSender {
    url: String,
    transport: Box<dyn HttpTransport>,
    mode: ModeSource,
}

impl FeedbackSender {
    /// The feedback route of the service at `service`, over `transport`, gated under `mode`.
    #[must_use]
    pub fn new(
        service: &AccountService,
        transport: Box<dyn HttpTransport>,
        mode: ModeSource,
    ) -> Self {
        Self {
            url: format!("{}{API_BASE_PATH}/ai/feedback", service.base_url()),
            transport,
            mode,
        }
    }

    /// Posts one item's `body`, after asking the gate.
    ///
    /// # Errors
    ///
    /// Returns the [`FeedbackError`] that kept it from being delivered.
    pub fn send(&self, access_token: &str, body: &str) -> Result<(), FeedbackError> {
        admit(Destination::AllodiaRelay, &self.mode).map_err(FeedbackError::Refused)?;
        let answered = self
            .transport
            .post_json(HttpRequest {
                url: &self.url,
                bearer: Some(access_token),
                body,
                timeout: TIMEOUT,
            })
            .map_err(|_| FeedbackError::Unreachable)?;
        match answered.status {
            200..=299 => Ok(()),
            status => Err(FeedbackError::Status(status)),
        }
    }

    /// Sends `items`, each an id and a body, oldest first. Nothing is sent, and `token` is not
    /// asked, while [`AI_FEEDBACK_ROUTE_LIVE`] is false.
    pub fn deliver<'a>(
        &self,
        token: impl FnOnce() -> Option<String>,
        items: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Delivery {
        if !AI_FEEDBACK_ROUTE_LIVE {
            return Delivery::default();
        }
        self.deliver_now(token, items)
    }

    /// [`deliver`](Self::deliver) without the switch. Stops at the first item not delivered: the
    /// ones after it would meet the same answer, and each stays for a later pass.
    fn deliver_now<'a>(
        &self,
        token: impl FnOnce() -> Option<String>,
        items: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Delivery {
        let mut items = items.into_iter().peekable();
        let mut delivery = Delivery::default();
        if items.peek().is_none() {
            return delivery;
        }
        let Some(token) = token() else {
            delivery.stopped = Some(FeedbackError::Unauthorized);
            return delivery;
        };
        for (id, body) in items {
            if let Err(error) = self.send(&token, body) {
                delivery.stopped = Some(error);
                break;
            }
            delivery.delivered.push(id.to_owned());
        }
        delivery
    }
}

/// What a pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Delivery {
    /// The ids the service took, which leave the outbox.
    pub delivered: Vec<String>,
    /// Why the pass stopped early, when it did.
    pub stopped: Option<FeedbackError>,
}

impl fmt::Debug for FeedbackSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeedbackSender").finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
