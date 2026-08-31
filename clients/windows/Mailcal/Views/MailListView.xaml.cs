// The mailbox detail, the Windows twin of macOS's mailDetail. Renders the Rust-driven
// row snapshot, with search, a flat/threaded toggle, a per-message context menu
// (reply/forward/read/flag/delete), and a footer (compose/reset/refresh). Every action is
// a dispatched intent; state stays in Rust.

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

    private void OnSearchChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        // Act on typed input AND the built-in clear (X) button, which raises a
        // ProgrammaticChange, otherwise clearing the box leaves search mode stuck on. We
        // never set the text in code apart from that, and show no suggestions.
        if (args.Reason == AutoSuggestionBoxTextChangeReason.SuggestionChosen)
        {
            return;
        }
        var query = sender.Text;
        Model?.Search(query);
        // Re-label the header for the search context, like macOS's "Search results".
        HeaderText.Text = string.IsNullOrEmpty(query) ? (Model?.CurrentFolderName ?? L10n.FolderFallback()) : L10n.SearchResults();
    }

    // The right-clicked row, captured via the menu item's Tag (robust against flyout
    // DataContext quirks), with a DataContext fallback.
    private static MailRow? RowOf(object sender) =>
        (sender as FrameworkElement)?.Tag as MailRow
        ?? (sender as FrameworkElement)?.DataContext as MailRow;

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

    // Opening a message hands the detail column to the reading pane, which, now that the composer
    // lives there instead of in a modal, may be holding an unsent draft. Ask first (the shell knows
    // whether anything has actually been written); "Keep editing" abandons the open. This click was
    // simply impossible while the composer was a ContentDialog, so it is new surface, and losing a
    // draft to it silently was rejected. See MainWindow.Compose.cs.
    //
    // On approval the composer is CLOSED, not merely permitted: leaving it up would open the message
    // behind it, and the click would read as having done nothing at all.
    private async Task<bool> MayOpenMessageAsync()
    {
        if (App.Shell is not { } shell)
        {
            return true;
        }
        if (!await shell.ConfirmDiscardDraftAsync())
        {
            return false;
        }
        shell.CloseComposer();
        return true;
    }

    // Starting a *different* draft also drops the open one, so it asks the same question, but it
    // doesn't close anything: the composer is about to be rebuilt for the new request either way.
    private async Task<bool> MayStartDraftAsync() =>
        App.Shell is not { } shell || await shell.ConfirmDiscardDraftAsync();

    // Tap a row to open it in the reading pane: a flat row opens its message, a conversation
    // row opens its latest message (the model resolves which key to open). The highlight follows
    // from the open message (SyncSelectionToOpenedMessage), so nothing selects a row here, a
    // collapse, which opens nothing, then correctly leaves the highlight where the reader is.
    private async void OnRowClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not MailRow row || !await MayOpenMessageAsync())
        {
            return;
        }
        // A conversation toggles its inline expansion (and opens its latest message in the
        // reading pane); a flat row opens its message. The sub-rows are Buttons, so their taps
        // don't reach here, only the header does.
        if (row.IsThread)
        {
            Model?.ToggleThread(row);
        }
        else
        {
            Model?.OpenMessage(row);
        }
    }

    private async void OnOpen(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row || !await MayOpenMessageAsync())
        {
            return;
        }
        Model?.OpenMessage(row);
    }

    // A tap on a conversation sub-row opens that specific message in the reading pane.
    private async void OnThreadMessageOpen(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is not ThreadMessageItem message
            || !await MayOpenMessageAsync())
        {
            return;
        }
        Model?.OpenThreadMessage(message);
    }

    // Right-click a conversation → Archive conversation (the core archives the received side and
    // leaves any Sent copies in Sent).
    private void OnArchiveThread(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is { } row)
        {
            Model?.ArchiveConversation(row);
        }
    }

    // Reply / reply-all / forward from the row's context menu open the SAME composer as the reading
    // pane's toolbar and the Compose button, the shell derives the To/Cc pre-fill, the From
    // account, and the quoted original, and renders it in the reading-pane slot
    // (MainWindow.Compose.cs). Starting a different draft replaces whatever is already open, so ask
    // about an unsent one first.
    private async void OnReply(object sender, RoutedEventArgs e) => await BeginReplyAsync(sender, replyAll: false);

    private async void OnReplyAll(object sender, RoutedEventArgs e) => await BeginReplyAsync(sender, replyAll: true);

    private async Task BeginReplyAsync(object sender, bool replyAll)
    {
        if (RowOf(sender) is not { } row || !await MayStartDraftAsync())
        {
            return;
        }
        App.Shell?.ComposeReply(row.Account, row.Key, replyAll);
    }

    private async void OnForward(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row || !await MayStartDraftAsync())
        {
            return;
        }
        App.Shell?.ComposeForward(row.Account, row.Key);
    }

    private void OnToggleRead(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is { } row)
        {
            Model?.MarkRead(row.Account, row.Key, row.Unread);
        }
    }

    private void OnToggleFlag(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is { } row)
        {
            Model?.SetFlagged(row.Account, row.Key, !row.Flagged);
        }
    }

    // Archive / Move to Trash from the row's context menu run through the SAME deferred machine as a
    // swipe, hide the row, dispatch nothing, raise the undo bar (MailListView.Swipe.cs). The swipe
    // gesture answers to touch, pen, and a precision touchpad, but never to a mouse, so for a
    // mouse-only user this menu IS the feature; giving it the undo window too is what makes it an
    // equal path rather than a lesser one.
    private void OnArchive(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is { } row)
        {
            PerformSwipe(row, SwipeActionKind.Archive);
        }
    }

    private void OnDelete(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is { } row)
        {
            PerformSwipe(row, SwipeActionKind.Delete);
        }
    }

    private async void OnPermanentlyDelete(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row || Model is null)
        {
            return;
        }
        var result = await DialogHelper.ConfirmAsync(
            this.XamlRoot,
            L10n.DeletePermanentlyTitle(),
            L10n.DeletePermanentlyMessage(),
            L10n.ActionDelete());
        if (result == ContentDialogResult.Primary)
        {
            Model.PermanentlyDelete(row.Account, row.Key);
        }
    }

    private async void OnCompose(object sender, RoutedEventArgs e)
    {
        if (await MayStartDraftAsync())
        {
            App.Shell?.ComposeNew();
        }
    }

    private void OnRefresh(object sender, RoutedEventArgs e) => Model?.Refresh();

    // Re-authenticate the first Microsoft account whose mail write/send is withheld for lack of the
    // Mail.ReadWrite / Mail.Send scopes: re-runs its sign-in (login_hint = its address), re-granting
    // the full scope set. The banner clears once a send/action succeeds; if several are affected it
    // re-renders for the next after each completes.
    private void OnMailReauth(object sender, RoutedEventArgs e)
    {
        if (Model?.MailReauthEmail is { } email)
        {
            Model.SignInWithMicrosoft(email);
        }
    }

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
