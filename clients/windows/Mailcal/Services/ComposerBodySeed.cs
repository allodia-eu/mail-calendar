// The editor call that seeds a composer's body, as a plain rule the WinUI composer runs.
// WinUI-free so Mailcal.Tests can link it: the composer view itself cannot be built there.

using System.Text.Json;

namespace Allodia.Mailcal.Services;

/// <summary>The script that puts a composer's opening body into the shared editor.</summary>
internal static class ComposerBodySeed
{
    /// <summary>The call for a composer opening on <paramref name="storedHtml"/>, the HTML of a
    /// message it reopens (a resumed draft, or one moved back out of the Outbox), or on the text
    /// <paramref name="text"/>; <c>null</c> when it opens empty.</summary>
    /// <remarks>
    /// A reopened message goes through <c>setComposerBody</c>, which reads its HTML back into the
    /// editor's document: its formatting, its pictures, its quote and its signature. The text rides
    /// beside it for a message with no HTML. Everything is passed as one JSON string the editor
    /// parses, never spliced into the script, so no body can close the call and run as code.
    /// </remarks>
    internal static string? Script(string? storedHtml, string? text)
    {
        if (!string.IsNullOrWhiteSpace(storedHtml))
        {
            var seed = JsonSerializer.Serialize(new { html = storedHtml, text = text ?? "" });
            return $"window.setComposerBody({JsonSerializer.Serialize(seed)})";
        }
        return string.IsNullOrEmpty(text) ? null : $"window.setPlainText({JsonSerializer.Serialize(text)})";
    }
}
