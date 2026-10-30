// The mailbox detail, the Windows twin of macOS's mailDetail. Renders the Rust-driven row
// snapshot, with the row menu the core decides (MailListView.RowMenu.cs) and a status-line footer.
// Every action is a dispatched intent; state stays in Rust.
//
// The search field is NOT here: it is the window's own, in the title bar (MainWindow.Search.cs),
// because its scope is every account and every folder rather than this one column.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>The mailbox-list detail view.</summary>
public sealed partial class MailListView : UserControl
{
    /// <summary>The shared app model (set by the host via <see cref="Init"/>).</summary>
    public MailboxModel? Model { get; private set; }

    /// <summary>Initialises the control.</summary>
    public MailListView() => this.InitializeComponent();

    /// <summary>Binds the view to the shared model.</summary>
    public void Init(MailboxModel model)
    {
        Model = model;
        this.Bindings.Update();
        InitSwipe();
        InitReadingWindows();
        // The highlight follows the READING PANE, not the last click: archive/delete advances the
        // pane to the next message on its own (ReadingAdvance), and nothing clicked that row. The
        // row set is re-run too, because a snapshot reconcile replaces changed rows outright, the
        // selected instance is then no longer in the collection and the highlight silently drops.
        model.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(MailboxModel.OpenedMessage))
            {
                SyncSelectionToOpenedMessage();
            }
        };
        model.Rows.CollectionChanged += (_, _) => SyncSelectionToOpenedMessage();
        SyncSelectionToOpenedMessage();
    }

    // Highlight the row standing for the open message (ReadingSelection), or clear the highlight
    // when the pane is empty or the message is no longer in the list. Idempotent, so the click
    // handlers don't need to select anything themselves, opening is what selects.
    private void SyncSelectionToOpenedMessage()
    {
        if (Model is not { } model)
        {
            return;
        }
        // Never over a multi-selection. This runs on every snapshot reconcile, so a background
        // sync arriving mid-selection would otherwise collapse the set the user is building down
        // to the one message the reading pane happens to hold.
        if (RowsList.SelectedItems.Count > 1)
        {
            return;
        }
        MailRow? row = null;
        if (model.OpenedMessage is { } opened)
        {
            var stops = model.Rows
                .Select(r => new RowStop(
                    r.Account, r.LatestKey, r.Messages.Select(m => m.Key).ToList()))
                .ToList();
            if (ReadingSelection.RowOf(opened.Account, opened.Key, stops) is { } index)
            {
                row = model.Rows[index];
            }
        }
        if (!ReferenceEquals(RowsList.SelectedItem, row))
        {
            RowsList.SelectedItem = row;
        }
    }

    // Infinite scroll: the core ships the list one page at a time (the visible window). When
    // the user scrolls within a viewport of the end, ask the core for the next page; it grows
    // the window and re-projects, the reconcile appends the new rows in place (scroll position
    // kept). Hook the ListView's inner ScrollViewer once it's templated.
    private ScrollViewer? _scrollViewer;

    private void OnRowsListLoaded(object sender, RoutedEventArgs e)
    {
        _scrollViewer ??= FindScrollViewer(RowsList);
        if (_scrollViewer is not null)
        {
            _scrollViewer.ViewChanged -= OnRowsScrolled;
            _scrollViewer.ViewChanged += OnRowsScrolled;
        }
    }

    private void OnRowsScrolled(object? sender, ScrollViewerViewChangedEventArgs e)
    {
        if (_scrollViewer is null)
        {
            return;
        }
        // Prefetch when within one viewport of the bottom, so the next page is ready before
        // the user reaches the end. The model coalesces the burst and no-ops when nothing's left.
        var remaining = _scrollViewer.ScrollableHeight - _scrollViewer.VerticalOffset;
        if (remaining <= _scrollViewer.ViewportHeight)
        {
            Model?.ShowMore();
        }
    }

    private static ScrollViewer? FindScrollViewer(DependencyObject root)
    {
        if (root is ScrollViewer scrollViewer)
        {
            return scrollViewer;
        }
        var count = VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            if (FindScrollViewer(VisualTreeHelper.GetChild(root, i)) is { } found)
            {
                return found;
            }
        }
        return null;
    }

    // Opening a message hands the detail column to the reading pane, which may be holding a
    // composer. Leaving it keeps what was written there in Drafts and asks nothing
    // (docs/drafts.md, "Leaving a composer"); see MainWindow.Compose.cs.
    //
    // The composer is then CLOSED: leaving it up would open the message behind it, and the click
    // would read as having done nothing at all.
    private async Task LeaveComposerForMessageAsync()
    {
        if (App.Shell is not { } shell)
        {
            return;
        }
        await shell.LeaveComposerAsync();
        shell.CloseComposer();
    }

    // Starting a *different* draft leaves the open one too, but closes nothing: the composer is
    // about to be rebuilt for the new request either way.
    private static async Task LeaveComposerForDraftAsync()
    {
        if (App.Shell is { } shell)
        {
            await shell.LeaveComposerAsync();
        }
    }

    // Tap a row to open it in the reading pane: a flat row opens its message, a conversation
    // row its newest (the model resolves which). The highlight follows
    // from the open message (SyncSelectionToOpenedMessage), so nothing selects a row here, a
    // collapse, which opens nothing, then correctly leaves the highlight where the reader is.
    private async void OnRowClick(object sender, ItemClickEventArgs e)
    {
        // A Ctrl- or Shift-click is aimed at the selection, and the ListView has already applied
        // it. Opening as well would fetch and display a body for every row added to a
        // twenty-row selection (docs/list-selection.md). The pane is still recorded: a
        // double-click on a row that was reached that way restores from the same record, and a
        // record left over from an earlier click would put the pane back to a message the reader
        // has since moved on from (MailListView.ReadingWindows.cs).
        if (e.ClickedItem is not MailRow row)
        {
            return;
        }
        if (SelectionModifierDown)
        {
            RecordPane();
            return;
        }
        // The second click of a double-click on a MESSAGE opens nothing here: it means a window
        // (MailListView.ReadingWindows.cs), and the pane is meant to be left alone. This also
        // records what the pane is showing, so that window can put it back. A conversation header
        // is not a message and gets no window, so its second click still toggles the thread, which
        // is what a double-click has always done there.
        if (!row.IsThread && !ClickOpensInPane(row.Id))
        {
            return;
        }
        await LeaveComposerForMessageAsync();
        // A conversation toggles its inline expansion (and opens its newest message in the
        // reading pane); a flat row opens its message. The sub-rows are Buttons, so their taps
        // don't reach here, only the header does.
        if (row.IsThread)
        {
            await ToggleThreadAsync(row);
        }
        else
        {
            await OpenOrResumeAsync(row);
        }
    }

    // A conversation opens its newest message, or in Drafts its newest draft, and a draft opens
    // back into its composer as a flat row's does (docs/drafts.md, docs/row-actions.md rule 2).
    private async Task ToggleThreadAsync(MailRow row)
    {
        if (Model is not { } model)
        {
            return;
        }
        if (row.IsExpanded || App.Shell is not { } shell || model.Representative(row) is not { } message)
        {
            model.ToggleThread(row);
            return;
        }
        model.ToggleThread(row, openLatest: false);
        await shell.OpenOrResumeAsync(
            message.Account, message.Key, message.Draft, () => model.OpenThreadMessage(message));
    }

    // A draft opens the composer it was written in, wherever it is listed; every other message
    // opens the reading view (docs/drafts.md). The shell decides, because it is the shell that
    // owns the composer the draft goes into.
    private async Task OpenOrResumeAsync(MailRow row)
    {
        if (Model is not { } model)
        {
            return;
        }
        if (App.Shell is not { } shell)
        {
            model.OpenMessage(row);
            return;
        }
        await shell.OpenOrResumeAsync(row.Account, row.Key, row.Draft, () => model.OpenMessage(row));
    }

    // A tap on a conversation sub-row opens that specific message in the reading pane. On the same
    // terms as a flat row: the second click of a double-click opens a window instead, so it opens
    // nothing here.
    private async void OnThreadMessageOpen(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is not ThreadMessageItem message
            || !ClickOpensInPane($"{message.Account}/{message.Key}"))
        {
            return;
        }
        await LeaveComposerForMessageAsync();
        await OpenThreadMessageAsync(message);
    }

    private async Task OpenThreadMessageAsync(ThreadMessageItem message)
    {
        if (Model is { } model && App.Shell is { } shell)
        {
            await shell.OpenOrResumeAsync(
                message.Account, message.Key, message.Draft, () => model.OpenThreadMessage(message));
        }
    }

    // The connection flyout's "Try again": a re-dial of the current scope, which is the same
    // dispatch the actions bar's Sync makes.
    private void OnRefresh(object sender, RoutedEventArgs e) => Model?.Refresh();

    // Signs the first account whose mail writes the grant withholds in again, in place. The banner
    // clears once a send or action succeeds; if several are affected it re-renders for the next
    // after each completes.
    private void OnMailReauth(object sender, RoutedEventArgs e) => Model?.ReconsentMail();

    /// <summary>
    /// The horizon line's "Change" link: opens Settings on the Accounts category, where the
    /// per-account sync depth lives. Raised as an event rather than opened here, the dialog needs
    /// the window's <c>XamlRoot</c>, which a UserControl has no business reaching for.
    /// </summary>
    private void OnChangeSyncDepth(object sender, RoutedEventArgs e) =>
        SettingsRequested?.Invoke(this, "accounts");

    /// <summary>Raised when something in the list asks for a Settings category by tag.</summary>
    public event EventHandler<string>? SettingsRequested;

    // Sign the first account with a dead grant back in. The model picks the flow from that
    // account's provider; the button is hidden for a password/JMAP account (there is no browser
    // flow to run), whose message points at Settings instead.
    private void OnSignInExpired(object sender, RoutedEventArgs e) => Model?.ReconnectExpiredSignIn();
}
