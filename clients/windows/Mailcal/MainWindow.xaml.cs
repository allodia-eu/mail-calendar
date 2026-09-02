// The shell window: owns the shared MailboxModel, wires the sidebar and the detail views
// together, and surfaces the zone-changed / connection prompts. The Windows counterpart of the
// macOS ContentView/NavigationSplitView.
//
// The window's other responsibilities are partials, so no one file carries the whole shell:
//   MainWindow.TitleBar.cs   the custom caption (extend, drag region, caption-button theming)
//   MainWindow.Sidebar.cs    the NavigationView accordion (accounts, folders, settings gear)
//   MainWindow.Splitter.cs   the draggable list|reading divider and its persisted width
//   MainWindow.Placement.cs  window icon, saved position/size, DPI scaling, foreground grab
//   MainWindow.Theme.cs      the light/dark appearance the core persists
//   MainWindow.Showcase.cs   the screenshot driver (inert unless ShowcaseMode.IsOn)

using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;

namespace Allodia.Mailcal;

/// <summary>The application's single main window.</summary>
public sealed partial class MainWindow : Window
{
    /// <summary>The shared app model the views bind to.</summary>
    public MailboxModel Model { get; }

    /// <summary>Builds the window, wires the views, and starts the reactive loop.</summary>
    public MainWindow()
    {
        Model = new MailboxModel();
        this.InitializeComponent();
        // Window.Title is still what the taskbar, Alt-Tab and UI Automation read; the TitleBar
        // control only draws the strip. Both come from the same catalog key, so they can't disagree.
        this.Title = L10n.AppTitle();
        // Before the caption is wired, so its buttons are themed from the appearance the app is
        // actually coming up in rather than from the desktop's (MainWindow.Theme.cs).
        ApplyAppearance(AppearanceChoice.AtLaunch);
        InitTitleBar();
        SetWindowIcon();
        // The item only exists once the NavigationView's template has been applied.
        //
        // The saved folder-pane width is restored here too, and not beside RestorePaneWidth below:
        // its clamp is measured against Nav.ActualWidth, which is 0 until the layout pass, so
        // restoring in the constructor silently pins every launch to the minimum width.
        Nav.Loaded += (_, _) =>
        {
            LocalizeSettingsItem();
            RestoreSidebarWidth();
        };
        Nav.SizeChanged += OnNavSizeChanged;

        // Reopen where the user last left the window, and keep saving its placement as it changes
        // / on close so "same size as when we closed it" holds across launches.
        RestoreWindowState();
        RestorePaneWidth();
        this.AppWindow.Changed += OnAppWindowChanged;
        this.Closed += OnClosed;

        Welcome.Init(Model);
        SetupView.Init(Model);
        MailView.Init(Model);
        // The list's search-horizon line asks for the depth setting; the dialog needs this
        // window's XamlRoot, so the control raises and the window opens.
        MailView.SettingsRequested += (_, category) => _ = OpenSettingsAsync(category);
        CalendarDetail.Init(Model);
        ContactsDetail.Init(Model);
        ReadingPanel.Init(Model);

        // The sidebar depends on the account set, the selected account's folders, and the
        // current selection (account / folder / calendar). Each of these signals is a request to
        // RECONCILE, not to rebuild: SyncNavItems diffs the bound accordion and mutates only what
        // moved, so a signal that changes nothing costs nothing. That matters because these arrive
        // in bursts, the model refills Folders with Clear() + one Add() per folder, so opening a
        // 57-folder account raises sixty of them. When each one rebuilt the whole NavigationView
        // by hand, that burst was ten seconds of frozen UI thread (see ViewModels/SidebarTree.cs).
        Model.Accounts.CollectionChanged += (_, _) =>
        {
            SyncNavItems();
            // A mail link that arrived before there was an account to send from is held rather than
            // dropped, so the first account to appear is what finally opens it.
            TryOpenPendingMailLink();
            // The earliest honest moment to offer to become the machine's mail app: the core
            // refuses to offer before an account exists, and this is when one appears.
            OfferDefaultMailAppIfDue();
        };
        Model.Folders.CollectionChanged += (_, _) => SyncNavItems();
        // Connectivity changes re-badge accounts (an unreachable warning). Raised on the UI thread
        // already; the badge is a bound bool, so this only flips a property.
        Model.ConnectivityChanged += SyncNavItems;
        Model.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName is nameof(MailboxModel.SelectedAccount)
                or nameof(MailboxModel.SelectedFolder)
                or nameof(MailboxModel.Destination))
            {
                SyncNavItems();
            }
        };
        SyncNavItems();

        // An assistant asked to open a prefilled draft (docs/mcp.md). The model marshals it onto
        // this thread; the shell owns the composer, so this is where it becomes one. Subscribed
        // before Start(), so a server that comes up listening cannot deliver into a dead handler.
        Model.AgentDraftRequested += ComposeAgentDraft;

        // Arm the screenshot driver before Start(), so no row can arrive before it is listening.
        ShowcaseInit();

        Model.Start();

        // `--calendar` (or /calendar) opens straight on the grid, the shipping equivalent of the
        // DEBUG-only MAILCAL_CALENDAR hook, and what lets a shortcut or a secondary tile drop the user
        // into the calendar. After Start(), so the core is up and the display settings the grid seeds
        // from can be read.
        if (StartupOptions.CalendarAtLaunch)
        {
            ShowCalendarSurface();
        }

        // A cold start FROM a mail link (Program parked it; MainWindow.MailLink.cs opens it). After
        // Start(), so there is an account list to send from, and it stays parked if there is not.
        if (MailLinkInbox.Take() is { } mailLink)
        {
            OpenMailLink(mailLink);
        }

        Log.Info("window launched");
    }

    private void OnAcceptZone(object sender, RoutedEventArgs e) => Model.AcceptTimeZoneChange();

    private void OnDismissZone(object sender, RoutedEventArgs e) => Model.DismissTimeZoneChange();

    // The reply-undelivered prompt. Both buttons carry the tick, so it applies to whichever way the
    // user went, beside "Don't send" it is a standing *no*, which is what stops a server that
    // fails every reply asking again at every meeting. The box is reset here rather than trusted to
    // the bar closing: the InfoBar's content is not re-created when IsOpen goes false and back, so
    // a tick left over from the last meeting would silently apply to the next one.
    private void OnSendUndeliveredReply(object sender, RoutedEventArgs e) => AnswerReply(send: true);

    private void OnDismissUndeliveredReply(object sender, RoutedEventArgs e) => AnswerReply(send: false);

    private void AnswerReply(bool send)
    {
        var remember = RememberReplyChoice.IsChecked == true;
        RememberReplyChoice.IsChecked = false;
        // The subject is composed in the shell, not the core, on the same terms as the RSVP's: the
        // core carries no locale, and this is copy a stranger reads in their inbox.
        var prompt = Model.ReplyPrompt;
        Model.AnswerReplyPrompt(
            send,
            remember,
            prompt is null ? null : InvitationText.ReplySubject(prompt.Response, prompt.Summary));
    }

    // "Save to Sent" on the missing-copy bar: files the copy of a message that already went out.
    // It sends nothing, and the core ignores a second press while one is in flight.
    private void OnRetryUnfiledCopy(object sender, RoutedEventArgs e) => Model.RetryUnfiledCopy();

    // "Not now": the user has read it and accepts the missing copy. The message stays sent.
    private void OnDismissUnfiledCopy(object sender, RoutedEventArgs e) => Model.DismissUnfiledCopy();

    // "Try again" on the connection banner: a refresh re-dials any disconnected account (the
    // bindings retry the outaged accounts on a RefreshMail) and catches up whatever arrived.
    private void OnRetryConnections(object sender, RoutedEventArgs e) => Model.Refresh();
}
