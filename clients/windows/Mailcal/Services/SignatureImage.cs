// Turning a picked file into the `data:` URI a signature embeds, and saying why it can't, when it
// can't. The pure half lives here so the cap and the media-type refusal are testable without a file
// picker; the WinUI half (Dialogs/SettingsDialog.SignatureEditor.cs) reads the bytes and shows the
// message.
//
// A signature stores its images inline, which is right for the library (one self-contained file) and
// what the core rewrites to a `cid:` part on send, because Outlook's reader blocks `data:` images
// (docs/signatures.md).

using System.Globalization;

namespace Allodia.Mailcal.Services;

/// <summary>The outcome of picking an image for a signature.</summary>
internal abstract record SignatureImageOutcome
{
    /// <summary>A <c>data:image/…;base64,…</c> URI, ready to insert at the caret.</summary>
    /// <param name="Value">The URI.</param>
    /// <param name="AltText">The alt text to give the image (the file's base name).</param>
    internal sealed record DataUrl(string Value, string AltText) : SignatureImageOutcome;

    /// <summary>The file is over the per-image cap; carries the cap so the message can name it.</summary>
    /// <param name="LimitBytes">The cap, in bytes.</param>
    internal sealed record TooLarge(int LimitBytes) : SignatureImageOutcome;

    /// <summary>The file could not be read, or is not an image.</summary>
    internal sealed record Failed : SignatureImageOutcome;
}

/// <summary>The size cap and media-type rule an embedded signature image must pass.</summary>
internal static class SignatureImage
{
    /// <summary>
    /// The cap on an embedded signature image, in bytes. A signature rides in **every** message the
    /// account sends, so a 5 MB logo is 5 MB per mail, and base64 adds a third on top. 512 KB is
    /// generous for a logo and small enough that nobody notices it on the wire. Enforced where the
    /// file is picked, so the user is told; the core does not police it.
    /// </summary>
    internal const int LimitBytes = 512 * 1024;

    /// <summary>
    /// The outcome for <paramref name="bytes"/> of <paramref name="mediaType"/>. The size check is
    /// separate from the read failure so the user is told WHICH problem it is. Anything that is not
    /// an <c>image/*</c> is refused here rather than embedded: the editor would drop it anyway (it
    /// only accepts <c>data:image/</c>), and the picker is where the user can still be told.
    /// </summary>
    internal static SignatureImageOutcome From(byte[] bytes, string? mediaType, string altText)
    {
        if (bytes.Length > LimitBytes)
        {
            return new SignatureImageOutcome.TooLarge(LimitBytes);
        }
        if (bytes.Length == 0
            || mediaType is null
            || !mediaType.StartsWith("image/", StringComparison.OrdinalIgnoreCase))
        {
            return new SignatureImageOutcome.Failed();
        }
        return new SignatureImageOutcome.DataUrl(
            $"data:{mediaType.ToLowerInvariant()};base64,{Convert.ToBase64String(bytes)}",
            altText);
    }

    /// <summary>The cap as the message shows it ("512 KB"), in the current UI culture, the same
    /// decimal-KB convention DiagnosticsLog.FormatBytes and the Android/Apple limits use.</summary>
    internal static string FormatLimit(int bytes) =>
        $"{(bytes / 1000.0).ToString("0.#", CultureInfo.CurrentCulture)} KB";
}
