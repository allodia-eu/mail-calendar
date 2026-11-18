// The signature surface of the model (docs/signatures.md): Settings → Signatures as the core draws
// it, the changes it dispatches, the two rules its editor asks, and the two resolutions a composer
// runs.
//
// Nothing is cached here: the page is read from the core's memory each time, and an assignment can
// change under an open composer, where a stale copy would send the wrong signature.
//
// These carry the generated UniFFI types, which are `internal`, so the generated types stay
// confined to this assembly, exactly as with SwipeSettings.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private static readonly ControlState Held = new(false, false);

    /// <summary>Raised when Settings → Signatures may have changed (<c>Surface.Signatures</c>,
    /// and <c>Surface.Settings</c>, which an account added or removed raises): the Settings page
    /// and every composer's signature picker read <see cref="SignaturesPage"/> again.</summary>
    internal event Action? SignaturesChanged;

    /// <summary>Whether the core is up, used only to tell "the library is empty" apart from
    /// "we asked before connecting" in the diagnostic log.</summary>
    internal bool HasCore => _app is not null;

    /// <summary>Settings → Signatures: the library, each account's two slots, every control on the
    /// page and what the last change came to. Read from memory, so the UI thread may ask it; a
    /// held, empty page before the app has connected.</summary>
    internal SignaturesPage SignaturesPage() =>
        _app?.SignaturesPage()
        ?? new SignaturesPage([], [], Held, string.Empty, Held, null);

    /// <summary>A change to the library or to an account's slot. Returns at once: a save or a
    /// deletion runs in the core, and what it came to is the page's outcome.</summary>
    internal void ChangeSignatures(SignatureIntent intent) => _app?.Dispatch(new Intent.Signature(intent));

    /// <summary>Why a signature named <paramref name="name"/> would not be saved, or <c>null</c>.</summary>
    internal static SignatureRefusal? NameRefusal(string name) =>
        MailcalBindingsMethods.SignatureNameRefusal(name);

    /// <summary>Why a picture of <paramref name="mediaType"/> and <paramref name="size"/> bytes may
    /// not go into a signature, or <c>null</c>. Asked before the file is read.</summary>
    internal static SignatureImageRefusal? ImageRefusal(string mediaType, ulong size) =>
        MailcalBindingsMethods.SignatureImageRefusal(mediaType, size);

    /// <summary>One signature's stored HTML body, or <c>null</c> when the id names nothing, what
    /// the signature editor loads when the user opens an existing signature.</summary>
    internal string? SignatureHtml(string id) => _app?.SignatureHtml(id);

    /// <summary>The signature a composer should open with for <paramref name="account"/> in
    /// <paramref name="slot"/>, or <c>null</c> when that slot is unassigned. Called when a composer
    /// opens and <b>again whenever its From account changes</b>, so the account's own signature
    /// follows the sender.</summary>
    internal SignatureBody? ResolveSignature(string account, SignatureSlotKind slot) =>
        _app?.ResolveSignature(account, slot);

    /// <summary>One signature's id and both bodies by id, the composer's per-message override,
    /// where the user names a signature directly instead of inheriting the account's.</summary>
    internal SignatureBody? SignatureBodyOf(string id) => _app?.SignatureBody(id);

    private void RaiseSignaturesChanged() => SignaturesChanged?.Invoke();
}
