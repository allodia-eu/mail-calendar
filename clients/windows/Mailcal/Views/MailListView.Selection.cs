// The message list's multi-selection: reading what the ListView has picked, keeping the
// highlight honest against the reading pane, and the bar's buttons. Split from
// MailListView.xaml.cs to keep each file under the 500-line limit.
//
// The ListView owns the selection (docs/list-selection.md, rule 1). What this file adds is the
// two places WinUI's defaults are not what a mailbox wants: a modified click must not also open
// a message, and the auto-advancing reading-pane highlight must not collapse a selection the user
// is still building.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using uniffi.mailcal_bindings;
using Windows.System;
using Windows.UI.Core;

namespace Allodia.Mailcal.Views;

public sealed partial class MailListView : UserControl
{
    /// <summary>
    /// Whether the click that is being handled carried Ctrl or Shift, in which case it was aimed
    /// at the selection and must not also open a message: Ctrl-clicking twenty rows would
    /// otherwise fetch and display twenty bodies in turn.
    /// </summary>
    private static bool SelectionModifierDown =>
        IsDown(VirtualKey.Control) || IsDown(VirtualKey.Shift);

    private static bool IsDown(VirtualKey key) =>
        InputKeyboardSource
            .GetKeyStateForCurrentThread(key)
            .HasFlag(CoreVirtualKeyStates.Down);

    private void OnSelectionChanged(object sender, SelectionChangedEventArgs e) =>
        Model?.SetSelection(RowsList.SelectedItems.OfType<MailRow>());

    // Delete (and Backspace) on the list moves the selection to Trash; recoverable, so it asks
    // nothing (docs/list-selection.md, rule 6).
    private void OnDeleteSelection(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        Model?.ActOnSelection(BulkAction.Delete);
    }

    private void OnClearSelection(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        RowsList.SelectedItems.Clear();
    }

    private void OnClearSelectionClicked(object sender, RoutedEventArgs e) =>
        RowsList.SelectedItems.Clear();

    private void OnSelectAll(object sender, RoutedEventArgs e) => RowsList.SelectAll();

    // Whatever the button currently says, so a click runs the action the user read on it.
    private void OnSelectionToggleRead(object sender, RoutedEventArgs e) =>
        Model?.ActOnSelection(Model.SelectionReadAction);

    private void OnSelectionToggleFlag(object sender, RoutedEventArgs e) =>
        Model?.ActOnSelection(Model.SelectionFlagAction);

    private void OnSelectionArchive(object sender, RoutedEventArgs e) =>
        Model?.ActOnSelection(BulkAction.Archive);

    private void OnSelectionDelete(object sender, RoutedEventArgs e) =>
        Model?.ActOnSelection(BulkAction.Delete);

    // The one irreversible action on the bar, so it asks first, exactly as the row menu's does.
    private async void OnSelectionPermanentlyDelete(object sender, RoutedEventArgs e)
    {
        if (Model is not { } model || model.SelectionCount == 0)
        {
            return;
        }
        var result = await DialogHelper.ConfirmAsync(
            this.XamlRoot,
            L10n.DeletePermanentlyTitle(),
            L10n.DeletePermanentlyMessageMany(model.SelectionCount),
            L10n.ActionDelete());
        if (result == ContentDialogResult.Primary)
        {
            model.ActOnSelection(BulkAction.PermanentlyDelete);
        }
    }
}
