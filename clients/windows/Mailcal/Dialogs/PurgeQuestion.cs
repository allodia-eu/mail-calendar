// The question before a permanent delete (docs/command-bar.md, rule 6), wherever it is asked from:
// the command bar, a row menu, the Delete key on the list and the reading pane's Delete. It names
// what the rows are and counts them when there are several, and its button says Delete
// permanently: Delete alone moves to Trash everywhere else.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

internal static class PurgeQuestion
{
    /// <summary>Asks whether to delete for good what <paramref name="wording"/> names,
    /// <paramref name="count"/> of them; true on yes.</summary>
    internal static async Task<bool> AskAsync(XamlRoot root, PermanentDeleteWording wording, int count)
    {
        var message = wording switch
        {
            PermanentDeleteWording.Messages => L10n.DeletePermanentlyMessageMany(count),
            PermanentDeleteWording.Conversation => L10n.DeletePermanentlyConversation(),
            PermanentDeleteWording.Conversations => L10n.DeletePermanentlyConversations(count),
            _ => L10n.DeletePermanentlyMessage(),
        };
        var result = await DialogHelper.ConfirmAsync(root, L10n.DeletePermanentlyTitle(), message, L10n.ActionDeletePermanently());
        return result == ContentDialogResult.Primary;
    }
}
