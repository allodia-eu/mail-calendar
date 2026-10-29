// The UI suites' way into the running app (clients/windows/Mailcal.UiTests): a named pipe that
// answers questions about the windows' automation peers and acts on them, on the UI thread, in
// microseconds rather than the hundreds of milliseconds a cross-process UI Automation walk costs.
// The peers are the objects UI Automation itself reads, so what a suite finds here is what a
// screen reader is given.
//
// Off unless MAILCAL_TEST_AGENT names a pipe at launch. Every call to it is compiled out of a
// release build.

using System;
using System.Diagnostics;
using Microsoft.UI.Xaml;

#if DEBUG
using System.Collections.Generic;
using System.IO;
using System.IO.Pipes;
using System.Linq;
using System.Text;
using System.Text.Json.Nodes;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI.Dispatching;
#endif

namespace Allodia.Mailcal.Services;

/// <summary>The UI suites' in-process view of the app's windows.</summary>
internal static partial class TestAgent
{
    /// <summary>
    /// Starts listening when MAILCAL_TEST_AGENT names a pipe, and registers the mailbox. Call it on
    /// the UI thread, once the mailbox exists.
    /// </summary>
    [Conditional("DEBUG")]
    internal static void Start(Window mailbox)
    {
#if DEBUG
        var pipe = Environment.GetEnvironmentVariable("MAILCAL_TEST_AGENT")?.Trim();
        if (string.IsNullOrEmpty(pipe))
        {
            return;
        }
        _ui = mailbox.DispatcherQueue;
        Track(mailbox);
        new Thread(() => Serve(pipe)) { IsBackground = true, Name = "test agent" }.Start();
        Log.Info("test agent: listening");
#endif
    }

    /// <summary>Registers a window the app has opened, so the suites can see into it. Call it on the
    /// UI thread.</summary>
    [Conditional("DEBUG")]
    internal static void Track(Window window)
    {
#if DEBUG
        if (_ui is null)
        {
            return;
        }
        Tracked.Add(new WeakReference<Window>(window));
        // A closed window outlives its close, and every property read on it then throws. The window
        // comes from the sender, not a capture: a handler on a native object that refers back to it
        // keeps both alive for as long as the app runs (docs/client-traps.md).
        window.Closed += static (sender, _) => Tracked.RemoveAll(reference => !reference.TryGetTarget(out var open) || ReferenceEquals(open, sender));
#endif
    }

    /// <summary>
    /// Marks <paramref name="element"/> as having reached <paramref name="state"/>, for a suite to wait
    /// on: a moment the app knows and the screen does not show, such as a composer's editor having
    /// finished seeding. Call it on the UI thread.
    /// </summary>
    [Conditional("DEBUG")]
    internal static void Signal(UIElement element, string state)
    {
#if DEBUG
        if (_ui is not null)
        {
            Signals.AddOrUpdate(element, state);
        }
#endif
    }

#if DEBUG
    private static readonly System.Runtime.CompilerServices.ConditionalWeakTable<UIElement, string> Signals = new();
    private static DispatcherQueue? _ui;
    private static readonly List<WeakReference<Window>> Tracked = new();

    // One suite at a time: the pipe takes one client, and a client that goes is replaced by the next.
    private static void Serve(string pipe)
    {
        while (true)
        {
            try
            {
                using var server = new NamedPipeServerStream(
                    pipe, PipeDirection.InOut, 1, PipeTransmissionMode.Byte, PipeOptions.Asynchronous);
                server.WaitForConnection();
                using var reader = new StreamReader(server, new UTF8Encoding(false));
                using var writer = new StreamWriter(server, new UTF8Encoding(false)) { AutoFlush = true };
                while (reader.ReadLine() is { } line)
                {
                    writer.WriteLine(Answer(line).ToJsonString());
                }
            }
            catch (IOException)
            {
                // The suite went away mid-answer; wait for the next one.
            }
        }
    }

    private static JsonObject Answer(string line)
    {
        try
        {
            var request = JsonNode.Parse(line)?.AsObject() ?? throw new AgentException("an empty request");
            var op = (string?)request["op"] ?? throw new AgentException("a request without an op");
            return op switch
            {
                "ping" => new JsonObject { ["ok"] = true },
                "windows" => OnUi(ListWindows),
                "query" => OnUi(() => Query(request)),
                "wait" => Wait(request),
                "read" => OnUi(() => Read(request)),
                "invoke" => OnUi(() => Invoke(request)),
                "select" => OnUi(() => Select(request)),
                "setValue" => OnUi(() => SetValue(request)),
                "toggle" => OnUi(() => Toggle(request)),
                "expand" => OnUi(() => Expand(request, open: true)),
                "collapse" => OnUi(() => Expand(request, open: false)),
                "scrollIntoView" => OnUi(() => ScrollIntoView(request)),
                "focus" => OnUi(() => Focus(request)),
                "focused" => OnUi(Focused),
                "parent" => OnUi(() => Parent(request)),
                "scrollTo" => OnUi(() => ScrollTo(request)),
                "settle" => Settle(request),
                "dump" => OnUi(() => Dump(request)),
                "collect" => OnUi(Collect),
                _ => throw new AgentException($"an unknown op '{op}'"),
            };
        }
        catch (Exception e) when (e is AgentException or System.Text.Json.JsonException)
        {
            return new JsonObject { ["error"] = e.Message };
        }
    }

    /// <summary>Runs <paramref name="work"/> on the UI thread and waits for its answer. A peer may
    /// only be touched there.</summary>
    private static JsonObject OnUi(Func<JsonObject> work)
    {
        var done = new TaskCompletionSource<JsonObject>(TaskCreationOptions.RunContinuationsAsynchronously);
        var queued = _ui!.TryEnqueue(DispatcherQueuePriority.Normal, () =>
        {
            try
            {
                done.SetResult(work());
            }
            catch (AgentException e)
            {
                done.SetResult(new JsonObject { ["error"] = e.Message });
            }
            catch (Exception e)
            {
                // Where it was thrown as well: an HRESULT from a peer reads as nothing without it.
                var at = string.Join(" < ", (e.StackTrace ?? "").Split('\n').Take(4).Select(line => line.Trim()));
                done.SetResult(new JsonObject { ["error"] = $"{e.GetType().Name}: {e.Message} [{at}]" });
            }
        });
        if (!queued)
        {
            throw new AgentException("the UI thread has shut down");
        }
        if (!done.Task.Wait(TimeSpan.FromSeconds(30)))
        {
            throw new AgentException("the UI thread did not answer within 30 s");
        }
        return done.Task.Result;
    }

    /// <summary>A collection, for a suite that has to know the agent survives one: the release probe
    /// collects while the suites run.</summary>
    private static JsonObject Collect()
    {
        GC.Collect();
        return new JsonObject { ["ok"] = true };
    }

    /// <summary>A request the agent cannot answer, said to the suite rather than thrown at the app.</summary>
    private sealed class AgentException(string message) : Exception(message);
#endif
}
