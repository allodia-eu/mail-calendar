// Dragging messages onto a folder in the pane (docs/folder-pane.md, rule 24). The list starts the
// drag; MainWindow.Folders.cs decides where it lands and dispatches the move.
//
// A row that is part of the selection drags the whole selection, which is the ListView's own
// behaviour and what rule 24 asks: a selection drags as one. A conversation row travels as its
// thread, as the selection bar's batch does, so the core moves every message it stands for.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;

namespace Allodia.Mailcal.Views;

public sealed partial class MailListView
{
    private void OnRowsDragItemsStarting(object sender, DragItemsStartingEventArgs e)
    {
        var rows = e.Items.OfType<MailRow>().ToArray();
        if (rows.Length == 0)
        {
            e.Cancel = true;
            return;
        }
        FolderActions.Dragged = new MessageDrag(SelectionActions.Rows(rows));
        // The payload stays in process; the package carries only the format a drop target checks.
        e.Data.SetData(FolderActions.DragFormat, "messages");
        e.Data.RequestedOperation = DataPackageOperation.Move;
    }

    private void OnRowsDragItemsCompleted(ListViewBase sender, DragItemsCompletedEventArgs args) =>
        FolderActions.Dragged = null;
}
