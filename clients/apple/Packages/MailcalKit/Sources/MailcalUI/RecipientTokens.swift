// The composer's recipient-field string work: which part of a To/Cc/Bcc field the user is currently
// typing, which recipients are finished, and what the field becomes when one is accepted or removed.
//
// Plain functions with no SwiftUI in sight, so the test suite drives them directly (the same shape
// `CalendarZoom` and Android's `RecipientAutosuggest.kt` take). The ranking itself lives in the Rust
// core; everything here is string handling.
//
// The whole problem is that these fields hold a *list*: "ada@example.test, gr" is one string whose
// last token is what the user is completing. Query the core with the whole field and it matches
// nothing once a first recipient is entered; replace the whole field on selection and every
// recipient already typed is silently destroyed. Both failures are silent, which is why
// `docs/contacts.md` §4 binds every client to the same rule and why this is factored out and tested
// rather than inlined into a view.

import Foundation

/// The separator between recipients in a To/Cc/Bcc field, and what a completed entry ends with.
private let recipientSeparator = ", "

/// The token the caret is in, the text after the last comma, trimmed.
///
/// This is what goes to the core as the autosuggest query. Empty when the field is empty or ends at
/// a separator, which the core answers with no suggestions, so the list closes rather than offering
/// everyone the moment a recipient is completed.
func currentRecipientToken(_ field: String) -> String {
    let tail = field.lastIndex(of: ",").map { field[field.index(after: $0)...] } ?? field[...]
    return tail.trimmingCharacters(in: .whitespaces)
}

/// The recipients in `field` the user has **finished**, everything before the last comma.
///
/// These are what the UI draws as pills. The split is the same one autosuggest uses (the trailing
/// token is what is being typed; everything before it is settled), so the pills and the completion
/// cannot disagree about where one recipient ends and the next begins.
///
/// Empty entries are dropped, so a stray `,,` or a trailing separator never becomes a blank pill.
func committedRecipients(_ field: String) -> [String] {
    guard let lastComma = field.lastIndex(of: ",") else { return [] }
    return field[..<lastComma]
        .split(separator: ",", omittingEmptySubsequences: false)
        .map { $0.trimmingCharacters(in: .whitespaces) }
        .filter { !$0.isEmpty }
}

/// Rebuilds the field's text from the finished `committed` recipients plus the `token` being typed.
///
/// The inverse of `committedRecipients` / `currentRecipientToken`, and the only place the field's
/// string is assembled, so removing a pill and accepting a suggestion cannot drift apart on
/// spacing.
func recipientFieldText(_ committed: [String], _ token: String) -> String {
    committed.map { $0 + recipientSeparator }.joined() + token
}

/// A field the composer was **opened with**, a reply's derived recipients, an assistant's draft:
/// with every address in it marked finished.
///
/// The trailing-token rule above infers what the user is *in the middle of typing*, and that
/// inference is simply wrong for a value nobody typed: everything the caller supplied is a complete
/// address. Seeded raw, a reply-all's `"bestuur@…, tc@…"` puts the last recipient in the input as
/// half-typed text beside one pill, and a single-address Cc gets no pill at all, the field looks
/// like it lost the recipients it is in fact holding. So the seed is normalised once, here, at the
/// only moment it is known that nothing is in progress.
///
/// Idempotent, so a caller that already ends at a separator is unchanged, and an empty field stays
/// empty rather than becoming a lone separator (the composer gates Send on a non-blank To).
func seededRecipientField(_ field: String) -> String {
    let all = field
        .split(separator: ",")
        .map { $0.trimmingCharacters(in: .whitespaces) }
        .filter { !$0.isEmpty }
    return recipientFieldText(all, "")
}

/// The field after accepting `email` for the token currently being typed.
///
/// Replaces **only** the last token and appends a separator, so the recipients already entered
/// survive and the user can type the next one without reaching for the comma key.
///
/// The address is inserted bare rather than as `Name <address>`: the core's submit path parses
/// addresses, a display name adds nothing it uses, and a name containing a comma would split into
/// two invalid recipients, a corruption the user would not see until the send failed.
func acceptRecipientSuggestion(_ field: String, _ email: String) -> String {
    recipientFieldText(committedRecipients(field) + [email], "")
}

/// The field with the pill at `index` removed, keeping the token currently being typed.
///
/// An out-of-range index returns the field unchanged rather than trapping: the pill list and a tap
/// on it are a frame apart, and a re-render in between must not crash the composer.
func removeRecipient(_ field: String, at index: Int) -> String {
    var committed = committedRecipients(field)
    guard committed.indices.contains(index) else { return field }
    committed.remove(at: index)
    return recipientFieldText(committed, currentRecipientToken(field))
}

/// Whether a suggestion list is worth showing for `field`, given the addresses the core returned.
///
/// Hidden once the current token is exactly one of them: the user has finished that recipient, and
/// a list offering what is already typed covers the next field for nothing.
func shouldShowRecipientSuggestions(_ field: String, _ suggestions: [String]) -> Bool {
    let token = currentRecipientToken(field)
    return !token.isEmpty
        && !suggestions.isEmpty
        && !suggestions.contains { $0.compare(token, options: .caseInsensitive) == .orderedSame }
}

/// Whether the composer must open with Cc and Bcc revealed, given what the caller pre-filled them
/// with.
///
/// The row is collapsed by default, so anything a caller puts in it would otherwise be a recipient
/// the sender cannot see, and cannot remove. A `mailto:` link may name `bcc`, which makes this a
/// security rule rather than a nicety (docs/composer-security.md, Gate 12); a reply-all and an
/// assistant's draft reach it the same way. Whitespace is not an address.
func revealsCcBcc(cc: String, bcc: String) -> Bool {
    !cc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        || !bcc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
}
