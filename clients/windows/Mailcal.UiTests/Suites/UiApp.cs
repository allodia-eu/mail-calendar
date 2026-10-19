// One app for every suite in a collection: launching costs seconds, a case costs less, so the suites
// in the collection share the launch and each case starts by putting away what the last one left.

using System.Drawing;
using System.Drawing.Imaging;
using System.Windows.Forms;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

[assembly: CollectionBehavior(DisableTestParallelization = true)]

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The app a collection's suites share, launched on first use and stopped after the last suite.</summary>
public abstract class UiApp : IDisposable
{
    private App? _app;

    internal App App => _app ??= Launch();

    /// <summary>The launch, made once, on first use.</summary>
    private protected abstract App Launch();

    /// <summary>The MAILCAL_* and log switches this launch is made with. The app reads them once,
    /// at startup, so suites that need different ones need a launch, and a collection, of their own.</summary>
    protected virtual IReadOnlyDictionary<string, string> Env { get; } = new Dictionary<string, string>();

    /// <summary>
    /// Runs one case against a clean surface, then <paramref name="cleanup"/> whatever happened. A
    /// failure leaves a screenshot, what the agent saw and the exception in failures/, named after the
    /// case, because whoever reads it did not write the test. A cleanup that fails after the case failed is not
    /// allowed to replace the case's own failure: that one is the answer. With
    /// <paramref name="reset"/> false the surface is left as the case before it left it, for a case
    /// that goes on from there in a suite whose cases run in the order written.
    /// </summary>
    internal void Case(string name, Action<App> body, Action<App>? cleanup = null, bool reset = true)
    {
        if (reset)
        {
            App.Reset();
        }
        Exception? failed = null;
        try
        {
            body(App);
        }
        catch (Exception e)
        {
            SaveFailure(name, e);
            failed = e;
        }
        try
        {
            cleanup?.Invoke(App);
        }
        catch (Exception e) when (failed is null)
        {
            SaveFailure(name + "-cleanup", e);
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

    private void SaveFailure(string name, Exception failure)
    {
        var dir = Path.Combine(App.Repo, "clients", "windows", "Mailcal.UiTests", "failures");
        Directory.CreateDirectory(dir);
        var stem = Path.Combine(dir, string.Concat(name.Select(c => char.IsLetterOrDigit(c) ? c : '-')));
        try
        {
            File.WriteAllText(stem + ".error.txt", failure.ToString());
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

    public void Dispose()
    {
        _app?.Dispose();
        GC.SuppressFinalize(this);
    }
}

