// The signature surface of the model: the library's CRUD, the per-account assignments, and the two
// resolutions a composer runs (docs/signatures.md). Split into its own partial to keep
// MailboxModel.cs under the 500-line limit.
//
// State lives in Rust, the library in signatures.toml, the per-account pointers in
// preferences.toml, and every mutator re-signals Surface.Settings. Nothing is cached here on
// purpose: the assignment can change under an open composer, and a stale copy would send the wrong
// signature, which is precisely the failure this feature exists to prevent.
//
// These carry the generated UniFFI types (SignaturesSnapshot / SignatureRow / SignatureBody /
// SignatureSlotKind), which are `internal`, the generated types stay confined to this service
// layer. Internal is enough: the settings dialog and the composer that consume them are in this
// same assembly, exactly as with SwipeSettings.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Whether the core is up, used only to tell "the library is empty" apart from
    /// "we asked before connecting" in the diagnostic log.</summary>
    internal bool HasCore => _app is not null;

    /// <summary>The user's signature library in their chosen order, plus one row per configured
    /// account carrying its two assignments. Metadata only, a body is fetched one at a time with
    /// <see cref="SignatureHtml"/>, so drawing a list of names never drags an embedded logo across
    /// the FFI. Empty before the app has connected.</summary>
    internal SignaturesSnapshot Signatures =>
        _app?.Signatures()
        ?? new SignaturesSnapshot(Array.Empty<SignatureRow>(), Array.Empty<AccountSignatureRow>());

    /// <summary>One signature's stored HTML body, or <c>null</c> when the id names nothing, what
    /// the signature editor loads when the user opens an existing signature.</summary>
    internal string? SignatureHtml(string id) => _app?.SignatureHtml(id);

    /// <summary>Creates a signature and returns its row, <b>including the minted id</b>, so the
    /// caller can select what it just created without re-pulling the snapshot and guessing which row
    /// is new.</summary>
    internal SignatureRow? CreateSignature(string name, string bodyHtml, string bodyPlain) =>
        _app?.CreateSignature(name, bodyHtml, bodyPlain);

    /// <summary>Replaces a signature's name and body. An unknown id is a no-op, never a silent
    /// create.</summary>
    internal void UpdateSignature(string id, string name, string bodyHtml, string bodyPlain) =>
        _app?.UpdateSignature(id, name, bodyHtml, bodyPlain);

    /// <summary>Deletes a signature. The core clears it from every account slot that pointed at it,
    /// across accounts, so no assignment is left naming something that no longer exists, this
    /// client does not have to (and must not) do that teardown itself.</summary>
    internal void DeleteSignature(string id) => _app?.DeleteSignature(id);

    /// <summary>Assigns (or clears, with <c>null</c>) which signature an account uses in one slot.
    /// An id naming nothing in the library clears the slot rather than storing a pointer that
    /// resolves to nothing.</summary>
    internal void SetAccountSignature(string account, SignatureSlotKind slot, string? signature) =>
        _app?.SetAccountSignature(account, slot, signature);

    /// <summary>The signature a composer should open with for <paramref name="account"/> in
    /// <paramref name="slot"/>, or <c>null</c> when that slot is unassigned. Called when a composer
    /// opens and <b>again whenever its From account changes</b>, so the account's own signature
    /// follows the sender.</summary>
    internal SignatureBody? ResolveSignature(string account, SignatureSlotKind slot) =>
        _app?.ResolveSignature(account, slot);

    /// <summary>One signature's id and both bodies by id, the composer's per-message override,
    /// where the user names a signature directly instead of inheriting the account's.</summary>
    internal SignatureBody? SignatureBodyOf(string id) => _app?.SignatureBody(id);
}
