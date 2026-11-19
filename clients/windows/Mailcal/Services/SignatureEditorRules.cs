// What the Settings signature editor does with the core's page (docs/signatures.md, "Changing the
// library"): which signature it saves under, Save's state, which outcome closes it and how a
// refusal is worded. WinUI-free and L10n-free, so Mailcal.Tests can link it; the words arrive as
// parameters and the editor itself (Dialogs/SettingsDialog.SignatureEditor.cs) draws the answer.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>What the editor does with an outcome it has not acted on yet.</summary>
internal enum SignatureEditorStep
{
    /// <summary>Nothing: no outcome, one it already acted on, or one for another signature.</summary>
    Stay,

    /// <summary>The change it asked for is done, or done and not written, which the failed-save
    /// notice says: the editor closes.</summary>
    Close,

    /// <summary>The save was refused: the editor stays open and says why.</summary>
    Refused,
}

/// <summary>The catalog's words for why a signature or a picture was refused.</summary>
/// <param name="NameRequired"><c>settings_signatures_name_required</c>.</param>
/// <param name="Gone"><c>settings_signatures_gone</c>.</param>
/// <param name="ImageFormat"><c>settings_signatures_image_format</c>.</param>
/// <param name="ImageTooLarge"><c>settings_signatures_image_too_large</c>, given the limit.</param>
internal sealed record SignatureRefusalWords(
    string NameRequired,
    string Gone,
    string ImageFormat,
    Func<string, string> ImageTooLarge);

/// <summary>
/// One opening of the editor: the signature it saves under, and the outcomes it has already seen,
/// so each newer outcome for that signature is acted on once.
/// </summary>
internal sealed class SignatureEditorSession
{
    private ulong _seen;

    private SignatureEditorSession(string id, SignaturesPage page)
    {
        Id = id;
        _seen = page.Outcome?.Serial ?? 0;
    }

    /// <summary>The id every save from this editor names.</summary>
    internal string Id { get; }

    /// <summary>An editor on a new signature, which saves under the page's <c>new_id</c>.</summary>
    internal static SignatureEditorSession New(SignaturesPage page) => new(page.NewId, page);

    /// <summary>An editor on <paramref name="entry"/>.</summary>
    internal static SignatureEditorSession Edit(SignatureEntry entry, SignaturesPage page) => new(entry.Id, page);

    /// <summary>What to do with the page's <paramref name="outcome"/>; <paramref name="reason"/>
    /// is set for <see cref="SignatureEditorStep.Refused"/>.</summary>
    internal SignatureEditorStep Read(SignatureOutcome? outcome, out SignatureRefusal? reason)
    {
        reason = null;
        if (outcome is null || outcome.Serial <= _seen || outcome.Id != Id)
        {
            return SignatureEditorStep.Stay;
        }
        _seen = outcome.Serial;
        switch (outcome.Result)
        {
            case SignatureResult.Refused refused:
                reason = refused.Reason;
                return SignatureEditorStep.Refused;
            case SignatureResult.Saved or SignatureResult.Deleted or SignatureResult.NotWritten:
                return SignatureEditorStep.Close;
            default:
                return SignatureEditorStep.Stay;
        }
    }

    /// <summary>Save as drawn: the page's, and held besides while the name typed is refused
    /// (<c>signature_name_refusal</c>).</summary>
    internal static ControlState SaveState(ControlState save, SignatureRefusal? nameRefusal) =>
        new(save.Enabled && nameRefusal is null, save.Running);

    /// <summary>Why a save was refused, in the catalog's words.</summary>
    internal static string Say(SignatureRefusal refusal, SignatureRefusalWords words) => refusal switch
    {
        SignatureRefusal.Image image => Say(image.Refusal, words),
        SignatureRefusal.Gone => words.Gone,
        SignatureRefusal.BlankName => words.NameRequired,
        _ => throw new ArgumentOutOfRangeException(nameof(refusal), refusal, null),
    };

    /// <summary>Why a picture may not go into a signature, in the catalog's words. The limit is
    /// the core's, said as File Explorer says a file's size.</summary>
    internal static string Say(SignatureImageRefusal refusal, SignatureRefusalWords words) => refusal switch
    {
        SignatureImageRefusal.TooLarge large => words.ImageTooLarge(ByteSize.Format(large.LimitBytes)),
        SignatureImageRefusal.Format => words.ImageFormat,
        _ => throw new ArgumentOutOfRangeException(nameof(refusal), refusal, null),
    };
}
