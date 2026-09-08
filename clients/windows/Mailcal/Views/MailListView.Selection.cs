// The message list's multi-selection: reading what the ListView has picked, the two keys bound on
// it, and what the actions bar calls in. Split from MailListView.xaml.cs to keep each file under
// the 500-line limit.
//
// The ListView owns the selection (docs/list-selection.md, rule 1). What this file adds is the
// place WinUI's defaults are not what a mailbox wants (a modified click must not also open a
// message), and the entry points the bar uses: the bar spans both panes, so it is declared in
// MainWindow.xaml and reaches the selection through here.

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

    /// <summary>
    /// Picks every loaded row, which is the window the list is showing rather than the whole
    /// folder (docs/list-selection.md, rule 10). The bar's own affordance for it, since Ctrl+A
    /// belongs to whatever has focus.
    /// </summary>
    internal void SelectAllRows() => RowsList.SelectAll();

    /// <summary>Drops the selection, which the bar's cross and Escape both do.</summary>
    internal void ClearRowSelection() => RowsList.SelectedItems.Clear();

    /// <summary>Runs one action over the selection as a single batch in the core.</summary>
    internal void ActOnSelection(BulkAction action) => Model?.ActOnSelection(action);

    /// <summary>
    /// The read or flag toggle, whichever the button currently says, so a click runs the action
    /// the user read on it.
    /// </summary>
    internal void ToggleSelectionRead() => Model?.ActOnSelection(Model.SelectionReadAction);

    internal void ToggleSelectionFlag() => Model?.ActOnSelection(Model.SelectionFlagAction);

    /// <summary>
    /// The one irreversible action on the bar, so it asks first, exactly as the row menu's does,
    /// and names the count because that is what is actually going.
    /// </summary>
    internal async Task PermanentlyDeleteSelectionAsync()
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
