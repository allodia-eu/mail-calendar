// What the diagnostic log says when something is about to kill the app (docs/logging.md → "A
// crash says so on the way out"). Without it a crash is indistinguishable in the file from a clean
// exit: the log simply stops, with nothing wrong on the last line.
//
// Added because one did. A COMException out of NavigationView.IsPaneOpen took the app down as a
// stowed exception inside Microsoft.UI.Xaml.dll, and the only record of it anywhere on the machine
// was a Windows Error Reporting entry naming an offset in combase.dll.
//
// WinUI-free on purpose, so the line composition is reachable from Mailcal.Tests (which cannot
// link WinUI types). The one handler that needs the Application object, WinUI's own
// UnhandledException, for XAML-thread faults, is wired in App.xaml.cs and formats its line
// through Record here, so all four shapes say the same thing.

using System.Threading.Tasks;

namespace Allodia.Mailcal.Services;

/// <summary>Writes the stack of anything about to kill the process into the diagnostic log.</summary>
internal static class CrashLog
{
    /// <summary>
    /// Wires the two runtime-wide handlers. Called from <c>Program.Main</c> immediately after
    /// <c>Log.Init</c>, the ordering is the point, since <c>Log.Write</c> silently drops
    /// everything until the sink has a path.
    /// </summary>
    /// <remarks>
    /// Neither handler marks the fault handled: the process is meant to still fail, and this only
    /// makes it say why on the way out.
    /// </remarks>
    internal static void WatchProcess()
    {
        // The CLR's own: everything off the XAML thread, including a background thread that throws.
        AppDomain.CurrentDomain.UnhandledException += (_, e) =>
            Log.Error(Record(e.IsTerminating ? "on a terminating thread" : "on a thread", e.ExceptionObject));

        // A Task nobody awaited, whose exception the finalizer would otherwise swallow whole. This
        // one is NOT a crash, since .NET 4.5 the process survives it, which is exactly why it is
        // worth a line: an `async void` handler or a fire-and-forget `_ =` call that faults leaves
        // no other trace anywhere. SetObserved is deliberately not called; not calling it keeps the
        // framework's default behaviour rather than changing it.
        TaskScheduler.UnobservedTaskException += (_, e) =>
            Log.Error(Record("in a task nobody awaited, which did not stop the app", e.Exception));
    }

    /// <summary>
    /// Arms the core's native-fault handler over the same file.
    /// </summary>
    /// <remarks>
    /// An access violation inside the cdylib reaches none of the handlers above: the CLR tears the
    /// process down without raising <c>AppDomain.UnhandledException</c>, so the log stops mid-line
    /// and Windows Error Reporting holds the only record. The core installs a vectored exception
    /// handler for it, which observes and never handles, the process still fails exactly as it
    /// would have.
    /// <para>
    /// Kept out of <see cref="WatchProcess"/> because this is the one call here that crosses the
    /// FFI, and <c>Mailcal.Tests</c> links no cdylib; keeping them apart is what lets the record
    /// composition above stay unit-tested. A missing library is swallowed for the reason every
    /// other log failure is: diagnostics must never be what stops the app from starting, and a
    /// cdylib that is genuinely absent takes the app down at its first real call regardless.
    /// </para>
    /// </remarks>
    internal static void WatchForNativeFaults(string logPath)
    {
        try
        {
            uniffi.mailcal_bindings.MailcalBindingsMethods.WatchForNativeFaults(logPath);
        }
        catch (Exception e)
        {
            Log.Warn($"the native-fault handler could not be armed: {e.GetType().Name}");
        }
    }

    /// <summary>The line one fault writes.</summary>
    /// <remarks>
    /// <c>unhandled</c> is the word every platform's crash line carries, one string support greps
    /// across four clients, and a .NET exception's <c>ToString()</c> already opens with
    /// <c>Type: Message</c> and continues into the frames, so the phrase says only what the type
    /// cannot: which part of the runtime the fault came out of.
    /// <para>
    /// The fault arrives as <see cref="object"/> because <c>AppDomain.UnhandledException</c> hands
    /// over an <c>ExceptionObject</c>, which is only an <c>Exception</c> by convention.
    /// </para>
    /// </remarks>
    internal static string Record(string where, object? fault) => $"unhandled {where}: {fault}";
}
