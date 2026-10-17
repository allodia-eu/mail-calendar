// One app for every harness suite: launching costs seconds, a case costs less, so the suites in the
// collection share the launch and each case starts by putting away what the last one left.

using System.Drawing;
using System.Drawing.Imaging;
using System.Windows.Forms;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

[assembly: CollectionBehavior(DisableTestParallelization = true)]

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The app on the harness store, launched on first use and stopped after the last suite.</summary>
public sealed class HarnessApp : IDisposable
{
    private App? _app;

    internal App App => _app ??= App.LaunchOnHarness();

    /// <summary>
    /// Runs one case against a clean surface, then <paramref name="cleanup"/> whatever happened. A
    /// failure leaves a screenshot and what the agent saw in failures/, named after the case, because
    /// whoever reads it did not write the test. A cleanup that fails after the case failed is not
    /// allowed to replace the case's own failure: that one is the answer.
    /// </summary>
    internal void Case(string name, Action<App> body, Action<App>? cleanup = null)
    {
        App.Reset();
        Exception? failed = null;
        try
        {
            body(App);
        }
        catch (Exception e)
        {
            SaveFailure(name);
            failed = e;
        }
        try
        {
            cleanup?.Invoke(App);
        }
        catch (Exception e) when (failed is null)
        {
            SaveFailure(name + "-cleanup");
            throw new InvalidOperationException($"the case passed, and putting things back after it failed: {e.Message}", e);
        }
        catch (Exception e)
        {
            failed = new AggregateException(failed!.Message, failed, e);
        }
        if (failed is not null)
        {
            System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(failed).Throw();
        }
    }

    private void SaveFailure(string name)
    {
        var dir = Path.Combine(App.Repo, "clients", "windows", "Mailcal.UiTests", "failures");
        Directory.CreateDirectory(dir);
        var stem = Path.Combine(dir, string.Concat(name.Select(c => char.IsLetterOrDigit(c) ? c : '-')));
        try
        {
            var screen = SystemInformation.VirtualScreen;
            using var shot = new Bitmap(screen.Width, screen.Height);
            using (var graphics = Graphics.FromImage(shot))
            {
                graphics.CopyFromScreen(screen.Left, screen.Top, 0, 0, shot.Size);
            }
            shot.Save(stem + ".png", ImageFormat.Png);
            File.WriteAllLines(stem + ".tree.txt", App.Agent.Dump());
        }
        catch (Exception e) when (e is IOException or InvalidOperationException or System.ComponentModel.Win32Exception)
        {
            // The failure being reported matters more than its picture.
        }
    }

    public void Dispose() => _app?.Dispose();
}

[CollectionDefinition(Name)]
public sealed class HarnessCollection : ICollectionFixture<HarnessApp>
{
    public const string Name = "harness";
}

/// <summary>
/// A case that needs the harness and a Debug build. Skipped, and saying why, when either is
/// missing, unless MAILCAL_UITESTS_REQUIRE_HARNESS=1: then it runs and fails, so a host meant to
/// have both cannot pass by having neither.
/// </summary>
public sealed class HarnessFactAttribute : FactAttribute
{
    public HarnessFactAttribute()
    {
        if (Environment.GetEnvironmentVariable("MAILCAL_UITESTS_REQUIRE_HARNESS") == "1")
        {
            return;
        }
        if (!Harness.IsUp())
        {
            Skip = "the harness is not up (scripts/dev/harness.sh up)";
        }
        else if (!App.HasBuild())
        {
            Skip = "no Debug build to launch (clients/windows/build-and-run.ps1 -NoRun)";
        }
    }
}
