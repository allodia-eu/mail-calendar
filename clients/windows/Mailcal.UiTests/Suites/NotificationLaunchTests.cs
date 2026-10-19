// A launch that is a click on a new-mail notification survives its own startup
// (docs/background-sync.md, docs/client-traps.md).
//
// Reading such a launch's activation builds an AppNotificationActivatedEventArgs, which the Windows
// App SDK cannot do unless AppNotificationManager.Register() has already run in this process.
// Unregistered, it does not throw: it fails fast, 0xc0000409 inside the runtime, before Log.Init,
// so the app vanishes on the one launch the feature exists for and leaves an empty log.
// check-notification-registration pins the order in the source; this runs the real binary, on the
// showcase, so CI runs it. NotificationClickTests delivers a real click, on the harness.
//
// It asserts on an exit code, and waits for one, because the activation is real but nothing behind
// it is: a broken build fails fast after 7 to 8 seconds, a fixed one waits for activation data no
// shell will send and gives up after about ten with a managed exception (0xE0434352). Killing the
// launch before either is a case that passes on a broken build, which is how this was first
// written; so a launch that has not ended proves nothing and fails. The cost is real: a green run
// still ends in a managed crash and a Windows Error Reporting entry.
//
// The app goes first, through its window's close, which unregisters the platform: with a copy
// running, a second one reads its activation on the broken build too. So the suite has a launch of
// its own, which the case ends.

using System.Diagnostics;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase, for a suite whose case quits it.</summary>
public sealed class ColdStartShowcaseApp : ShowcaseApp;

[Trait("Dataset", "showcase")]
public sealed class NotificationLaunchTests(ColdStartShowcaseApp showcase) : IClassFixture<ColdStartShowcaseApp>
{
    // The command line the shell gives an unpackaged app it activates from a notification. Empty on
    // purpose: what is under test is reading the activation at all, not this app's encoding.
    private const string ActivationArgument = "----AppNotificationActivated:";

    // STATUS_STACK_BUFFER_OVERRUN, what a __fastfail reports: the one code that means the
    // notification platform was not up.
    private const int FailFast = unchecked((int)0xC0000409);

    // Past both endings, measured rather than guessed.
    private static readonly TimeSpan EndsWithin = TimeSpan.FromSeconds(30);

    [SkippableFact]
    public void ANotificationActivationDoesNotFailFastOnStartup()
    {
        // A host that cannot show notifications, a CI runner among them, fails any notification
        // launch fast whatever the order in Main; nothing is ever posted there, so no click can
        // reach that launch. Asked of the suite's own launch, before the launch under test, which
        // dies before it can log.
        Skip.IfNot(App.HasBuild(), "no Debug build to launch (clients/windows/build-and-run.ps1 -NoRun)");
        _ = showcase.App;
        var registration = AppLog.NewestSession().Split('\n')
            .LastOrDefault(line => line.Contains("notifications: registered", StringComparison.Ordinal)
                || line.Contains("notifications: could not register", StringComparison.Ordinal))?.Trim();
        Skip.If(registration?.Contains("system setting Unsupported", StringComparison.Ordinal) == true,
            $"this host cannot show notifications (the app logged: {registration}); check-notification-registration still pins the order");
        showcase.Case(nameof(ANotificationActivationDoesNotFailFastOnStartup), app => Launch(app, registration));
    }

    private static void Launch(App app, string? registration)
    {
        if (!app.CloseMailbox(TimeSpan.FromSeconds(30)))
        {
            throw new InvalidOperationException("the app was still running 30 s after its window was closed, so the platform is still registered and this would pass on a broken build");
        }
        var before = Process.GetProcessesByName("Mailcal").Select(p => p.Id).ToHashSet();
        int? exit = null;
        using (var launch = app.StartAnother(ActivationArgument))
        {
            if (launch.WaitForExit(EndsWithin))
            {
                exit = launch.ExitCode;
            }
            else
            {
                launch.Kill();
                launch.WaitForExit(10_000);
            }
        }
        // The activation is a COM one, so the runtime may answer it with another copy, which finds
        // nothing to redirect to and ends by itself: the case leaves the process table as it found it.
        App.Eventually(() => Process.GetProcessesByName("Mailcal").All(p => before.Contains(p.Id)), TimeSpan.FromSeconds(30));
        Assert.True(exit is not null, $"the launch had not ended after {EndsWithin.TotalSeconds} s, so neither ending was seen and this proved nothing");
        Assert.True(exit != FailFast,
            $"a launch activated from a notification died with 0x{exit:X8}: the platform is not registered before the activation is read, so every click that starts the app kills it, with no log and no window. The app had logged: {registration ?? "no registration line"}");
    }
}
