//! The recipient field's string work: which part of a To/Cc/Bcc field the user is typing, which
//! recipients are finished, and what the field becomes when one is accepted or removed.
//!
//! No GTK in sight, so it is driven directly by tests; the same shape the Windows client's
//! `RecipientTokens.cs`, Apple's `RecipientTokens.swift` and Android's `RecipientAutosuggest.kt`
//! take, because both of the failures it prevents are silent.
//!
//! The whole problem is that these fields hold a *list*: `"ada@example.test, gr"` is one string
//! whose last token is what the user is completing. Query the core with the whole field and it
//! matches nothing once a first recipient is entered; replace the whole field on selection and
//! every recipient already typed is destroyed. `docs/contacts.md` §4 binds every client to the
//! same split, which is why it lives here rather than inline in the widget.

/// What a finished recipient is followed by.
const SEPARATOR: &str = ", ";

/// The token the caret is in; the text after the last comma, trimmed.
///
/// This is the autosuggest query. Empty when the field is empty or ends at a separator, which the
/// core answers with no suggestions; so the list closes rather than offering everyone the moment
/// a recipient is completed.
pub(super) fn current_token(field: &str) -> &str {
    field
        .rsplit_once(',')
        .map_or(field, |(_, tail)| tail)
        .trim()
}

/// The recipients the user has **finished**; everything before the last comma.
///
/// These are what the field draws as pills. It is the same split autosuggest uses, so the pills
/// and the completion cannot disagree about where one recipient ends and the next begins. Empty
/// entries are dropped, so a stray `,,` or a trailing separator never becomes a blank pill.
pub(super) fn committed(field: &str) -> Vec<&str> {
    field
        .rsplit_once(',')
        .map_or_else(Vec::new, |(head, _)| entries(head))
}

/// Rebuilds the field from the finished recipients plus the token being typed.
///
/// The inverse of [`committed`] / [`current_token`], and the only place the string is assembled,
/// so removing a pill and accepting a suggestion cannot drift apart on spacing.
pub(super) fn field_text(committed: &[&str], token: &str) -> String {
    let mut text = String::new();
    for recipient in committed {
        text.push_str(recipient);
        text.push_str(SEPARATOR);
    }
    text.push_str(token);
    text
}

/// A field the composer was **opened with**: a reply's derived recipients, a `mailto:` link,
/// with every address in it marked finished.
///
/// The trailing-token rule infers what the user is *in the middle of typing*, and that inference
/// is simply wrong for a value nobody typed: everything the caller supplied is a complete address.
/// Seeded raw, a reply-all's `"bestuur@…, tc@…"` puts the last recipient in the input as half-typed
/// text beside one pill, and a single-address Cc gets no pill at all; the field looks like it lost
/// the recipients it is in fact holding. So the seed is normalised once, at the only moment it is
/// known that nothing is in progress.
///
/// Idempotent, and an empty field stays empty rather than becoming a lone separator; Send is gated
/// on a non-blank To.
pub(super) fn seeded(field: &str) -> String {
    field_text(&entries(field), "")
}

/// The field after accepting `email` for the token being typed.
///
/// Replaces **only** the last token and appends a separator, so the recipients already entered
/// survive and the next one can be typed without reaching for the comma key.
///
/// The address goes in **bare**, never as `Name <address>`: the core parses addresses, a display
/// name adds nothing it uses, and a name containing a comma would split into two invalid
/// recipients; which the user would not discover until the send failed.
pub(super) fn accept(field: &str, email: &str) -> String {
    let mut recipients = committed(field);
    recipients.push(email);
    field_text(&recipients, "")
}

/// The field with the pill at `index` removed, keeping the token being typed.
///
/// An out-of-range index returns the field unchanged rather than panicking: the pill row and a
/// click on it are a frame apart, and a rebuild in between must not take the composer down.
pub(super) fn remove(field: &str, index: usize) -> String {
    let mut recipients = committed(field);
    if index >= recipients.len() {
        return field.to_owned();
    }
    recipients.remove(index);
    field_text(&recipients, current_token(field))
}

/// Whether the field holds anything at all; what Send is gated on.
///
/// Asked through the same split rather than by trimming the raw string, so a field holding nothing
/// but separators reads as empty instead of as a recipient.
pub(crate) fn is_empty(field: &str) -> bool {
    committed(field).is_empty() && current_token(field).is_empty()
}

/// Whether a suggestion list is worth showing, given what the core returned.
///
/// Hidden once the token is exactly one of them: the user has finished that recipient, and a list
/// offering what is already typed covers the field below it for nothing.
pub(super) fn should_show_suggestions(field: &str, suggestions: &[String]) -> bool {
    let token = current_token(field);
    if token.is_empty() || suggestions.is_empty() {
        return false;
    }
    let token = token.to_lowercase();
    !suggestions
        .iter()
        .any(|suggestion| suggestion.to_lowercase() == token)
}

fn entries(field: &str) -> Vec<&str> {
    field
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "tokens_tests.rs"]
mod tests;
