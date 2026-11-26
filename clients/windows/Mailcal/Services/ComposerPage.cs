// The page the shared editor writes its message on (docs/reading-dark-mode.md, "Writing in a dark
// app"), and the order an editor host hands the core what the editor posts. WinUI-free, and handed
// the core's scripts rather than calling them, so Mailcal.Tests pins both without the cdylib.

using System.Text.Json;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Where a message the editor posted went.</summary>
internal enum EditorPost
{
    /// <summary>A colours request, answered by the core's script.</summary>
    Answered,

    /// <summary>The toolbar's sun or moon, which set the dark-messages preference.</summary>
    Chosen,

    /// <summary>Neither: the host's own requests parse it.</summary>
    Request,
}

/// <summary>The editor's page, as a host hands it over and answers for it.</summary>
internal static class ComposerPage
{
    /// <summary>
    /// The script the web view runs before the editor's document does: the app's scheme for the
    /// chrome and, when the host names one, <paramref name="page"/> for the message, so the editor's
    /// first frame is drawn on both. <paramref name="pagePreload"/> is the core's
    /// <c>composer_page_preload_script</c>.
    /// </summary>
    internal static string Preload(bool appDark, MessageScheme? page, Func<MessageScheme, string> pagePreload)
    {
        var chrome = $"window.composerScheme = {JsonSerializer.Serialize(appDark ? "dark" : "light")};";
        return page is { } scheme ? chrome + pagePreload(scheme) : chrome;
    }

    /// <summary>
    /// Hands <paramref name="message"/> to the core in the order the contract sets: a colours
    /// request is answered with <paramref name="answer"/>, run as it is; the toolbar's sun or moon
    /// is the core's to store (<paramref name="choose"/>, null for a host that names no page); and
    /// anything else is the host's own request.
    /// </summary>
    internal static EditorPost Route(
        string message,
        Func<string, string?> coloursAnswer,
        Func<string, bool>? choose,
        out string? answer)
    {
        answer = coloursAnswer(message);
        if (answer is not null)
        {
            return EditorPost.Answered;
        }
        return choose?.Invoke(message) == true ? EditorPost.Chosen : EditorPost.Request;
    }
}
