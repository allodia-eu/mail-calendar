// The process entry point. We take over Main (DISABLE_XAML_GENERATED_MAIN in the .csproj) for one
// reason: a browser sign-in returns through a custom-scheme redirect (<scheme>://auth for
// Microsoft 365, <scheme>://jmap-oauth for JMAP, where <scheme> is eu.allodia.mailcal for the
// packaged Store build and eu.allodia.mailcal.dev for an unpackaged dev one, so the two can coexist
// on a developer's machine; see MicrosoftOAuthConfig), and the OS delivers that by activating this app,
// launching a FRESH instance if one isn't already the registered target. WinUI 3 isn't
// single-instanced by default, so without this the callback would land in a second process that
// knows nothing about the sign-in in flight. We rendezvous every launch on a single-instance key,
// redirect any secondary activation into the already-running instance, and hand the redirect URI to
// the waiting OAuth flow (ProtocolAuthCallback, which matches it to the flow that armed it by the
// redirect's host). A normal launch falls straight through to Application.Start.
//
// The single-instancing + redirect boilerplate mirrors the Microsoft AppLifecycle sample; the
// OAuth delivery and the unpackaged-dev protocol registration are ours.

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Win32;
using Microsoft.Windows.AppLifecycle;
using Microsoft.Windows.AppNotifications;
using Windows.ApplicationModel.Activation;
using Windows.ApplicationModel.DataTransfer.ShareTarget;

namespace Allodia.Mailcal;

/// <summary>The process entry point: single-instances the app and routes the OAuth redirect.</summary>
public static class Program
{
    // The app-defined key every launch rendezvouses on, so the second (redirect) process finds the
    // first. Derived from the build's own scheme rather than hard-coded, so a dev build and an
    // installed Store build rendezvous on DIFFERENT keys: sharing one would let a dev launch
    // redirect its activation into the Store app (or vice versa), which is the same
    // wrong-instance hazard the separate scheme exists to remove, one layer down.
    private static string SingleInstanceKey => MicrosoftOAuthConfig.Scheme + ".single-instance";

    [STAThread]
    private static void Main()
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();

        // Before anything can create a web view: a release build reads none of the WebView2 loader's
        // variables, one of which opens a remote-debugging port on every page (WebViewInspection).
        var closedVariables = WebViewInspection.Close(WebViewInspection.DevToolsEnabled);

        // Unpackaged dev (build-and-run.ps1) has no MSIX manifest to declare the protocol, so
        // register it against the current exe at runtime, and withdraw every other dev build's
        // claim on it. Packaged/Store builds get it from Package.appxmanifest instead, skip there.
        RegisterProtocolForUnpackaged();

        // Above the line below, and that is the whole reason it is here rather than in the window.
        // Reading the activation arguments of a launch that IS a notification click means building
        // an AppNotificationActivatedEventArgs, which the runtime fails fast on, taking the process
        // with it, unless the notification platform is registered first (Services/NewMailNotifier.cs).
        // What the click then DOES is installed later, once there is a window (App.OnLaunched).
        NewMailNotifier.Arm();

        // A secondary activation (e.g. the OAuth redirect) is handed to the primary instance, which
        // then exits; only the primary starts the UI.
        var activation = AppInstance.GetCurrent().GetActivatedEventArgs();
        if (RedirectedToPrimaryInstance(activation))
        {
            return;
        }

        // The log opens HERE, not where the core is built, and the ordering is the whole point.
        // Log.Write silently drops everything until the sink has a path, so a crash handler armed
        // before Init writes nothing, and XAML init, which is exactly where a stowed WinUI
        // exception has taken this app down, is inside Application.Start below. Both lines have to
        // be above it.
        //
        // Deliberately after the redirect check: a secondary instance exists only to hand its
        // activation over and exit, and a session marker from it would read as a launch that never
        // happened.
        Log.Init(AppPaths.Root, AppIdentity.PackageVersion);
        // Now that there is somewhere to write it: how the registration above went, which had no
        // log to report into at the point it ran.
        NewMailNotifier.LogRegistration();
        CrashLog.WatchProcess();
        if (_devSchemeNote is { } note)
        {
            Log.Info(note);
        }
        if (closedVariables > 0)
        {
            Log.Info($"web views: ignored {closedVariables} WebView2 environment variable(s), as a release build does");
        }
        if (Log.FilePath is string logPath)
        {
            CrashLog.WatchForNativeFaults(logPath);
        }

        // What earlier sessions' composers staged. Only the primary instance, and only what was
        // made before this line: a composer opens once the window does, and its directory is newer.
        var launched = DateTime.UtcNow;
        _ = Task.Run(() =>
        {
            if (ComposerStaging.Sweep(launched) is var cleared and > 0)
            {
                Log.Info($"composer: deleted the files {cleared} earlier composer(s) staged");
            }
        });

        // A cold start FROM a mail link. Held rather than acted on: there is no window yet, and no
        // account list to send from, so MainWindow drains it once the core is up (MainWindow.MailLink.cs).
        MailLinkInbox.Pending = MailLinkFrom(activation) ?? MailLink.FromArguments(Environment.GetCommandLineArgs());

        // A cold start FROM a click on a new-mail notification. Held for the same reason as the
        // link above: there is no window to open the message in and no core to find it with, so
        // the shell drains it once both exist (Services/NotificationOpenInbox.cs).
        NotificationOpenInbox.Pending = NotifiedMessageFrom(activation);

        // A cold start FROM a share. Read here rather than held as the activation is, because a
        // ShareOperation's access to what was shared ends when the operation reports completion,
        // which reading it does: by the time the window drains this, the bytes are already staged
        // and the paths are ones this app can still open (Services/ShareIntake.cs).
        if (ShareOperationFrom(activation) is { } sharing)
        {
            ShareInbox.Pending = ShareIntake.ReadAsync(sharing).GetAwaiter().GetResult();
        }

        Application.Start(p =>
        {
            var context = new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            _ = new App();
        });
    }

    // Rendezvous on the single-instance key. The primary keeps running and subscribes to future
    // activations; a secondary redirects its activation to the primary and returns true (exit).
    private static bool RedirectedToPrimaryInstance(AppActivationArguments activation)
    {
        var primary = AppInstance.FindOrRegisterForKey(SingleInstanceKey);
        if (primary.IsCurrent)
        {
            primary.Activated += (_, e) => OnActivated(e);
            return false;
        }
        RedirectActivationTo(activation, primary);
        return true;
    }

    // A redirected protocol activation carries the OAuth redirect URI
    // (eu.allodia.mailcal://auth?code=...&state=..., or //jmap-oauth for a JMAP sign-in); pass it
    // to the waiting sign-in and bring the app forward. Fires on a background thread,
    // ProtocolAuthCallback is thread-safe, and the window touch is marshaled onto its dispatcher.
    private static void OnActivated(AppActivationArguments activation)
    {
        // A second launch of an already-running app, a shortcut, a tile, `Mailcal.exe --calendar`
        // typed again, is redirected here rather than starting a new process. Honour `--calendar` so
        // a "Calendar" shortcut works whether or not the app was already open, and bring us forward.
        if (activation.Kind == ExtendedActivationKind.Launch
            && activation.Data is ILaunchActivatedEventArgs launch
            && StartupOptions.WantsCalendar(launch.Arguments)
            && App.MainWindow is MainWindow calendarWindow)
        {
            Log.Info("launch activation: --calendar -> showing calendar");
            calendarWindow.DispatcherQueue.TryEnqueue(() =>
            {
                calendarWindow.ShowCalendarSurface();
                calendarWindow.BringToForeground();
            });
            return;
        }

        // A click on a notification while the app is already running. It usually does NOT come
        // this way: the notification COM server is registered in this process, so the shell
        // activates it directly and NewMailNotifier's own handler answers. This covers the window
        // where it is not registered yet, a click on a previous run's notification during startup,
        // which launches a second process that is redirected here instead.
        if (activation.Kind == ExtendedActivationKind.AppNotification)
        {
            // On the KIND, not on the message: a click on the overflow summary names none, and
            // that click still has to bring the app forward.
            var notified = NotifiedMessageFrom(activation);
            if (App.Shell is MainWindow notifiedWindow)
            {
                notifiedWindow.DispatcherQueue.TryEnqueue(
                    () => notifiedWindow.OpenNotification(notified));
            }
            else
            {
                NotificationOpenInbox.Pending = notified;
            }
            return;
        }

        // A share while the app is already running. Ahead of the mail-link and OAuth branches
        // because it is decided on the activation KIND, which neither of those can be: they are
        // both protocol activations and are told apart by their scheme.
        if (ShareOperationFrom(activation) is { } sharing)
        {
            _ = HandleShareAsync(sharing);
            return;
        }

        // A mail link clicked while the app is already running, a browser hands it here rather
        // than starting a second process. Checked before the OAuth delivery below, and gated on the
        // SCHEME: both arrive as protocol activations, and mistaking one for the other would either
        // swallow a sign-in or open a composer over it.
        if (MailLinkFrom(activation) is { } link)
        {
            // Parked rather than dropped when the window is not reachable yet: an activation can
            // land during startup, and the OAuth delivery below would otherwise be handed a
            // `mailto:` URI it can match to nothing, a click that silently did nothing.
            if (App.Shell is MainWindow linkWindow)
            {
                linkWindow.DispatcherQueue.TryEnqueue(() => linkWindow.OpenMailLink(link));
            }
            else
            {
                MailLinkInbox.Pending = link;
            }
            return;
        }

        if (activation.Kind != ExtendedActivationKind.Protocol
            || activation.Data is not IProtocolActivatedEventArgs protocol)
        {
            return;
        }
        // Log arrival only, never the query, which carries the one-time auth code.
        Log.Info($"protocol activation received ({protocol.Uri.Scheme})");
        ProtocolAuthCallback.Deliver(protocol.Uri);
        // Raise the app to the foreground after the browser hand-off, the default browser (unlike
        // macOS/Android's session APIs) can't dismiss its own tab, so at least bring us forward.
        if (App.MainWindow is MainWindow window)
        {
            window.DispatcherQueue.TryEnqueue(window.BringToForeground);
        }
    }

    // The mail link an activation carries, or null when it carries none, which is the common case
    // (every OAuth redirect is a protocol activation too). Two shapes reach us: the packaged build
    // is activated through its manifest declaration and gets a `mailto:` URI, and a classic
    // registration invokes the exe with the link as an argument.
    //
    // The URI itself is opaque here and is never logged, it is message content end to end
    // (recipients, subject, body). The shared core parses it (docs/composer-security.md, Gate 12).
    private static string? MailLinkFrom(AppActivationArguments activation) => activation switch
    {
        { Kind: ExtendedActivationKind.Protocol, Data: IProtocolActivatedEventArgs protocol }
            // OriginalString, not ToString(): the URI must reach the core with its percent-encoding
            // intact. Canonicalizing it can unescape a `%26`, and the core's defence against a
            // subject smuggling in an extra `bcc` is that it splits the query into fields BEFORE
            // decoding, which only holds if what it was handed is still encoded.
            when MailLink.CarriesMailLink(protocol.Uri.Scheme) => protocol.Uri.OriginalString,
        { Kind: ExtendedActivationKind.Launch, Data: ILaunchActivatedEventArgs launch }
            => MailLink.FromArgumentLine(launch.Arguments),
        _ => null,
    };

    // The message a clicked new-mail notification named, or null when this activation is not one
    // (or is a click on the overflow summary, which names no message: that one only has to bring
    // the app forward, and being activated at all does exactly that).
    //
    // The arguments are the toast's own, put there by NewMailNotifier, so nothing a sender wrote
    // reaches here: an account id and a provider key, both of which this app minted.
    private static NotificationTarget? NotifiedMessageFrom(AppActivationArguments activation) =>
        activation.Kind == ExtendedActivationKind.AppNotification
            && activation.Data is AppNotificationActivatedEventArgs notification
            ? NotificationTarget.From(notification.Arguments)
            : null;

    // The share operation an activation carries, or null when it is not a share. Only a packaged
    // build is ever activated this way: `windows.shareTarget` is an MSIX manifest extension, so
    // the unpackaged dev loop never appears in the share sheet at all.
    private static ShareOperation? ShareOperationFrom(AppActivationArguments activation) =>
        activation.Kind == ExtendedActivationKind.ShareTarget
            && activation.Data is IShareTargetActivatedEventArgs share
            ? share.ShareOperation
            : null;

    // Reads a share that arrived at a running app and hands it to the window, or parks it when the
    // window is not reachable yet. The read is awaited off the activation callback: it copies the
    // shared bytes, and Windows keeps the SHARING app's UI blocked until the operation completes,
    // so doing it inline would freeze that app rather than this one.
    private static async Task HandleShareAsync(ShareOperation sharing)
    {
        var prefill = await ShareIntake.ReadAsync(sharing);
        if (prefill is null)
        {
            return;
        }
        if (App.Shell is MainWindow shareWindow)
        {
            shareWindow.DispatcherQueue.TryEnqueue(() => shareWindow.OpenShare(prefill));
        }
        else
        {
            ShareInbox.Pending = prefill;
        }
    }

    private static void RegisterProtocolForUnpackaged()
    {
        if (AppIdentity.IsPackaged)
        {
            return;
        }
        // MicrosoftOAuthConfig.Scheme resolves to the DEV scheme here, being unpackaged is exactly
        // what makes it so, which is the point: a developer's machine also has the Store build
        // installed, and if both registered the same scheme Windows could only ask the user which
        // app should receive a redirect carrying a live auth code. The scheme MUST equal the one in
        // MicrosoftOAuthConfig.RedirectUri (and the Azure registration, which lists both forms); it
        // is this build's single scheme, so registering it covers every sign-in that redirects to
        // it, JMAP included. Pass the real exe path explicitly: an empty exePath leaves the
        // handler's shell\open\command unwritten (the OS then can't launch us on activation).
        var exePath = Environment.ProcessPath ?? string.Empty;
        WithdrawOtherDevBuilds(exePath);
        ActivationRegistrationManager.RegisterForProtocolActivation(
            MicrosoftOAuthConfig.Scheme, string.Empty, "Allodia Mail & Calendar (dev)", exePath);
    }

    // Every checkout, worktree and architecture registers the dev scheme against its own exe, and
    // a redirect with more than one handler opens a "select an app" picker in which only the
    // running build can finish the sign-in (DevSchemeOwnership). So the build that launches last
    // takes the scheme. The registrations are read where the Windows App SDK writes them: under
    // WindowsAppRuntimeApplications, in a key the SDK itself spells "Capabilties", mapping the
    // scheme to a ProgID whose shell\open\command names the exe. A failure here costs at most a
    // picker, so it never stops the launch. What happened is held for the log, which this runs
    // too early to write to (Main).
    private static void WithdrawOtherDevBuilds(string exePath)
    {
        try
        {
            var scheme = MicrosoftOAuthConfig.Scheme;
            var others = DevSchemeOwnership.OthersToWithdraw(RegisteredDevSchemeExes(scheme), exePath);
            foreach (var other in others)
            {
                ActivationRegistrationManager.UnregisterForProtocolActivation(scheme, other);
            }
            if (others.Count > 0)
            {
                _devSchemeNote = $"dev scheme: withdrew {others.Count} other build(s)' handler";
            }
        }
        catch (Exception error)
        {
            _devSchemeNote = $"dev scheme: could not withdraw other builds' handlers: {error.GetType().Name}";
        }
    }

    private static string? _devSchemeNote;

    private static IEnumerable<string?> RegisteredDevSchemeExes(string scheme)
    {
        using var apps = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\WindowsAppRuntimeApplications");
        if (apps is null)
        {
            yield break;
        }
        foreach (var app in apps.GetSubKeyNames())
        {
            using var urls = apps.OpenSubKey($@"{app}\Capabilties\URLAssociations");
            if (urls?.GetValue(scheme) is not string progId)
            {
                continue;
            }
            using var command = Registry.CurrentUser.OpenSubKey($@"Software\Classes\{progId}\Shell\open\command");
            yield return DevSchemeOwnership.ExeFromCommand(command?.GetValue(null) as string);
        }
    }

    // Redirect on a worker thread and pump COM messages while waiting, so the STA main thread never
    // deadlocks on the async redirect. Verbatim from the Microsoft single-instancing sample.
    private static void RedirectActivationTo(AppActivationArguments activation, AppInstance primary)
    {
        var completed = CreateEvent(IntPtr.Zero, true, false, null);
        Task.Run(() =>
        {
            primary.RedirectActivationToAsync(activation).AsTask().Wait();
            SetEvent(completed);
        });
        const uint CWMO_DEFAULT = 0;
        const uint INFINITE = 0xFFFFFFFF;
        _ = CoWaitForMultipleObjects(CWMO_DEFAULT, INFINITE, 1, new[] { completed }, out _);
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr CreateEvent(IntPtr attributes, bool manualReset, bool initialState, string? name);

    [DllImport("kernel32.dll")]
    private static extern bool SetEvent(IntPtr handle);

    [DllImport("ole32.dll")]
    private static extern uint CoWaitForMultipleObjects(uint flags, uint milliseconds, ulong count, IntPtr[] handles, out uint index);
}
