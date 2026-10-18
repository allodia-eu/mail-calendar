// The running app: launched on the harness store, or on an empty one for a first run, with its test
// agent listening, and brought to the state every suite starts from. The launch mirrors control.ps1
// and the readiness mirrors uitests/dataset.ps1, whose headers say why each step is there.

using System.Diagnostics;

namespace Allodia.Mailcal.UiTests.Driver;

/// <summary>The app under test, and the moves every suite shares.</summary>
internal sealed partial class App : IDisposable
{
    private readonly Process _process;

    internal Agent Agent { get; }

    /// <summary>Whether the app is still running: a crash leaves every other read stale.</summary>
    internal bool Running => !_process.HasExited;

    internal int ProcessId => _process.Id;

    private App(Process process, Agent agent)
    {
        _process = process;
        Agent = agent;
    }

    /// <summary>The repository root, found from where the suites were built.</summary>
    internal static string Repo { get; } = FindRepo();

    /// <summary>Launches the newest Debug build on the harness store and waits until it is ready:
    /// past the first-run question and the default-mail-app offer, rows listed, and the calendar
    /// back from the server.</summary>
    internal static App LaunchOnHarness(IReadOnlyDictionary<string, string>? env = null) => Launch(env, firstRun: false);

    /// <summary>
    /// Launches the newest Debug build on an empty store and waits for the account-setup form,
    /// past the welcome question: for a suite that sets an account up against the harness through
    /// the form, as a first run does. The store is emptied first, its credentials included
    /// (clear-dev-namespace.ps1 says why both), because a first run is defined by there being
    /// nothing in it.
    /// </summary>
    internal static App LaunchOnFirstRun(IReadOnlyDictionary<string, string>? env = null) => Launch(env, firstRun: true);

    private static App Launch(IReadOnlyDictionary<string, string>? env, bool firstRun)
    {
        Stop();
        if (firstRun)
        {
            ClearNamespace("dev-first-run");
        }
        var pipe = $"mailcal-uitests-{Environment.ProcessId}";
        var environment = new Dictionary<string, string?>
        {
            ["MAILCAL_OPEN_FIRST"] = null,
            ["MAILCAL_CALENDAR"] = null,
            ["MAILCAL_SWIPE"] = null,
            ["MAILCAL_SHOWCASE"] = null,
            ["MAILCAL_SHOWCASE_SCREEN"] = null,
            ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = null,
            ["MAILCAL_DEV_ACCOUNT"] = firstRun ? "first-run" : Environment.GetEnvironmentVariable("MAILCAL_DEV_ACCOUNT") ?? "stalwart",
            ["MAILCAL_EXTRA_CA"] = Path.Combine(Repo, "docker", "stalwart", "tls", "harness-ca.pem"),
            ["MAILCAL_TEST_AGENT"] = pipe,
        };
        foreach (var (name, value) in env ?? new Dictionary<string, string>())
        {
            environment[name] = value;
        }

        // Through the shell, so the app inherits none of this process's handles: a child holding the
        // test host's output open keeps `dotnet test` waiting after the last case. The shell hands
        // over this process's environment, so it is set here for the launch and put back after.
        var saved = environment.Keys.ToDictionary(name => name, Environment.GetEnvironmentVariable);
        var launchedAt = DateTimeOffset.Now;
        Process process;
        try
        {
            foreach (var (name, value) in environment)
            {
                Environment.SetEnvironmentVariable(name, value);
            }
            process = Process.Start(new ProcessStartInfo(Exe()) { UseShellExecute = true })
                ?? throw new InvalidOperationException("the app did not start");
        }
        finally
        {
            foreach (var (name, value) in saved)
            {
                Environment.SetEnvironmentVariable(name, value);
            }
        }
        var app = new App(process, Agent.Connect(pipe, TimeSpan.FromSeconds(60)));
        if (firstRun)
        {
            app.WaitSetupForm();
        }
        else
        {
            app.WaitReady(launchedAt, environment["MAILCAL_DEV_ACCOUNT"]!);
        }
        return app;
    }

    /// <summary>
    /// The welcome question answered with Get started, then the setup form. Ready means the question
    /// was answered and not merely that the form is up: a core that failed to open draws the form
    /// with no welcome question before it (uitests/dataset.ps1 has the case it hid).
    /// </summary>
    private void WaitSetupForm()
    {
        // The button is in the tree, out of view behind the launch view, from the start: only one on
        // screen is the question being asked.
        var asked = Filter.ById("WelcomeGetStarted", "Button") with { OnScreen = true };
        var form = Filter.ById("DetectEmail", "Edit") with { OnScreen = true };
        if (Agent.Wait(asked, TimeSpan.FromSeconds(60)) is null)
        {
            throw new InvalidOperationException(Agent.Find(form) is null
                ? "the launch view never gave way to the welcome question"
                : "the setup form came up with no welcome question before it, which is what a core that failed to open draws");
        }
        var answered = Eventually(() =>
        {
            if (Agent.Find(asked) is { } welcome)
            {
                Agent.Invoke(welcome);
            }
            return Agent.Wait(form, TimeSpan.FromSeconds(2)) is not null && Agent.Find(asked) is null;
        }, TimeSpan.FromSeconds(30));
        if (!answered)
        {
            throw new InvalidOperationException("the welcome question never gave way to the account-setup form");
        }
    }

    private static void ClearNamespace(string space)
    {
        var script = Path.Combine(Repo, "clients", "windows", "clear-dev-namespace.ps1");
        var clear = new ProcessStartInfo("pwsh") { UseShellExecute = false, RedirectStandardOutput = true, RedirectStandardError = true };
        foreach (var argument in new[] { "-NoProfile", "-NonInteractive", "-File", script, "-Namespace", space })
        {
            clear.ArgumentList.Add(argument);
        }
        using var process = Process.Start(clear) ?? throw new InvalidOperationException("pwsh did not start");
        var said = process.StandardOutput.ReadToEndAsync();
        var complained = process.StandardError.ReadToEndAsync();
        if (!process.WaitForExit(60_000) || process.ExitCode != 0)
        {
            throw new InvalidOperationException($"{space} could not be emptied: {said.Result}{complained.Result}");
        }
    }

    private void WaitReady(DateTimeOffset launchedAt, string devAccount)
    {
        // A store that has never been asked opens on the welcome question, answered with Get
        // started: a test run must not opt this machine into analytics (docs/analytics.md).
        var deadline = DateTime.UtcNow.AddSeconds(90);
        var list = Agent.Wait(Filter.ById("RowsList"), TimeSpan.FromSeconds(3));
        while (list is null && DateTime.UtcNow < deadline)
        {
            if (Agent.Find(Filter.ById("WelcomeGetStarted", "Button")) is { } welcome)
            {
                Agent.Invoke(welcome);
            }
            list = Agent.Wait(Filter.ById("RowsList"), TimeSpan.FromSeconds(1));
        }
        if (list is null || Agent.Wait(new Filter { Type = "ListItem", Under = list }, TimeSpan.FromSeconds(60)) is null)
        {
            throw new InvalidOperationException("the harness store never listed any mail; is the harness seeded (scripts/dev/harness.sh up)?");
        }
        DeclineDefaultMailAppOffer(devAccount);
        WaitCalendarSynced(launchedAt);
    }

    /// <summary>
    /// The one-time "Open mail links here?" offer, declined on a store that has never answered it.
    /// It is modal, and while it is up a real press reaches nothing behind it. It arrives late, so a
    /// store that has not answered is waited on; one that has is not.
    /// </summary>
    private void DeclineDefaultMailAppOffer(string devAccount)
    {
        // Each harness account runs on a store of its own (AppPaths.DevStoreSubdir).
        var store = devAccount.Trim().ToLowerInvariant() switch
        {
            "stalwart" => "dev",
            "stalwart-multi" => "dev-multi",
            "stalwart-imap" => "dev-imap",
            "stalwart-linked" => "dev-linked",
            "first-run" => "dev-first-run",
            _ => throw new InvalidOperationException($"no harness store is known for MAILCAL_DEV_ACCOUNT={devAccount}"),
        };
        var prefs = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Allodia", "MailCalendar", store, "preferences.toml");
        bool Answered() => File.Exists(prefs)
            && File.ReadLines(prefs).Any(line => line.TrimStart().StartsWith("default_mail_app_offer", StringComparison.Ordinal));
        if (Answered())
        {
            return;
        }
        var offer = Agent.Wait(Filter.ById("DefaultMailAppOffer"), TimeSpan.FromSeconds(30))
            ?? throw new InvalidOperationException("this store has never answered the default-mail-app offer, and it did not appear within 30 s");
        var decline = Agent.Find(Filter.ById("CloseButton", "Button") with { Under = offer })
            ?? throw new InvalidOperationException("the default-mail-app offer has no Not now button");
        Agent.Invoke(decline);
        if (!Eventually(Answered, TimeSpan.FromSeconds(10)))
        {
            throw new InvalidOperationException($"the offer was declined, but {prefs} never recorded it");
        }
    }

    /// <summary>The calendar's refresh from the server, read off the core's log because the
    /// calendar shows nothing when it is done. The first rebuild is from the cache and proves
    /// nothing; refresh_calendar is the one that has been to the server.</summary>
    private static void WaitCalendarSynced(DateTimeOffset since)
    {
        if (!Eventually(() => AppLog.Session(since).Contains("refresh_calendar:", StringComparison.Ordinal), TimeSpan.FromSeconds(60)))
        {
            throw new InvalidOperationException("this launch never refreshed its calendar from the server within 60 s");
        }
    }

    /// <summary>Whether <paramref name="condition"/> came true within <paramref name="timeout"/>.</summary>
    internal static bool Eventually(Func<bool> condition, TimeSpan timeout)
    {
        var deadline = DateTime.UtcNow + timeout;
        while (!condition())
        {
            if (DateTime.UtcNow > deadline)
            {
                return false;
            }
            Thread.Sleep(50);
        }
        return true;
    }

    /// <summary>The newest Debug build: the MAILCAL_* hooks and the agent are compiled out of a
    /// release one.</summary>
    private static string Exe() =>
        new DirectoryInfo(Path.Combine(Repo, "clients", "windows", "Mailcal", "bin"))
            .EnumerateFiles("Mailcal.exe", SearchOption.AllDirectories)
            .Where(f => f.FullName.Contains($"{Path.DirectorySeparatorChar}Debug{Path.DirectorySeparatorChar}", StringComparison.OrdinalIgnoreCase))
            .OrderByDescending(f => f.LastWriteTimeUtc)
            .FirstOrDefault()?.FullName
        ?? throw new InvalidOperationException("no Debug Mailcal.exe under clients/windows/Mailcal/bin; run clients/windows/build-and-run.ps1 -NoRun first");

    /// <summary>Whether there is a Debug build to launch.</summary>
    internal static bool HasBuild()
    {
        try
        {
            return Exe().Length > 0;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
    }

    private static void Stop()
    {
        foreach (var running in Process.GetProcessesByName("Mailcal"))
        {
            running.Kill();
            running.WaitForExit(10_000);
        }
    }

    private static string FindRepo()
    {
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            if (File.Exists(Path.Combine(dir.FullName, "VERSION")) && File.Exists(Path.Combine(dir.FullName, "Cargo.toml")))
            {
                return dir.FullName;
            }
        }
        throw new InvalidOperationException($"no repository root above {AppContext.BaseDirectory}");
    }

    public void Dispose()
    {
        // The app first: the pipe is the app's to close, and a suite that ends must not leave it running.
        if (!_process.HasExited)
        {
            _process.Kill();
            _process.WaitForExit(10_000);
        }
        _process.Dispose();
        Agent.Dispose();
    }
}
