//! What a remembered refresh failure *is*, and how long it silences the account.
//!
//! Split out of `token_source.rs` to keep that file under the 500-line cap, and because it is one
//! idea: a refresh can fail in three ways with three different consequences, and every one of
//! them is a decision about whether re-presenting the same refresh token is safe. Keeping the
//! three cool-downs beside the enum that chooses between them means the reasoning for each is
//! readable against the other two, which is where it stops being arbitrary.

use mailcal_oauth::TokenRequestReach;
use time::{Duration, OffsetDateTime};

use crate::AccountError;

/// How long a refresh failure that **provably never left the device** suppresses further
/// attempts for this account.
///
/// Short on purpose. Re-presenting the refresh token after such a failure is not a replay,
/// so the only job here is to collapse the fan-out: every folder provider on the account
/// finds the token stale within the same few milliseconds, and without a shared outcome each
/// one posts its own request. Ten seconds is far longer than that burst and short enough
/// that a user who has just walked back into Wi-Fi does not watch a stale mailbox.
pub(super) const NOT_SENT_COOLDOWN: Duration = Duration::seconds(10);

/// How long a refresh failure that **may already have been processed** suppresses further
/// attempts for this account.
///
/// Longer, and for a different reason. Each attempt against a failing network is an
/// independent chance that *this* is the request the server processes and answers into a
/// void; spending the refresh token and losing its replacement. Retrying in a tight loop
/// does not improve the odds of success, it multiplies the number of rolls. So back off far
/// enough that a bad minute costs one attempt instead of hundreds.
///
/// It is deliberately shorter than [`super::REFRESH_SKEW`], so a transient blip still gets a second
/// and third try before the cached access token actually dies and the account goes
/// unreachable. A timer is a proxy for the question we actually want to ask; *is the network
/// usable yet*, and the answer arrives with the `blocked`-status work; this is the seam it
/// replaces.
pub(super) const MAYBE_PROCESSED_COOLDOWN: Duration = Duration::seconds(60);

/// How long a refresh the server answered with **`invalid_grant`** suppresses further attempts
/// for this account.
///
/// The longest of the three, because it is the one outcome that certainly will not change: the
/// stored refresh token is expired or revoked, and no amount of asking makes a dead grant live.
/// This arm deliberately bypassed the cool-down at first, on the reasoning that a terminal
/// failure needs no back-off, which had it backwards. A production log shows four refreshes in
/// under two minutes (08:39:56, 08:40:48, 08:41:23, 08:41:41), each presenting a token the
/// previous one had already been told was dead, and every folder provider on the account joins
/// that queue.
///
/// It costs the grant nothing (it is already revoked) but it hammers an authorization server
/// with credentials it has just rejected, which is precisely the traffic a server is entitled to
/// start rate-limiting or locking out, and it buries the *one* line support needs under repeats
/// of itself.
///
/// Memoizing cannot wedge the recovery, which is the only reason a long value is safe here.
/// Re-authenticating does not retry this source: it builds a **new** one and replaces the
/// registry entry (JMAP), or seeds the fresh access token in, and
/// [`super::GraphTokenSource::seed_access_token`] clears the remembered failure explicitly, for
/// exactly this reason.
pub(super) const DEAD_GRANT_COOLDOWN: Duration = Duration::minutes(30);

/// Why a refresh failed, in the distinction that decides how long the account holds off **and
/// which error every caller queued behind it is handed**.
///
/// The second half is why this is not just [`TokenRequestReach`]. A dead grant must keep
/// surfacing as [`AccountError::SigninRejected`] however it is served; from the server or from
/// this memo; because that variant is what raises the "sign in again" prompt (rule 12), and a
/// remembered failure that flattened into a transient error would make the prompt appear only for
/// whichever caller happened to lose the race to the server.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum FailureKind {
    /// The server answered, refusing the stored refresh token (`invalid_grant`). Terminal.
    DeadGrant,
    /// No usable answer arrived, classified by how far the request provably got.
    Unanswered(TokenRequestReach),
}

impl FailureKind {
    /// How long this failure suppresses further attempts; see the three cool-down constants.
    pub(super) const fn cooldown(self) -> Duration {
        match self {
            Self::DeadGrant => DEAD_GRANT_COOLDOWN,
            Self::Unanswered(TokenRequestReach::NotSent) => NOT_SENT_COOLDOWN,
            Self::Unanswered(TokenRequestReach::MaybeProcessed) => MAYBE_PROCESSED_COOLDOWN,
        }
    }

    /// How this failure reads in the line a caller emits when it takes the memo instead of
    /// posting its own request.
    pub(super) const fn describe(self) -> &'static str {
        match self {
            Self::DeadGrant => "with invalid_grant: the sign-in is dead",
            Self::Unanswered(TokenRequestReach::NotSent) => "before leaving this device",
            Self::Unanswered(TokenRequestReach::MaybeProcessed) => {
                "after possibly reaching the server"
            }
        }
    }

    /// The error to hand a caller that took this memo, preserving the one distinction downstream
    /// acts on: a dead sign-in needs a person, a transient failure needs a retry.
    pub(super) fn error(self, message: &str) -> AccountError {
        match self {
            Self::DeadGrant => AccountError::SigninRejected(message.to_owned()),
            Self::Unanswered(_) => AccountError::Graph(format!("token refresh: {message}")),
        }
    }
}

/// A remembered refresh failure: when it happened, why, and what to tell the callers that arrive
/// while it still stands.
pub(super) struct RefreshFailure {
    pub(super) at: OffsetDateTime,
    pub(super) kind: FailureKind,
    pub(super) message: String,
}
