// The composer's recipient-autosuggest text handling: which part of a To/Cc/Bcc field the user is
// currently typing, and what the field becomes when they pick a suggestion.
//
// A plain class, not a knot of `remember`s inside the composable, so the JVM suite can drive it
// without composing a screen (AGENTS.md; the same shape SwipeUndoController follows). Everything
// here is pure string work, the ranking lives in the Rust core.
//
// The whole problem is that these fields hold a *list*: "ada@example.test, gr" is one string whose
// last token is what the user is completing. Query the core with the whole field and it matches
// nothing; replace the whole field on selection and the recipients already entered are destroyed.
package eu.allodia.mailcal

/** The separator between recipients in a To/Cc/Bcc field, and what a completed entry ends with. */
private const val SEPARATOR = ", "

/**
 * The token the caret is in, the text after the last comma, trimmed.
 *
 * This is what gets sent to the core as the autosuggest query. Empty when the field is empty or
 * ends at a separator, which the core answers with no suggestions (so the dropdown closes rather
 * than listing everyone the moment a recipient is completed).
 */
internal fun currentRecipientToken(field: String): String =
    field.substringAfterLast(',').trim()

/**
 * The field after accepting [email] for the token currently being typed.
 *
 * Replaces **only** the last token and appends a separator, so the already-entered recipients
 * survive and the user can keep typing the next one without reaching for the comma key.
 *
 * The address is inserted bare rather than as `Name <address>`: the core's submit path parses
 * addresses, a display name adds nothing it uses, and a name containing a comma would split into
 * two invalid recipients, a corruption the user would not see until the send failed.
 */
internal fun acceptRecipientSuggestion(field: String, email: String): String {
    val head = field.substringBeforeLast(',', missingDelimiterValue = "")
    return if (head.isEmpty()) {
        email + SEPARATOR
    } else {
        head.trimEnd() + SEPARATOR + email + SEPARATOR
    }
}

/**
 * Whether a suggestion is worth showing for [field] given the [suggestions] the core returned.
 *
 * Hidden once the current token is exactly one of the suggested addresses: the user has finished
 * that recipient, and a dropdown offering what is already typed covers the next field for nothing.
 */
internal fun shouldShowSuggestions(field: String, suggestions: List<String>): Boolean {
    val token = currentRecipientToken(field)
    return token.isNotEmpty() &&
        suggestions.isNotEmpty() &&
        suggestions.none { it.equals(token, ignoreCase = true) }
}

/**
 * The recipients in [field] the user has **finished**, everything before the last comma.
 *
 * These are what the UI draws as pills. The split is the same one autosuggest already uses (the
 * trailing token is what is being typed, everything before it is settled), so the pills and the
 * completion cannot disagree about where one recipient ends and the next begins.
 *
 * Empty entries are dropped, so a stray `,,` or a trailing separator never becomes a blank pill.
 */
internal fun committedRecipients(field: String): List<String> =
    field.substringBeforeLast(',', missingDelimiterValue = "")
        .split(',')
        .map { it.trim() }
        .filter { it.isNotEmpty() }

/**
 * Rebuilds the field's text from finished [committed] recipients plus the [token] being typed.
 *
 * The inverse of [committedRecipients] / [currentRecipientToken], and the only place the field's
 * string is assembled, so removing a pill and accepting a suggestion cannot drift apart on
 * spacing.
 */
internal fun recipientFieldText(committed: List<String>, token: String): String =
    committed.joinToString("") { "$it$SEPARATOR" } + token

/**
 * A field the composer was **opened with**, a reply's derived recipients, with every address in
 * it marked finished.
 *
 * The trailing-token rule above infers what the user is *in the middle of typing*, and that
 * inference is simply wrong for a value nobody typed: everything the caller supplied is a complete
 * address. Seeded raw, a reply-all's `"bestuur@…, tc@…"` puts the last recipient in the input as
 * half-typed text beside one pill, and a single-address Cc gets no pill at all, the field looks
 * like it lost the recipients it is in fact holding. So the seed is normalised once, here, at the
 * only moment it is known that nothing is in progress.
 *
 * Idempotent, so a caller that already ends at a separator is unchanged, and an empty field stays
 * empty rather than becoming a lone separator (Send is gated on a non-blank To).
 */
internal fun seededRecipientField(field: String): String =
    recipientFieldText(field.split(',').map { it.trim() }.filter { it.isNotEmpty() }, "")

/**
 * The field with the pill at [index] removed, keeping the token currently being typed.
 *
 * An out-of-range index returns the field unchanged rather than throwing: the pill list and a tap
 * on it are a frame apart, and a recomposition in between must not crash the composer.
 */
internal fun removeRecipient(field: String, index: Int): String {
    val committed = committedRecipients(field)
    if (index !in committed.indices) {
        return field
    }
    return recipientFieldText(
        committed.filterIndexed { at, _ -> at != index },
        currentRecipientToken(field),
    )
}
