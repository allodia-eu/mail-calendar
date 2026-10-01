// Move to folder… on a message's row menu (docs/folder-pane.md, rule 24): the same move a drop onto
// a folder makes, reached from the keyboard and a screen reader as well as the pointer.
//
// Like every row menu item it acts on the whole selection when its row is in it, and on that row
// alone when it is not (docs/list-selection.md, rule 12). What it lists is FolderActions, gated by
// Mailcal.Tests; this half reads the menu's row and the model and shows the picker.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class MailListView
{
    /// <summary>The automation id the item carries in both row menus, which is also how the
    /// menu finds it when it opens.</summary>
    private const string MoveToFolderItemId = "MoveToFolderItem";

    /// <summary>What Move to folder… would move and where it may go, or <c>null</c> when it has
    /// nothing to offer: rows of several accounts, or no folder left to list.</summary>
    private sealed record MovePlan(SelectedRow[] Rows, string Account, IReadOnlyList<MoveTarget> Targets);

    private MovePlan? MoveToFolderPlan(MailRow row)
    {
        if (Model is not { } model)
        {
            return null;
        }
        IEnumerable<ISelectableRow> acted = model.SelectionCovers(row) ? model.SelectedRows : [row];
        var rows = SelectionActions.Rows(acted);
        if (FolderActions.MessageAccount(rows) is not { } account)
        {
            return null;
        }
        // The list shows one folder only within its own account; a unified view shows none.
        var showing = model.SelectedAccount == account ? model.SelectedFolder : null;
        var targets = FolderActions.MessageTargets(model.FoldersOf(account), showing);
        return targets.Count == 0 ? null : new MovePlan(rows, account, targets);
    }

    // Decided as the menu opens rather than bound, because the answer depends on the selection and
    // the folder tree at that moment, and neither is a property of the row.
    private void OnRowMenuOpening(object sender, object e)
    {
        if (sender is not MenuFlyout flyout)
        {
            return;
        }
        foreach (var item in flyout.Items.OfType<MenuFlyoutItem>())
        {
            if (AutomationProperties.GetAutomationId(item) == MoveToFolderItemId)
            {
                item.Visibility = RowOf(item) is { } row && MoveToFolderPlan(row) is not null
                    ? Visibility.Visible
                    : Visibility.Collapsed;
            }
        }
    }

    // The move goes through the drop's own path, so the reading pane lets go of a message that
    // leaves in the same way. The moved rows leave the list, and the selection with them, as they
    // do after the bar's Archive.
    private async void OnMoveToFolder(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row || MoveToFolderPlan(row) is not { } plan)
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
