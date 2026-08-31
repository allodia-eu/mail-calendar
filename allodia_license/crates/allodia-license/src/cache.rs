//! What a client draws between one answer and the next.
//!
//! The rules are [`entitlement.md`](../../../entitlement.md); this is where they are decided, so
//! that four clients cannot each decide them slightly differently. Nothing here reads a clock
//! (the host passes `now` as Unix seconds), because a rule about time that cannot be tested at an
//! arbitrary time is a rule nobody tests.

use serde::{Deserialize, Serialize};

use crate::{Answer, Entitlement};

/// How long a stored answer keeps granting while the service cannot be reached: **30 days**.
///
/// Long on purpose. Someone on a three-week trip with poor connectivity must not lose a capability
/// they are paying for, and being wrong in that direction costs almost nothing: a paid capability
/// is inert without Allodia's servers, so a stale *yes* grants the use of something that cannot
/// work anyway. Being wrong the other way takes a capability from someone who paid for it, during
/// an outage that is Allodia's fault.
pub const GRACE_SECONDS: i64 = 30 * 24 * 60 * 60;

/// What happened when a client last tried to ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The service replied. Whatever it said is the truth from now on, including "not entitled".
    Answered(Answer),
    /// The request did not arrive. Says nothing about the entitlement.
    Unreachable,
}

/// A stored answer and when it was taken, for the host to persist.
///
/// It goes in the app's preferences beside the store, **not** the platform keystore: it is derived
/// rather than secret, and a keystore read costs a prompt on some platforms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stored {
    /// The last answer the service gave.
    pub answer: Answer,
    /// When it gave it, in Unix seconds.
    pub fetched_at: i64,
}

/// The last answer, and the rules for acting on it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cache {
    stored: Option<Stored>,
}

impl Cache {
    /// Start from what the host had persisted, or from nothing.
    ///
    /// A stored value that would not parse is `None` here, which reads as the free app, never as
    /// an error, and never as a reason to block.
    #[must_use]
    pub fn restore(stored: Option<Stored>) -> Self {
        Self { stored }
    }

    /// What the host should persist. `None` means there is nothing worth keeping.
    #[must_use]
    pub fn stored(&self) -> Option<&Stored> {
        self.stored.as_ref()
    }

    /// What to draw right now.
    ///
    /// Anything that is not a live entitlement inside its grace is the free app: no stored answer,
    /// a stored answer past grace, or an answer that says `active: false`.
    #[must_use]
    pub fn effective(&self, now: i64) -> Entitlement {
        match &self.stored {
            Some(stored) if now.saturating_sub(stored.fetched_at) < GRACE_SECONDS => {
                stored.answer.entitlement.clone()
            }
            _ => Entitlement::free(),
        }
    }

    /// Whether it is time to ask again.
    ///
    /// True when there is nothing stored, and once the server's own interval has elapsed. A client
    /// asks in the background and never waits for the answer, so a `true` here is a hint to start
    /// a refresh, never a reason to hold a screen.
    #[must_use]
    pub fn should_refresh(&self, now: i64) -> bool {
        match &self.stored {
            None => true,
            Some(stored) => {
                now.saturating_sub(stored.fetched_at) >= stored.answer.refresh_after_seconds
            }
        }
    }

    /// Fold in what happened when the client last tried.
    ///
    /// **The distinction this method exists for.** A reply replaces the stored answer whatever it
    /// says, so a cancellation takes effect at once. A failure to reach the service changes
    /// nothing, so an outage does not take a capability away. Collapsing the two either honours a
    /// cancellation a month late or cuts off every paying customer during a bad afternoon, and
    /// neither failure produces anything a person would report.
    pub fn apply(&mut self, outcome: Outcome, now: i64) {
        match outcome {
            Outcome::Answered(answer) => {
                self.stored = Some(Stored {
                    answer,
                    fetched_at: now,
                });
            }
            Outcome::Unreachable => {}
        }
    }
}
