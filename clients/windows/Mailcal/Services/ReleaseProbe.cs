// A debug-only count of what the app has closed and the runtime still holds: windows, composers and
// editor web views. The UI suite reads it from the log (uitests/ReleaseProbe.Tests.ps1), because a
// closed window that is never freed looks exactly like one that is, and Mailcal.Tests cannot host a
// window at all.
//
// Off unless MAILCAL_RELEASE_PROBE=1 at launch. Every call to it is compiled out of a release build.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Threading.Tasks;

namespace Allodia.Mailcal.Services;

/// <summary>Counts closed objects the runtime has not yet taken back.</summary>
internal static class ReleaseProbe
{
#if DEBUG
    // XAML gives a closed tree back over several collections, each releasing what the previous one
    // let go of: three to six for a composer window, measured. The bound is well past that, and a
    // probe stops as soon as nothing is held.
    private const int Rounds = 40;
    private static readonly TimeSpan Interval = TimeSpan.FromMilliseconds(250);

    private static readonly bool On =
        Environment.GetEnvironmentVariable("MAILCAL_RELEASE_PROBE")?.Trim() == "1";
    private static readonly List<WeakReference> Watched = new();
    private static bool _probing;
#endif

    /// <summary>
    /// Records that <paramref name="closed"/>, a <paramref name="kind"/>, has been closed and should
    /// now be freed, and starts a probe unless one is running. Call it on the UI thread.
    /// </summary>
    [Conditional("DEBUG")]
    internal static void Watch(object closed, string kind)
    {
#if DEBUG
        if (!On)
        {
            return;
        }
        Watched.Add(new WeakReference(closed));
        Log.Info($"release probe: watching a closed {kind}");
        if (!_probing)
        {
            _probing = true;
            _ = ProbeAsync();
        }
#endif
    }

#if DEBUG
    // Collects and counts until nothing watched is held or the rounds run out, then logs once. A
    // close arriving meanwhile starts the rounds again, and the count and the line are written in
    // one turn of the UI thread, so a line answers for everything watched before it.
    //
    // Never GC.WaitForPendingFinalizers: this runs on the UI thread, and releasing a WinRT object
    // from its finaliser can need that thread.
    private static async Task ProbeAsync()
    {
        var seen = -1;
        var held = 0;
        for (var round = 0; round < Rounds; round++)
        {
            await Task.Delay(Interval);
            if (Watched.Count != seen)
            {
                seen = Watched.Count;
                round = 0;
            }
            GC.Collect();
            held = Watched.Count(reference => reference.IsAlive);
            if (held == 0)
            {
                break;
            }
        }
        _probing = false;
        Log.Info($"release probe: {held} of {Watched.Count} closed still held");
    }
#endif
}
