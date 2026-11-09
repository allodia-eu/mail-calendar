//! What a boot dial produced, and how it is recorded.
//!
//! The bounded fan-out over the registered accounts, the failure classification a host reads
//! differently per channel, and the per-account record that seeds both the outage badge and the
//! reconnect queue.

use std::time::Instant;

use crate::{BoxedAccount, SharedRegistry, account_registry::dial_all};

/// Routes one dialed account into the app's account list, recording a mail failure.
///
/// The account is **always** added: a failed dial keeps its placeholder, so an outaged account
/// still lists with a badge instead of vanishing, which is the difference between "my server is
/// down" and "the app lost my account". A mail failure also records its `(id, detail)` in `failed`,
/// which seeds the outage badge and queues the account for reconnect.
///
/// Generic over the account so the classification is unit-testable without a live provider.
pub(crate) fn record_dial_outcome<A>(
    account: A,
    id: String,
    failure: Option<DialFailure>,
    accounts: &mut Vec<A>,
    account_errors: &mut Vec<String>,
    failed: &mut Vec<FailedDial>,
) {
    accounts.push(account);
    match failure {
        Some(DialFailure::MailFailed {
            detail,
            signin_rejected,
        }) => {
            account_errors.push(detail.clone());
            failed.push(FailedDial {
                id,
                detail,
                signin_rejected,
            });
        }
        None => {}
    }
}

/// An account whose boot dial failed: what to tell the user, and **which** of the two things to
/// tell them.
///
/// `signin_rejected` is the same verdict `reconnect_all` branches on, carried here so the two boot
/// modes cannot disagree about what a refusal means: one badging an outage while the other prompts
/// is a difference the user sees and neither mode can detect.
pub(crate) struct FailedDial {
    /// The account id, for the badge/prompt and the reconnect queue.
    pub(crate) id: String,
    /// The account-labelled cause, shown behind the outage badge's "details" link.
    pub(crate) detail: String,
    /// The server refused the account's credential: raise the reconnect prompt instead.
    pub(crate) signin_rejected: bool,
}

/// What went wrong on one account's boot dial. A calendar that did not connect beside working mail
/// is not a dial failure: it is a use problem, which the calendar's outage notice reports.
pub(crate) enum DialFailure {
    /// The **mail** connect failed: the account is kept as its placeholder, carrying this
    /// account-labelled detail, and re-dialed later.
    MailFailed {
        /// The account-labelled cause, for the badge's "details" link and the log.
        detail: String,
        /// Whether the server **refused the credential** rather than being unreachable; the
        /// reconnect prompt, not an outage badge (`docs/provider-oauth.md` rule 12).
        signin_rejected: bool,
    },
}

/// Dials every prepared account, at most [`MAX_CONCURRENT_DIALS`] at a time, and returns each
/// account (live on success, its original placeholder on failure) beside what went wrong.
///
/// Bounded rather than a plain `join_all`: an unbounded fan-out is what produced bursts of ten and
/// eleven simultaneous connections on a production device right after a network transition. See
/// [`MAX_CONCURRENT_DIALS`].
pub(super) async fn dial_registered(
    registry: &SharedRegistry,
    placeholders: Vec<BoxedAccount>,
    device_tz: &engine_api::TimeZoneId,
) -> Vec<(BoxedAccount, Option<DialFailure>)> {
    dial_all(placeholders, |index, placeholder| async move {
        let started = Instant::now();
        let id = placeholder.id.clone();
        // Registered a moment ago by `prepare_accounts`, so this is always `Some`, but it is
        // asked rather than assumed, because asking is what makes an unregistered account
        // undialable instead of merely undocumented.
        let Some(dial) = registry.dial(id.as_str()) else {
            log::error!("boot: account[{index}] vanished from the registry before its dial");
            return (placeholder, None);
        };
        let label = dial.label();
        let outcome = dial.run(&id, device_tz.clone()).await;
        let status = match &outcome {
            Ok(_) => "ok",
            // A refused credential is not an outage, and a support log that calls both
            // "unreachable" cannot tell them apart.
            Err(err) if err.signin_expired() => "sign-in REFUSED (kept as placeholder)",
            Err(_) => "unreachable (kept as placeholder)",
        };
        log::info!(
            "boot: account[{index}] connect {status} in {}ms",
            started.elapsed().as_millis(),
        );
        match outcome {
            Ok(outcome) => {
                registry.learn_dav_starts(id.as_str(), &outcome.learned);
                (outcome.account, None)
            }
            Err(err) => (
                placeholder,
                Some(DialFailure::MailFailed {
                    signin_rejected: err.signin_expired(),
                    detail: format!("{label}: {err}"),
                }),
            ),
        }
    })
    .await
}
