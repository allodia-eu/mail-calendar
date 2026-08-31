//! Composer recipient autosuggest: ranked addresses for a partially-typed To/Cc/Bcc token.
//!
//! # Why this works before contacts do
//!
//! The engine derives recipient history from messages in each account's **Sent** mailbox, and
//! it does that inside the ordinary mail sync (plus a one-time backfill of already-stored
//! mail). So suggestions are useful on an account with **no address book at all**, which is
//! most accounts, and a user who has never opened Contacts still gets completion for
//! everyone they have written to. Synced contacts then rank alongside that history rather
//! than replacing it.
//!
//! That is also why this lives in its own module rather than inside `contacts.rs`: the two
//! features share the engine's people index but neither depends on the other being enabled.

use std::time::Instant;

use engine_api::Provider;

use crate::App;

/// How many suggestions one query returns. A composer dropdown that needs scrolling has
/// stopped being a shortcut; the engine ranks, so the best few are the ones worth showing.
const SUGGESTION_LIMIT: usize = 8;

/// One ranked recipient suggestion for the composer.
///
/// Deliberately **not** named `RecipientSuggestion`: [`crate::protocol::RecipientSuggestion`]
/// already exists and means something entirely different (the To/Cc a reply is pre-filled
/// with). Two types with one name, one of them plural-ish and both about recipients, is a
/// confusion that would be paid for repeatedly at every call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipientMatch {
    /// The canonical email address to insert.
    pub email: String,
    /// The display name, empty when only the address is known.
    pub display_name: String,
    /// Whether a saved personal contact supplies this address, as opposed to it being known
    /// only from mail the user has sent. A host may mark the two apart; it must not hide the
    /// history-only ones, which are usually the most useful.
    pub is_saved: bool,
}

impl<P: Provider> App<P> {
    /// Ranked recipient suggestions for a partially-typed address.
    ///
    /// Returns nothing for a blank query: a dropdown of "everyone you have ever emailed"
    /// the moment the field takes focus is noise, not help. Network-free: it reads the
    /// already-derived people index and recipient history rather than syncing. It is not,
    /// however, *cheap*: the engine answers it from the store, so a host debounces and calls
    /// it off whatever thread draws the composer.
    pub async fn recipient_suggestions(&self, query: &str) -> Vec<RecipientMatch> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        // The token's LENGTH, never the token: a partially-typed recipient is a name or an
        // address, which is content and never logged (`docs/logging.md`). The length is still
        // what a debounce bug looks like; one query per character prints an ascending run of
        // lines a keystroke apart, which is visible at a glance and invisible without this.
        let query_chars = query.chars().count();
        let started = Instant::now();
        match self
            .engine
            .recipient_suggestions(query, SUGGESTION_LIMIT)
            .await
        {
            Ok(found) => {
                let matches: Vec<RecipientMatch> = found
                    .suggestions
                    .into_iter()
                    .map(|suggestion| RecipientMatch {
                        email: suggestion.email.as_str().to_owned(),
                        display_name: suggestion.display_name,
                        is_saved: suggestion.is_saved,
                    })
                    .collect();
                // `saved` splits the two sources this ranks over. Zero saved with a non-empty
                // result means contacts contributed nothing while sent-mail history carried the
                // dropdown, which is the expected state on an account with no address book,
                // and a bug on one that has just synced a full one.
                log::debug!(
                    "recipient_suggestions: {} match(es) ({} saved) for a {query_chars}-char \
                     token in {}ms",
                    matches.len(),
                    matches.iter().filter(|found| found.is_saved).count(),
                    started.elapsed().as_millis(),
                );
                matches
            }
            Err(error) => {
                // A failed lookup yields no suggestions rather than an error state: the user
                // is mid-keystroke in a text field, and the fallback; typing the address
                // themselves, is the thing they were already doing.
                log::warn!(
                    "recipient_suggestions: failed in {}ms for a {query_chars}-char token: \
                     {error}",
                    started.elapsed().as_millis(),
                );
                Vec::new()
            }
        }
    }
}
