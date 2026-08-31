// The WinUI application entry point: creates the single main window. The Windows App SDK
// targets generate the process Main (and, for this unpackaged app, the runtime bootstrap),
// so this just opens MainWindow, the shell that owns the Rust-driven MailboxModel.

using Microsoft.UI.Xaml;

namespace Allodia.Mailcal;

/// <summary>The app object; opens the main window on launch.</summary>
public partial class App : Application
{
    private Window? _window;
    internal static Window? MainWindow { get; private set; }

    /// <summary>The shell, typed, the detail views reach it to open the composer in the reading-pane
    /// slot (MainWindow.Compose.cs) and to ask about an unsent draft before they replace it.
    /// <see cref="MainWindow"/> stays a plain <see cref="Window"/> because the window-handle interop
    /// (file pickers, foreground) only ever needs that.</summary>
    internal static MainWindow? Shell => MainWindow as MainWindow;

    /// <summary>Initialises the XAML resources, re-applying the persisted language override.</summary>
    public App()
    {
        // The language override lives in an in-memory MRT-Core ResourceContext that resets each
        // launch, so the app re-applies the stored choice here, before MainWindow (and any
        // resource lookup) is created in OnLaunched, so the right .resw language is selected from
        // the start. A screenshot run (MAILCAL_SHOWCASE=en|nl) pins the language for this session
        // instead, without persisting it, so one launch yields a fully localised store screenshot.
        Services.LanguageStore.Apply(
            Services.ShowcaseMode.LanguageOverride ?? Services.LanguageStore.Read());
        this.InitializeComponent();
        WatchForCrashes();
    }

    /// <summary>
    /// Wires WinUI's own <see cref="UnhandledException"/>, the XAML-thread half of the crash log
    /// (<see cref="Services.CrashLog"/>), which needs the Application object and so cannot be armed
    /// beside the other handlers.
    /// </summary>
    /// <remarks>
    /// The CLR's domain handler covers everything else and the two do not overlap, so both are
    /// wired; that one and the unobserved-task handler are armed in <c>Program.Main</c>, where they
    /// are already in place before this constructor runs, and, more to the point, after
    /// <c>Log.Init</c>, which is what gives any of them somewhere to write.
    /// Not marked handled: the process is meant to still fail, and this only makes it say why.
    /// </remarks>
    private void WatchForCrashes()
    {
        this.UnhandledException += (_, e) =>
            Services.Log.Error(Services.CrashLog.Record("on the XAML thread", e.Exception));
    }

    /// <summary>Creates and activates the main window.</summary>
    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _window = new MainWindow();
        MainWindow = _window;
        _window.Activate();
        // A mail link that arrived mid-startup: the window drains the inbox as it is built, and
        // Program parks a link there whenever the shell is not reachable yet, so the two can cross
        //, the window taking an empty inbox a moment before the link lands in it. Draining once
        // more here, with the shell now reachable, is what stops that click doing nothing.
        if (Services.MailLinkInbox.Take() is { } link)
        {
            Shell?.OpenMailLink(link);
        }
    }
}
