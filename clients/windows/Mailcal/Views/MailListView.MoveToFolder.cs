// Move to folder… on a row menu (docs/folder-pane.md, rule 24): the same move a drop onto a folder
// makes, reached from the keyboard and a screen reader as well as the pointer.
//
// Like every row menu item it acts on the whole selection when its row is in it, and on that row
// alone when it is not (docs/list-selection.md, rule 12). Whether a move would take anything is the
// core's (docs/row-actions.md); what the picker lists is FolderActions, gated by Mailcal.Tests. This
// half reads the rows and the model and shows the picker.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class MailListView
{
    /// <summary>What Move to folder… would move and where it may go, or <c>null</c> when it has
    /// nothing to offer: rows of several accounts, or no folder left to list.</summary>
    private sealed record MovePlan(SelectedRow[] Rows, string Account, IReadOnlyList<MoveTarget> Targets);

    private MovePlan? MoveToFolderPlan(SelectedRow[] rows)
    {
        if (Model is not { } model || FolderActions.MessageAccount(rows) is not { } account)
        {
            return null;
        }
        // The list shows one folder only within its own account; a unified view shows none.
        var showing = model.SelectedAccount == account ? model.SelectedFolder : null;
        var targets = FolderActions.MessageTargets(model.FoldersOf(account), showing);
        return targets.Count == 0 ? null : new MovePlan(rows, account, targets);
    }

    // The move goes through the drop's own path, so the reading pane lets go of a message that
    // leaves in the same way. The moved rows leave the list, and the selection with them, as they
    // do after the bar's Archive. The folders are read when the item is chosen rather than when the
    // menu opened, in case the tree moved in between.
    private async Task MoveToFolderAsync(SelectedRow[] rows)
    {
        if (MoveToFolderPlan(rows) is not { } plan)
        {
            return;
        }
        var target = await FolderDialogs.AskMoveTargetAsync(XamlRoot, L10n.MessageMoveTitle(), plan.Targets);
        if (target?.Key is { } key)
        {
            Model?.MoveMessagesToFolder(plan.Rows, plan.Account, key);
        }
    }
}
