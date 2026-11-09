// The message list's multi-selection: reading what the ListView has picked and the two keys bound
// on it. Split from MailListView.xaml.cs to keep each file under the 500-line limit.
//
// The ListView owns the selection (docs/list-selection.md, rule 1). What this file adds is the
// place WinUI's defaults are not what a mailbox wants (a modified click must not also open a
// message). The command bar over the selection is MainWindow's, and runs through the row menu's
// path (MailListView.RowMenu.cs).

using Allodia.Mailcal.Services;
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

    // Delete (and Backspace) on the list is the bar's Delete over the selection: a move to Trash,
    // which asks nothing, and in Trash a delete for good, which asks first (docs/command-bar.md,
    // rule 6).
    private async void OnDeleteSelection(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        if (Model is { } model && CommandBarPresentation.Delete(model.Bar) is { } delete)
        {
            await RunCommandAsync(delete);
        }
    }

    private void OnClearSelection(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        RowsList.SelectedItems.Clear();
    }
}
