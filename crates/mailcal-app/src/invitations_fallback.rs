//! What happens when the calendar server **said it would tell the organiser and then could
//! not**: the check that turns a silent failure into a decision the user gets to make.
//!
//! [`Delivery::Server`](crate::invitations::Delivery::Server) is a promise: the client writes a
//! `PARTSTAT`, the server mails the organiser, and we deliberately send nothing ourselves
//! because two replies are worse than one. This module is what happens when the promise is
//! broken, which a conforming server tells us, in the event itself, and which nothing in this
//! product used to read.
//!
//! # Why this is a check and not a capability
//!
//! It would be tidier to decide the route up front from capabilities, the way
//! [`delivery`](crate::invitations::delivery) does. It cannot be done: `calendar-auto-schedule`
//! is advertised by a server that never delivers a single reply, and there is no token for
//! *"…and it works"*. The only honest source is what the server reports **after** the write,
//! per RFC 6638 §3.2.9: so this is trust, then verify, then offer the fallback.
//!
//! # Why the user is asked rather than told
//!
//! Sending the reply ourselves is sending **mail as the user**, to someone they did not choose
//! in this moment. That is not a repair the app gets to make silently, so the default is
//! [`ReplyFallback::Ask`]. It is also not something to ask twice on a server that fails every
//! time; hence the remembered per-account choice (`docs/invitations.md`).
//!
//! **Privacy.** The log lines here carry a status code and an outcome. The organiser's address
//! and the meeting's title reach the *prompt*, which is UI, and never the log
//! (`docs/logging.md`).

use engine_api::{AccountId, Provider, ReplyDelivery};
use mailcal_account::ReplyFallback;

use crate::{
    App, invitations_imip::ClientAnswer, invitations_rsvp::InvitationResponse,
    reference::MessageRef,
};

/// What became of the reply the calendar server was supposed to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplyOutcome {
    /// The server sent it, or never said otherwise. Nothing to do, and nothing to tell the
    /// user: this is the overwhelmingly common path.
    ServerHandledIt,
    /// The server could not, and this account's standing choice is to send it ourselves: so we
    /// did.
    SentOurselves,
    /// The server could not, and this account's standing choice is that we do not send replies
    /// for it. The user is still told the organiser was not informed; the choice was about
    /// sending mail, not about hiding the outcome.
    NotSentByChoice,
    /// The server could not, and nobody has decided what this account should do. Carries the
    /// question to put to the user.
    Ask(ReplyPrompt),
    /// The server could not, we tried to send it ourselves, and that failed too. Carries the
    /// reason, which the user needs; at this point *neither* route worked.
    CouldNotSend(String),
}

/// The question a client asks when a calendar server could not deliver an answer.
///
/// Carries what the modal needs to be specific: a person answering "yes" should see *which*
/// meeting and *who* will be emailed; plus the handle to act on. Deliberately not the whole
/// answer: the send re-derives everything from the message, so a prompt that outlives a
/// restart cannot act on stale state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyPrompt {
    /// The account whose server failed, and whose standing choice a "remember" would set.
    pub account: AccountId,
    /// The invitation being answered, so the send can re-derive the reply from it.
    pub message: MessageRef,
    /// The meeting's title, for the sentence naming what is being confirmed.
    pub summary: String,
    /// The organiser's address; who the email would go to. A user consenting to send mail on
    /// their own behalf is entitled to see the recipient.
    pub organizer: String,
    /// The answer that was given, so the reply we send says the same thing the calendar does.
    /// Re-deriving it from the stored `PARTSTAT` would be a second source of truth for one
    /// fact, and the two could disagree if anything touched the event in between.
    pub response: InvitationResponse,
    /// The note the user wrote for the organiser, if any; it becomes the reply's `COMMENT`.
    pub comment: Option<String>,
    /// The RFC 6638 status the server reported (`5.2`, `3.7`, …). Not shown in the plain-language
    /// prompt; carried for the diagnostics screen and the log.
    pub status_code: String,
}

impl<P: Provider> App<P> {
    /// Acts on what the server reported about the reply, which the write receipt already
    /// carries.
    ///
    /// Called only on the [`Server`](crate::invitations::Delivery::Server) route, and only after
    /// the RSVP write has landed: the verdict does not exist before the server has tried.
    ///
    /// Every path logs, because this is the code a support conversation about an unfamiliar
    /// server starts from: a `NotReported` line is what distinguishes "the server is silent" from
    /// "we never asked", and the two look identical in a log that only speaks up on failure.
    pub(crate) async fn resolve_reply_delivery(
        &self,
        answer: &ClientAnswer<'_>,
        delivery: &ReplyDelivery,
    ) -> ReplyOutcome {
        // A debug build can be told to pretend the server said something else, because no
        // harness can produce a reported failure; see [`pretended_delivery`]. Always `None` in
        // a release build, where the whole function folds away.
        let pretended = pretended_delivery();
        let delivery = pretended.as_ref().unwrap_or(delivery);
        let code = match delivery {
            ReplyDelivery::Failed { status } => status.clone(),
            ReplyDelivery::Delivered { status } => {
                log::info!(
                    "respond_to_invitation: the calendar server delivered the reply (status \
                     {status})"
                );
                return ReplyOutcome::ServerHandledIt;
            }
            // A class RFC 5546 §3.6 does not define. Not actionable; guessing "failed" would
            // email an organiser who may already have the reply, but the token is exactly what
            // someone debugging this server needs, so it is logged at a level that is on by
            // default rather than swallowed.
            ReplyDelivery::Unrecognized { status } => {
                log::info!(
                    "respond_to_invitation: the calendar server reported an unrecognized \
                     delivery status ({status}); treating it as no report"
                );
                return ReplyOutcome::ServerHandledIt;
            }
            ReplyDelivery::NotReported => {
                log::info!(
                    "respond_to_invitation: the calendar server reported nothing about \
                     delivering the reply, assuming it was sent"
                );
                return ReplyOutcome::ServerHandledIt;
            }
        };
        log::info!(
            "respond_to_invitation: the calendar server could not deliver the reply (status {code})"
        );

        match self.reply_fallback_for(&answer.message.account) {
            ReplyFallback::Never => {
                log::info!(
                    "respond_to_invitation: not emailing the organizer; the account says never"
                );
                ReplyOutcome::NotSentByChoice
            }
            ReplyFallback::Always => {
                log::info!(
                    "respond_to_invitation: emailing the reply ourselves; the account says always"
                );
                match self.send_imip_reply(answer, answer.stored.as_ref()).await {
                    Ok(()) => ReplyOutcome::SentOurselves,
                    Err(reason) => {
                        log::warn!(
                            "respond_to_invitation: the reply could not be emailed either: \
                             {reason}"
                        );
                        ReplyOutcome::CouldNotSend(reason)
                    }
                }
            }
            ReplyFallback::Ask => {
                // No organiser means no address to send to, so there is no question to ask;
                // offering to email nobody would be a control that cannot work.
                let Some(organizer) = answer.scheduling.message.organizer() else {
                    log::info!(
                        "respond_to_invitation: not asking; the invitation names no organizer to \
                         email"
                    );
                    return ReplyOutcome::NotSentByChoice;
                };
                log::info!("respond_to_invitation: asking whether to email the organizer");
                ReplyOutcome::Ask(ReplyPrompt {
                    account: answer.message.account.clone(),
                    message: answer.message.clone(),
                    summary: answer.scheduling.message.event.title.clone(),
                    organizer: strip_mailto(organizer).to_owned(),
                    response: answer.response,
                    comment: answer.comment.map(str::to_owned),
                    status_code: code,
                })
            }
        }
    }

    /// This account's standing answer to the prompt. [`ReplyFallback::Ask`] where there is no
    /// preferences file at all (the demo and the tests), which is the default everywhere else
    /// too.
    pub(crate) fn reply_fallback_for(&self, account: &AccountId) -> ReplyFallback {
        self.prefs_path.as_ref().map_or(ReplyFallback::Ask, |path| {
            mailcal_account::load_preferences(path).reply_fallback(account.as_str())
        })
    }

    /// Turns the outcome into what the caller of an RSVP returns, and raises the prompt when
    /// there is one.
    ///
    /// Only [`ReplyOutcome::CouldNotSend`] is an error, and the distinction is the point: the
    /// answer **is** stored in every other case, so failing the whole action would tell the user
    /// their RSVP did not happen when it did, and invite them to press the button again.
    pub(crate) fn apply_reply_outcome(&self, outcome: ReplyOutcome) -> Result<(), String> {
        match outcome {
            ReplyOutcome::ServerHandledIt
            | ReplyOutcome::SentOurselves
            | ReplyOutcome::NotSentByChoice => Ok(()),
            ReplyOutcome::Ask(prompt) => {
                self.set_reply_prompt(Some(prompt));
                Ok(())
            }
            ReplyOutcome::CouldNotSend(reason) => Err(reason),
        }
    }

    /// The unanswered question, if a server has just failed to deliver a reply.
    #[must_use]
    pub fn reply_prompt(&self) -> Option<ReplyPrompt> {
        self.reply_prompt
            .lock()
            .expect("reply-prompt mutex poisoned")
            .clone()
    }

    /// Raises or clears the prompt, signalling the surface either way: a modal that cannot be
    /// dismissed by the core is one that outlives the thing it asks about.
    pub(crate) fn set_reply_prompt(&self, prompt: Option<ReplyPrompt>) {
        *self
            .reply_prompt
            .lock()
            .expect("reply-prompt mutex poisoned") = prompt;
        self.observer
            .surface_changed(crate::Surface::InvitationReply);
    }

    /// Acts on the user's answer to the prompt: remember the choice, and send the reply if they
    /// said to.
    ///
    /// **Takes the prompt**, so the question is gone before any await. Two taps on a modal that
    /// has not closed yet, or a client that dispatches on both press and release, would
    /// otherwise send the organiser two replies, and the second would arrive with no way for
    /// them to tell it apart from a change of mind.
    ///
    /// # Errors
    ///
    /// Returns the reason the reply could not be sent. Dismissing never fails: nothing was
    /// asked of the network, and the answer itself was stored long before the question arose.
    pub(crate) async fn answer_reply_prompt(
        &self,
        send: bool,
        remember: bool,
        reply_subject: Option<String>,
    ) -> Result<(), String> {
        let Some(prompt) = self.take_reply_prompt() else {
            log::info!("answer_reply_prompt: there is no pending question; ignoring");
            return Ok(());
        };
        if remember {
            self.remember_reply_fallback(
                &prompt.account,
                if send {
                    ReplyFallback::Always
                } else {
                    ReplyFallback::Never
                },
            );
        }
        if !send {
            log::info!("answer_reply_prompt: the organizer will not be emailed");
            return Ok(());
        }
        self.send_reply_for(&prompt, reply_subject).await
    }

    /// Rebuilds the answer from the message and sends the iTIP `REPLY` as mail.
    ///
    /// Everything is re-derived rather than carried on the prompt: the invitation's bytes, the
    /// matched alias, the meeting's instant: for the same reason the prompt is not persisted:
    /// the only state worth trusting here is what the store holds now.
    async fn send_reply_for(
        &self,
        prompt: &ReplyPrompt,
        reply_subject: Option<String>,
    ) -> Result<(), String> {
        let original = self
            .find_message_in(&prompt.message)
            .await
            .ok_or_else(|| "the message is no longer in the store".to_owned())?;
        let scheduling = self
            .fetch_scheduling(&prompt.message, &original)
            .await
            .ok_or_else(|| "the message carries no invitation to answer".to_owned())?;
        let event = &scheduling.message.event;

        let mut addresses = self.account_address_set(&prompt.account).await;
        addresses.extend(scheduling.delivery_recipients.iter().cloned());
        let attendee = crate::invitations::matched_attendee(event, &addresses)
            .ok_or_else(|| "this invitation is not addressed to this account".to_owned())?;

        let zone = self.active_zone();
        let starts_at = engine_api::resolve_instant_in(&event.start, &zone)
            .map_err(|err| format!("the invitation's start cannot be placed: {err}"))?;
        let ends_at = crate::invitations_build::end_of(starts_at, event);
        let stored = self
            .stored_event_by_uid(&prompt.account, event.uid.as_str(), starts_at, ends_at)
            .await;

        self.send_imip_reply(
            &ClientAnswer {
                message: &prompt.message,
                original: &original,
                scheduling: &scheduling,
                attendee: &attendee,
                response: prompt.response,
                comment: prompt.comment.as_deref(),
                notify_organizer: true,
                reply_subject: reply_subject.as_deref(),
                starts_at,
                ends_at,
                stored: stored.clone(),
            },
            stored.as_ref(),
        )
        .await
    }

    /// Removes and returns the pending question, signalling the surface so the modal closes.
    fn take_reply_prompt(&self) -> Option<ReplyPrompt> {
        let taken = self
            .reply_prompt
            .lock()
            .expect("reply-prompt mutex poisoned")
            .take();
        if taken.is_some() {
            self.observer
                .surface_changed(crate::Surface::InvitationReply);
        }
        taken
    }

    /// Records what the user decided, so the same server does not ask again on every meeting.
    pub(crate) fn remember_reply_fallback(&self, account: &AccountId, choice: ReplyFallback) {
        let Some(path) = &self.prefs_path else {
            return;
        };
        let mut prefs = mailcal_account::load_preferences(path);
        prefs.set_reply_fallback(account.as_str(), choice);
        let _ = mailcal_account::save_preferences(path, &prefs);
    }
}

/// The verdict a debug build has been told to pretend the calendar server reported, if any.
///
/// # Why this exists
///
/// The state that matters here (a **reported failure**) is the one no test fixture can
/// produce. Every local harness runs Stalwart, which delivers replies and reports nothing at
/// all, and the one server known to report `5.2` is somebody's production account. So the
/// failure path could be reached only by editing this file by hand, which is how it was
/// verified on every platform so far, and the client half of it (the prompt, its copy, the
/// tick's initial state, what a press dispatches) has no unit test that can see it at all;
/// `Mailcal.Tests` cannot link a WinUI type, and `cargo test` cannot see a XAML binding.
///
/// `MAILCAL_FAKE_REPLY_DELIVERY` substitutes **only** the server's verdict. Everything after it
/// is real: the core raises the question, signals `Surface::InvitationReply`, takes it on an
/// answer, remembers the choice and sends the mail. That is the smallest lie that makes the
/// path reachable: a hook that faked the *prompt* would be a mock of the thing under test, and
/// would go on passing after the wiring between core and client was cut.
///
/// The value names a **variant**, never a status token to be classified. Which class a code
/// belongs to is protocol knowledge, and it lives in the engine; whose own `ReplyDelivery`
/// docs say no caller should branch on the text (`AGENTS.md` → "Protocol knowledge belongs in
/// the engine"). Accepted forms, anything else ignored with a warning:
///
/// ```text
/// failed:5.2     delivered:2.0     unrecognized:9.9     notreported
/// ```
///
/// Debug builds only, like the harness CA trust it sits beside, so no release binary can be
/// talked into asking about a reply the server handled perfectly well. (The Android dev loop
/// builds the core `--release`, so reaching this from there would need a `dev-harness` feature
/// forwarded down from `mailcal-bindings`, nothing needs that yet.)
#[cfg(debug_assertions)]
fn pretended_delivery() -> Option<ReplyDelivery> {
    let raw = std::env::var("MAILCAL_FAKE_REPLY_DELIVERY").ok()?;
    let pretended = parse_pretended_delivery(&raw);
    // Warned rather than debugged, on both paths: this rewrites what the user is told about
    // their own mail, so a log that records the run must say it was in force, and a value that
    // silently did nothing is exactly how a test comes to prove the opposite of what it claims.
    match &pretended {
        Some(delivery) => log::warn!(
            "respond_to_invitation: MAILCAL_FAKE_REPLY_DELIVERY is set; pretending the calendar \
             server reported {delivery:?}"
        ),
        None => log::warn!(
            "respond_to_invitation: MAILCAL_FAKE_REPLY_DELIVERY={raw} names no verdict I \
             recognize (failed:<status> | delivered:<status> | unrecognized:<status> | \
             notreported); using what the server actually reported"
        ),
    }
    pretended
}

#[cfg(not(debug_assertions))]
fn pretended_delivery() -> Option<ReplyDelivery> {
    None
}

/// Reads [`pretended_delivery`]'s value. Split out from the environment lookup so the parsing is
/// a check that can fail without a test having to write a process-global variable.
#[cfg(debug_assertions)]
pub(crate) fn parse_pretended_delivery(raw: &str) -> Option<ReplyDelivery> {
    let trimmed = raw.trim();
    let (variant, status) = trimmed.split_once(':').unwrap_or((trimmed, ""));
    match (variant, status) {
        ("notreported", "") => Some(ReplyDelivery::NotReported),
        // A status is required for the three variants that carry one: an empty token would
        // reach a log line, and a prompt, as nothing at all.
        ("failed", status) if !status.is_empty() => Some(ReplyDelivery::Failed {
            status: status.to_owned(),
        }),
        ("delivered", status) if !status.is_empty() => Some(ReplyDelivery::Delivered {
            status: status.to_owned(),
        }),
        ("unrecognized", status) if !status.is_empty() => Some(ReplyDelivery::Unrecognized {
            status: status.to_owned(),
        }),
        _ => None,
    }
}

/// Drops the `mailto:` scheme from a calendar address, for display.
fn strip_mailto(address: &str) -> &str {
    address
        .strip_prefix("mailto:")
        .or_else(|| address.strip_prefix("MAILTO:"))
        .unwrap_or(address)
}
