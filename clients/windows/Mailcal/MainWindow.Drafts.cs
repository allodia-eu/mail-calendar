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
        var composition = Guid.NewGuid().ToString("N");
        var directory = MailboxModel.DraftStagingDirectory();
        var resumed = await Task.Run(
            () => Model.ResumeDraft(composition, account, key, directory));
        if (resumed is null)
        {
            read();
            await DialogHelper.TellAsync(Content.XamlRoot, L10n.ComposeDraftOpenFailed());
            return;
        }
        BeginCompose(ReopenedContext(resumed));
    }

    /// <summary>What a composer reopened on a message that already exists opens with: a draft
    /// resumed from Drafts, or a message moved back out of the Outbox. One context for both,
    /// because the core answers both with one <see cref="uniffi.mailcal_bindings.ComposeRequest"/>
    /// and the composer must open each holding all of it.</summary>
    /// <remarks>
    /// Two things separate it from every other new message. The composition is the one the core
    /// joined the stored copy to, never a fresh one, or the composer's first save would store a
    /// second copy beside the one it is showing. And it seeds <b>no signature</b>: the body already
    /// carries the signature it was written with, so seeding one would put a second under it and
    /// the next save would store that.
    /// </remarks>
    // Qualified, and it has to be: this file has both namespaces in scope, and the core's
    // ComposeRequest and this client's ComposeContext are two different things (ComposeContext.cs).
    internal ComposeContext ReopenedContext(uniffi.mailcal_bindings.ComposeRequest request)
    {
        var quotes = Model.QuoteSettings;
        return new ComposeContext(
            RichComposeKind.New,
            Account: null,
            Key: null,
            InitialFrom: Model.SendAccount(request.Account)?.Id,
            InitialTo: request.To,
            InitialCc: request.Cc,
            Quote: null,
            QuoteStyle: quotes.Style,
            QuoteStylePerMessage: quotes.PerMessage,
            InitialBcc: request.Bcc,
            InitialSubject: request.Subject,
            InitialBody: request.BodyText,
            SeedsSignature: false,
            Attachments: request.Attachments,
            Composition: request.Composition,
            StoredHtml: request.BodyHtml);
    }
}
