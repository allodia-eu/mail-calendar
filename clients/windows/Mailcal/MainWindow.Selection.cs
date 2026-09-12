// The mail actions bar's handlers. The bar spans the whole window, so it is declared in
// MainWindow.xaml rather than inside the list control; the selection it acts on is still the
// list's own (docs/list-selection.md, rule 1), which is what most of these delegate to. The two
// that name no selection, New Mail and Sync, act on the window and the mailbox instead.

using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    /// <summary>
    /// New Mail: the first button on the bar, and the first of the two that name no selection.
    /// </summary>
    /// <remarks>
    /// Mail first: the composer takes the reading pane's place, and that pane exists only on the
    /// mail surface, so writing from the calendar has to bring the mailbox back with it or the
    /// draft would open behind the grid. The draft already open is asked about first, for the
    /// reason every other route into the composer asks.
    /// </remarks>
    private async void OnNewMail(object sender, RoutedEventArgs e)
    {
        if (await ConfirmDiscardDraftAsync())
        {
            Model.ShowMail();
            ComposeNew();
        }
    }

    private void OnSelectionToggleRead(object sender, RoutedEventArgs e) =>
        MailView.ToggleSelectionRead();

    private void OnSelectionToggleFlag(object sender, RoutedEventArgs e) =>
        MailView.ToggleSelectionFlag();

    private void OnSelectionArchive(object sender, RoutedEventArgs e) =>
        MailView.ActOnSelection(BulkAction.Archive);

    private void OnSelectionDelete(object sender, RoutedEventArgs e) =>
        MailView.ActOnSelection(BulkAction.Delete);

    private async void OnSelectionPermanentlyDelete(object sender, RoutedEventArgs e) =>
        await MailView.PermanentlyDeleteSelectionAsync();

    private void OnSelectAll(object sender, RoutedEventArgs e) => MailView.SelectAllRows();

    private void OnClearSelection(object sender, RoutedEventArgs e) => MailView.ClearRowSelection();

    /// <summary>
    /// Sync: the one button on this bar that is not a selection action.
    /// </summary>
    /// <remarks>
    /// It acts on the mailbox rather than on what is picked, so it is never disabled and it sits
    /// past a divider (docs/list-selection.md, rule 5). Here rather than in a footer because this
    /// is where a user reaches for it, beside Archive and Delete.
    /// </remarks>
    private void OnSelectionSync(object sender, RoutedEventArgs e) => Model.Refresh();
}
