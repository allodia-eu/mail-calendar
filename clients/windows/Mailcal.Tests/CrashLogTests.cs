// The crash line (Services/CrashLog.cs). Until this file there was no gate on it at all: neither
// this suite nor uitests/ mentioned WatchForCrashes, so the one line that says the app died rather
// than quit was carried on trust.
//
// What is testable here is the composition, and it is pinned the way SessionMarkerTests pins the
// session marker: `unhandled` is one string support greps across four clients, so every .NET shape
// Every XAML thread, CLR domain, and unobserved task has to lead with it. Whether the handlers are
// actually SUBSCRIBED is not reachable from a plain net10.0 assembly; that half is verified by
// hand against the running app (docs/logging.md).

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CrashLogTests
{
    [Fact]
    public void Every_shape_leads_with_the_word_support_greps()
    {
        // The four phrases the wiring passes in. A shape that stopped saying "unhandled" would be
        // invisible in a support log searched for the word the other three clients also use.
        foreach (var where in new[]
                 {
                     "on the XAML thread",
                     "on a terminating thread",
                     "on a thread",
                     "in a task nobody awaited, which did not stop the app",
                 })
        {
            Assert.StartsWith($"unhandled {where}: ", CrashLog.Record(where, new InvalidOperationException("q")));
        }
    }

    [Fact]
    public void The_type_the_message_and_the_stack_all_reach_the_line()
    {
        // ToString() on a thrown exception opens with `Type: Message` and continues into the
        // frames. It has to be THROWN, an exception that was merely constructed carries no stack.
        Exception thrown;
        try
        {
            throw new InvalidOperationException("quorrix went sideways");
        }
        catch (InvalidOperationException e)
        {
            thrown = e;
        }

        var line = CrashLog.Record("on the XAML thread", thrown);

        Assert.Contains("System.InvalidOperationException", line, StringComparison.Ordinal);
        Assert.Contains("quorrix went sideways", line, StringComparison.Ordinal);
        Assert.Contains("   at ", line, StringComparison.Ordinal);
    }

    [Fact]
    public void An_inner_exception_survives_into_the_line()
    {
        // A stowed WinUI fault arrives wrapped, and the inner one is the half that says what broke.
        var line = CrashLog.Record(
            "on a terminating thread",
            new InvalidOperationException("outer", new ArgumentException("quorrix underneath")));

        Assert.Contains("quorrix underneath", line, StringComparison.Ordinal);
    }

    [Fact]
    public void A_fault_that_is_not_an_exception_still_writes_a_line()
    {
        // AppDomain.UnhandledException hands over an ExceptionObject, which is only an Exception by
        // convention, the CLR permits any object. A crash handler that threw here would take the
        // app down in place of the fault it was reporting.
        Assert.Equal("unhandled on a thread: 42", CrashLog.Record("on a thread", 42));
        Assert.Equal("unhandled on a thread: ", CrashLog.Record("on a thread", null));
    }
}
