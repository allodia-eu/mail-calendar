// A picked picture as the signature editor inserts it: a `data:` URI and its alt text. Whether it
// may go into a signature at all is the core's rule (signature_image_refusal), asked before the
// file is read (Dialogs/SettingsDialog.SignatureEditor.cs).
//
// A signature stores its images inline, which is right for the library (one self-contained file) and
// what the core rewrites to a `cid:` part on send, because Outlook's reader blocks `data:` images
// (docs/signatures.md).

using System.Text.Json;

namespace Allodia.Mailcal.Services;

internal static class SignatureImage
{
    /// <summary><paramref name="bytes"/> as a <c>data:</c> URI of <paramref name="mediaType"/>,
    /// the type the system reports for the file.</summary>
    internal static string DataUrl(byte[] bytes, string mediaType) =>
        $"data:{mediaType};base64,{Convert.ToBase64String(bytes)}";

    /// <summary>The <c>insertSignatureImage</c> payload. The alt text rides as a property the
    /// editor assigns to <c>node.alt</c>, so it cannot carry markup into the attribute.</summary>
    internal static string Payload(string dataUrl, string altText) =>
        JsonSerializer.Serialize(new Dictionary<string, string>
        {
            ["data_url"] = dataUrl,
            ["alt_text"] = altText,
        });
}
