// How a launch is set up: the environment it is made with, the store preferences it starts from,
// and the agent it is reached through.

using System.Diagnostics;

namespace Allodia.Mailcal.UiTests.Driver;

internal sealed partial class App
{
    /// <summary>The environment a launch is made with, and the pipe its agent listens on.</summary>
    private static (Dictionary<string, string?> Environment, string Pipe) LaunchEnvironment(IReadOnlyDictionary<string, string>? env, Dataset dataset)
    {
        var pipe = $"mailcal-uitests-{Environment.ProcessId}-{++_launches}";
        var environment = new Dictionary<string, string?>
        {
            ["MAILCAL_OPEN_FIRST"] = null,
            ["MAILCAL_CALENDAR"] = null,
            ["MAILCAL_SWIPE"] = null,
            ["MAILCAL_SHOWCASE"] = null,
            ["MAILCAL_SHOWCASE_SCREEN"] = null,
            ["MAILCAL_APPEARANCE"] = null,
            ["MAILCAL_FAKE_REPLY_DELIVERY"] = null,
            ["MAILCAL_FAKE_SYNC_HINT"] = null,
            ["MAILCAL_FAKE_SYNC_PAUSED"] = null,
            ["MAILCAL_FAKE_SYNC_PROGRESS"] = null,
            ["MAILCAL_RELEASE_PROBE"] = null,
            ["MAILCAL_FLOW_PREVIEW"] = null,
            ["MAILCAL_FAKE_KEYSTORE"] = null,
            ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = null,
            ["MAILCAL_DEV_ACCOUNT"] = dataset switch
            {
                Dataset.FirstRun => "first-run",
                Dataset.Showcase => null,
                _ => Environment.GetEnvironmentVariable("MAILCAL_DEV_ACCOUNT") ?? "stalwart",
            },
            ["MAILCAL_EXTRA_CA"] = dataset == Dataset.Showcase ? null : HarnessCa.For(Repo),
            ["MAILCAL_TEST_AGENT"] = pipe,
            // Nothing a case presses opens a browser or another app: a sign-in is completed with the
            // requests its page makes (NextcloudBrowser), so a tab would be left behind and nothing more.
            ["MAILCAL_NO_EXTERNAL_OPEN"] = "1",
        };
        if (dataset == Dataset.Showcase)
        {
            foreach (var (name, value) in ShowcaseDefaults)
            {
                environment[name] = value;
            }
        }
        foreach (var (name, value) in env ?? new Dictionary<string, string>())
        {
            environment[name] = value;
        }
        return (environment, pipe);
    }

    private static int _launches;

    /// <summary>The launch's agent, or the reason there is none: an app that ended before its
    /// agent answered is a launch that crashed, and its exit code says how (0xC000027B is a WinUI
    /// failure inside <c>Application.Start</c>), where a bare pipe timeout says nothing.</summary>
    private static Agent ConnectAgent(Process process, string pipe)
    {
        try
        {
            return Agent.Connect(pipe, TimeSpan.FromSeconds(60));
        }
        catch (TimeoutException) when (process.HasExited)
        {
            throw new InvalidOperationException($"the app exited with 0x{process.ExitCode:X8} before its agent answered; its log is the newest session in app.log");
        }
    }

    /// <summary>
    /// Puts a harness store's preferences where the cases expect them. The showcase pins both
    /// through <c>MAILCAL_SHOWCASE</c> and persists neither.
    /// <list type="bullet">
    /// <item>The language is English, which the cases' copy is read in (<see cref="Catalog.English"/>).
    /// Left at "system", the app follows the host's display language, so on a Dutch Windows every
    /// case that reads a sentence fails. A <paramref name="language"/> of <c>null</c> stores no
    /// choice, as a fresh install has none: for a case about that launch, which reads no copy.</item>
    /// <item>The window placement is forgotten, so the window opens at the app's own default size.
    /// The app reopens where it was closed, so a window left maximised by one launch, a case or the
    /// developer would size every later one, and a case that needs a narrow reading pane cannot
    /// reach it in a full-screen window.</item>
    /// </list>
    /// </summary>
    private static void PrepareStore(Dictionary<string, string?> environment, string? language = "en")
    {
        if (environment["MAILCAL_DEV_ACCOUNT"] is not { } devAccount)
        {
            return;
        }
        var store = DataDir.Store(devAccount);
        Directory.CreateDirectory(store);
        var languageFile = Path.Combine(store, "language.txt");
        if (language is null)
        {
            File.Delete(languageFile);
        }
        else
        {
            File.WriteAllText(languageFile, language);
        }
        File.Delete(Path.Combine(store, "window.txt"));
    }

    /// <summary>Runs <paramref name="start"/> with <paramref name="environment"/> set in this
    /// process, which the shell and COM hand to what they start, and puts the old values back.</summary>
    private static void WithEnvironment(Dictionary<string, string?> environment, Action start)
    {
        var saved = environment.Keys.ToDictionary(name => name, Environment.GetEnvironmentVariable);
        try
        {
            foreach (var (name, value) in environment)
            {
                Environment.SetEnvironmentVariable(name, value);
            }
            start();
        }
        finally
        {
            foreach (var (name, value) in saved)
            {
                Environment.SetEnvironmentVariable(name, value);
            }
        }
    }
}
