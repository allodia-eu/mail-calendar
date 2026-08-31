// The composer's recipient-field string work: which part of a To/Cc/Bcc field the user is currently
// typing, which recipients are finished, and what the field becomes when one is accepted or removed.
//
// Plain BCL with no WinUI in sight, so Mailcal.Tests drives it directly, the same shape the Apple
// client's `RecipientTokens.swift` and Android's `RecipientAutosuggest.kt` take. The ranking itself
// lives in the Rust core; everything here is string handling.
//
// The whole problem is that these fields hold a *list*: "ada@example.test, gr" is one string whose
// last token is what the user is completing. Query the core with the whole field and it matches
// nothing once a first recipient is entered; replace the whole field on selection and every
// recipient already typed is silently destroyed. Both failures are silent, which is why
// docs/contacts.md §4 binds every client to the same rule and why this is factored out and tested
// rather than inlined into the view.

using System.Collections.Generic;
using System.Linq;

namespace Allodia.Mailcal.Services;

/// <summary>The string ↔ (finished recipients, token being typed) split a recipient field rides on.</summary>
internal static class RecipientTokens
{
    /// <summary>The separator between recipients, and what a completed entry ends with.</summary>
    private const string Separator = ", ";

    /// <summary>
    /// The token the caret is in, the text after the last comma, trimmed.
    /// </summary>
    /// <remarks>
    /// This is what goes to the core as the autosuggest query. Empty when the field is empty or ends
    /// at a separator, which the core answers with no suggestions, so the list closes rather than
    /// offering everyone the moment a recipient is completed.
    /// </remarks>
    internal static string CurrentToken(string field)
    {
        var lastComma = field.LastIndexOf(',');
        var tail = lastComma < 0 ? field : field[(lastComma + 1)..];
        return tail.Trim();
    }

    /// <summary>
    /// The recipients in <paramref name="field"/> the user has <em>finished</em>, everything before
    /// the last comma.
    /// </summary>
    /// <remarks>
    /// These are what the UI draws as pills. The split is the same one autosuggest uses (the trailing
    /// token is what is being typed; everything before it is settled), so the pills and the completion
    /// cannot disagree about where one recipient ends and the next begins.
    /// <para>Empty entries are dropped, so a stray <c>,,</c> or a trailing separator never becomes a
    /// blank pill.</para>
    /// </remarks>
    internal static IReadOnlyList<string> Committed(string field)
    {
        var lastComma = field.LastIndexOf(',');
        if (lastComma < 0)
        {
            return [];
        }
        return [.. field[..lastComma]
            .Split(',')
            .Select(entry => entry.Trim())
            .Where(entry => entry.Length > 0)];
    }

    /// <summary>
    /// Rebuilds the field's text from the finished <paramref name="committed"/> recipients plus the
    /// <paramref name="token"/> being typed.
    /// </summary>
    /// <remarks>
    /// The inverse of <see cref="Committed"/> / <see cref="CurrentToken"/>, and the only place the
    /// field's string is assembled, so removing a pill and accepting a suggestion cannot drift
    /// apart on spacing.
    /// </remarks>
    internal static string FieldText(IReadOnlyList<string> committed, string token) =>
        string.Concat(committed.Select(recipient => recipient + Separator)) + token;

    /// <summary>
    /// A field the composer was <em>opened with</em>, a reply's derived recipients, an assistant's
    /// draft, with every address in it marked finished.
    /// </summary>
    /// <remarks>
    /// The trailing-token rule infers what the user is <em>in the middle of typing</em>, and that
    /// inference is simply wrong for a value nobody typed: everything the caller supplied is a
    /// complete address. Seeded raw, a reply-all's <c>"bestuur@…, tc@…"</c> puts the last recipient
    /// in the input as half-typed text beside one pill, and a single-address Cc gets no pill at all
    /// The field looks like it lost the recipients it is in fact holding. So the seed is
    /// normalised once, here, at the only moment it is known that nothing is in progress.
    /// <para>Idempotent, so a caller that already ends at a separator is unchanged, and an empty
    /// field stays empty rather than becoming a lone separator (Send is gated on a non-blank
    /// To).</para>
    /// </remarks>
    internal static string Seeded(string field) =>
        FieldText(
            [.. field.Split(',').Select(entry => entry.Trim()).Where(entry => entry.Length > 0)],
            string.Empty);

    /// <summary>
    /// The field after accepting <paramref name="email"/> for the token currently being typed.
    /// </summary>
    /// <remarks>
    /// Replaces <b>only</b> the last token and appends a separator, so the recipients already entered
    /// survive and the user can type the next one without reaching for the comma key.
    /// <para>The address is inserted bare rather than as <c>Name &lt;address&gt;</c>: the core's
    /// submit path parses addresses, a display name adds nothing it uses, and a name containing a
    /// comma would split into two invalid recipients, a corruption the user would not see until the
    /// send failed.</para>
    /// </remarks>
    internal static string Accept(string field, string email) =>
        FieldText([.. Committed(field), email], string.Empty);

    /// <summary>
    /// The field with the pill at <paramref name="index"/> removed, keeping the token being typed.
    /// </summary>
    /// <remarks>
    /// An out-of-range index returns the field unchanged rather than throwing: the pill list and a
    /// click on it are a frame apart, and a re-render in between must not crash the composer.
    /// </remarks>
    internal static string Remove(string field, int index)
    {
        var committed = Committed(field).ToList();
        if (index < 0 || index >= committed.Count)
        {
            return field;
        }
        committed.RemoveAt(index);
        return FieldText(committed, CurrentToken(field));
    }

    /// <summary>
    /// Whether the composer must open with Cc and Bcc revealed, given what the request pre-filled
    /// them with.
    /// </summary>
    /// <remarks>
    /// The row is collapsed by default, so anything a caller puts in it would otherwise be a
    /// recipient the sender cannot see, and cannot remove. A <c>mailto:</c> link may name
    /// <c>bcc</c>, which makes this a security rule rather than a nicety
    /// (docs/composer-security.md, Gate 12); a reply-all and an assistant's draft reach it the same
    /// way. Whitespace is not an address.
    /// </remarks>
    internal static bool RevealsCcBcc(string cc, string bcc) =>
        !string.IsNullOrWhiteSpace(cc) || !string.IsNullOrWhiteSpace(bcc);

    /// <summary>
    /// Whether a suggestion list is worth showing for <paramref name="field"/>, given the addresses
    /// the core returned.
    /// </summary>
    /// <remarks>
    /// Hidden once the current token is exactly one of them: the user has finished that recipient,
    /// and a list offering what is already typed covers the next field for nothing.
    /// </remarks>
    internal static bool ShouldShowSuggestions(string field, IReadOnlyList<string> suggestions)
    {
        var token = CurrentToken(field);
        return token.Length > 0
            && suggestions.Count > 0
            && !suggestions.Any(suggestion =>
                string.Equals(suggestion, token, StringComparison.OrdinalIgnoreCase));
    }
}
