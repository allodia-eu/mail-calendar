// Opening a draft from the Drafts folder back into its composer (docs/drafts.md).
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
    /// Opens a Drafts-folder row back into its composer, and every other row for reading.
    /// </summary>
    /// <remarks>
    /// Told by the <b>folder</b>, not by the row: the core answers per message but the list does
    /// not carry it, so a draft met in a search result or inside a thread opens read-only
    /// (<c>docs/drafts.md</c>, known gaps).
    /// <para>Resuming is a round trip and, unlike a forward, cannot count on the message being
    /// cached: a draft is opened from a list row, so the first open of one fetches it. It runs off
    /// the UI thread for that reason, and the composer appears when the answer does. A draft that
    /// could not be opened is <b>said</b>, and <paramref name="read"/> runs instead: a composer
    /// opened without the draft's content would replace it with what was on screen the next time
    /// it saved.</para>
    /// </remarks>
    internal async Task OpenOrResumeAsync(string account, string key, Action read)
    {
        if (!Model.ShowingDrafts)
        {
            read();
            return;
        }
        if (await OpenComposerAsync(new ComposeOrigin.Draft(account, key)) is { } context)
        {
            BeginCompose(context);
            return;
        }
        read();
    }
}