// Opening a draft back into its composer, wherever it is listed (docs/drafts.md).
//
// Where the composer goes is MainWindow.Compose.cs, which this reuses: a resumed draft is an
// ordinary composer in the reading pane's column, differing only in what it opens with.

using System.Threading.Tasks;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    /// <summary>
    /// Opens a draft back into its composer, and every other message for reading.
    /// </summary>
    /// <remarks>
    /// Told by the <b>row</b> (<paramref name="draft"/>), never by the folder, so a draft met in a
    /// search result or in a conversation shown from another folder resumes as one in Drafts does.
    /// <para>Resuming is a round trip and, unlike a forward, cannot count on the message being
    /// cached: a draft is opened from a list row, so the first open of one fetches it. It runs off
    /// the UI thread for that reason, and the composer appears when the answer does. A draft that
    /// could not be opened is <b>said</b>, and <paramref name="read"/> runs instead: a composer
    /// opened without the draft's content would replace it with what was on screen the next time
    /// it saved.</para>
    /// </remarks>
    internal async Task OpenOrResumeAsync(string account, string key, bool draft, Action read)
    {
        if (!draft)
        {
            read();
            return;
        }
        // Said, because a draft that opens for reading looks the same whichever of the two it was.
        var busy = _composerOpening;
        if (await OpenComposerAsync(new ComposeOrigin.Draft(account, key)) is { } context)
        {
            BeginCompose(context);
            return;
        }
        Log.Debug(busy
            ? "drafts: opened a draft for reading, since another composer was still opening"
            : "drafts: opened a draft for reading, since it could not be opened to edit");
        read();
    }
}