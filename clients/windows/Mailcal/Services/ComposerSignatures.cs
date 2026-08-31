// The three pure rules the composer's signature handling turns on (docs/signatures.md). They live
// here rather than inside ComposerView so the test project can pin them without a WinUI host, the
// same reason `signatureSlot` is a free function on Apple and Android.
//
// All three are silent when wrong, which is why they are pinned rather than trusted: a mis-mapped
// slot sends the reply signature on a new message and nobody notices until a recipient mentions it;
// a resolution that ignores an explicit choice undoes a deliberate act on send; and a seed payload
// with the wrong key names reaches `setComposerSignature` as an object with no `body_html`, which
// the editor reads as "remove the signature", so the message simply goes out without one.

using System.Text.Json;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>
/// What signature this one message carries, when the user has said so explicitly. **Absence of a
/// choice**, a <c>null</c> <c>SignatureChoice</c>, the state a composer opens in, means FOLLOW THE
/// ACCOUNT: it re-resolves whenever the From picker changes, which is what a user who never touched
/// the picker expects (their work signature when sending from work).
///
/// Once they do pick, the choice sticks even across a From change: they chose it *for this message*,
/// and silently replacing it would undo a deliberate act. (Outlook re-swaps regardless; it is its
/// most complained-about composer behaviour.)
/// </summary>
/// <param name="Id">The chosen signature's id, or <c>null</c> for the picker's <b>None</b>, an
/// explicit "no signature on this message", which is not the same as having made no choice.</param>
internal sealed record SignatureChoice(string? Id);

/// <summary>The composer's signature rules, free of WinUI so they can be tested directly.</summary>
internal static class ComposerSignatures
{
    /// <summary>
    /// Which of the account's two slots a composer opened for <paramref name="kind"/> seeds from.
    /// A reply, a reply-all and a forward share one slot (Outlook's grouping): all three continue an
    /// existing message, and splitting them produces a setting nobody sets.
    /// </summary>
    internal static SignatureSlotKind SlotFor(RichComposeKind kind) =>
        kind == RichComposeKind.New ? SignatureSlotKind.NewMessage : SignatureSlotKind.ReplyForward;

    /// <summary>
    /// The signature on this message right now: the user's explicit choice if they made one, else
    /// whatever <paramref name="account"/> assigns for this <paramref name="kind"/>. The two lookups
    /// are passed in rather than read from the core here, so this stays testable, and they are run
    /// on every call rather than cached, because an assignment can change under an open composer.
    /// </summary>
    internal static SignatureBody? Resolve(
        SignatureChoice? choice,
        string? account,
        RichComposeKind kind,
        Func<string, SignatureSlotKind, SignatureBody?> forAccount,
        Func<string, SignatureBody?> byId) => choice switch
        {
            null => account is null ? null : forAccount(account, SlotFor(kind)),
            { Id: null } => null,
            { Id: { } id } => byId(id),
        };

    /// <summary>
    /// The <c>setComposerSignature</c> payload: the shape the Rust composer's <c>Block::Signature</c>
    /// round-trips, so what the editor hands back on submit is what the core already understands.
    /// <c>null</c> for no signature, which the editor seam reads as "remove the region".
    /// </summary>
    internal static string? SeedJson(SignatureBody? body) => body is null
        ? null
        : JsonSerializer.Serialize(new SignatureSeed(body.BodyHtml, body.BodyPlain));

    // The wire shape of the seed. The property names are the Rust field names verbatim, the editor
    // reads `body_html`/`body_plain` and emits the same two back inside the Signature block.
    private sealed record SignatureSeed(
        [property: System.Text.Json.Serialization.JsonPropertyName("body_html")] string BodyHtml,
        [property: System.Text.Json.Serialization.JsonPropertyName("body_plain")] string BodyPlain);
}
