// The mail actions bar's handlers. The bar spans the message list and the reading pane, so it is
// declared in MainWindow.xaml rather than inside the list control; the selection it acts on is
// still the list's own (docs/list-selection.md, rule 1), which is what these delegate to.

using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
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
}
