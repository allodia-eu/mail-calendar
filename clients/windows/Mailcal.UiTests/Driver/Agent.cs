// The suites' side of the app's test agent (Mailcal/Services/TestAgent.cs): one JSON object a line
// each way over a named pipe. Finding and acting happen inside the app, on its UI thread, through
// the automation peers UI Automation reads.

using System.IO.Pipes;
using System.Text;
using System.Text.Json.Nodes;

namespace Allodia.Mailcal.UiTests.Driver;

/// <summary>A connection to the running app's test agent.</summary>
internal sealed class Agent : IDisposable
{
    private readonly NamedPipeClientStream _pipe;
    private readonly StreamReader _reader;
    private readonly StreamWriter _writer;

    private Agent(NamedPipeClientStream pipe)
    {
        _pipe = pipe;
        _reader = new StreamReader(pipe, new UTF8Encoding(false));
        _writer = new StreamWriter(pipe, new UTF8Encoding(false)) { AutoFlush = true };
    }

    /// <summary>Connects to <paramref name="pipeName"/>, waiting for the app to start listening.</summary>
    internal static Agent Connect(string pipeName, TimeSpan timeout)
    {
        var deadline = DateTime.UtcNow + timeout;
        while (true)
        {
            var pipe = new NamedPipeClientStream(".", pipeName, PipeDirection.InOut);
            try
            {
                pipe.Connect(250);
                return new Agent(pipe);
            }
            catch (TimeoutException) when (DateTime.UtcNow < deadline)
            {
                pipe.Dispose();
            }
        }
    }

    /// <summary>Where every call and its answer go, timed, when MAILCAL_UITESTS_TRACE=1: lined up
    /// against the app's own log, it says which step a server write followed.</summary>
    private static readonly string? TracePath = Environment.GetEnvironmentVariable("MAILCAL_UITESTS_TRACE") == "1"
        ? Path.Combine(Path.GetTempPath(), "mailcal-uitests-trace.log")
        : null;

    private JsonObject Call(JsonObject request)
    {
        var sent = request.ToJsonString();
        _writer.WriteLine(sent);
        var line = _reader.ReadLine() ?? throw new InvalidOperationException("the app closed the test agent's pipe");
        if (TracePath is not null)
        {
            File.AppendAllText(TracePath, $"{DateTime.Now:HH:mm:ss.fff} {sent}\n    -> {(line.Length > 300 ? line[..300] : line)}\n");
        }
        var answer = JsonNode.Parse(line)!.AsObject();
        if (answer["error"] is JsonNode error)
        {
            throw new InvalidOperationException($"the test agent refused '{request["op"]}': {error}");
        }
        return answer;
    }

    private static JsonObject Op(string op, Filter? filter = null)
    {
        var request = filter?.ToJson() ?? new JsonObject();
        request["op"] = op;
        return request;
    }

    /// <summary>The first element the filter accepts, or null.</summary>
    internal Element? Find(Filter filter) => Elements(Call(Op("query", filter))).FirstOrDefault();

    /// <summary>Every element the filter accepts, in the order UI Automation reports them.</summary>
    internal IReadOnlyList<Element> FindAll(Filter filter)
    {
        var request = Op("query", filter);
        request["all"] = true;
        return Elements(Call(request));
    }

    /// <summary>Waits for the filter to accept something, checked on every turn of the app's UI
    /// thread; null when <paramref name="timeout"/> runs out first.</summary>
    internal Element? Wait(Filter filter, TimeSpan timeout)
    {
        var request = Op("wait", filter);
        request["timeoutMs"] = (int)timeout.TotalMilliseconds;
        var answer = Call(request);
        return (bool)answer["ok"]! ? Elements(answer).First() : null;
    }

    /// <summary>Waits for the filter to accept nothing; whether it did within <paramref name="timeout"/>.</summary>
    internal bool WaitGone(Filter filter, TimeSpan timeout)
    {
        var request = Op("wait", filter);
        request["absent"] = true;
        request["timeoutMs"] = (int)timeout.TotalMilliseconds;
        return (bool)Call(request)["ok"]!;
    }

    /// <summary>Waits until the windows have looked the same for <paramref name="quiet"/>, and at
    /// least <paramref name="floor"/> has passed; whether that happened within <paramref name="cap"/>.</summary>
    internal bool Settle(TimeSpan? quiet = null, TimeSpan? floor = null, TimeSpan? cap = null)
    {
        var request = Op("settle");
        request["quietMs"] = (int)(quiet ?? TimeSpan.FromMilliseconds(150)).TotalMilliseconds;
        request["floorMs"] = (int)(floor ?? TimeSpan.Zero).TotalMilliseconds;
        request["capMs"] = (int)(cap ?? TimeSpan.FromSeconds(2)).TotalMilliseconds;
        return (bool)Call(request)["ok"]!;
    }

    /// <summary>The element's state now: its value, toggle, selection and focus as well.</summary>
    /// <summary>Has the app collect, as the release probe does while a suite runs.</summary>
    internal void Collect() => Call(Op("collect"));

    internal ElementState Read(Element element) => ElementState.From(Call(Handle("read", element)));

    /// <summary>Presses the element: Invoke, or Select where it cannot be invoked. A control that is
    /// disabled is waited for, as a person waits for one, because a peer refuses to press it.</summary>
    internal void Invoke(Element element)
    {
        if (!App.Eventually(() => Read(element).Element.Enabled, TimeSpan.FromSeconds(5)))
        {
            throw new InvalidOperationException($"'{element.Name}' (#{element.Id}) stayed disabled, so it cannot be pressed");
        }
        Call(Handle("invoke", element));
    }

    internal void Select(Element element) => Call(Handle("select", element));

    /// <summary>Sets a text box's value, which the box takes as an edit.</summary>
    internal void SetValue(Element element, string value)
    {
        var request = Handle("setValue", element);
        request["value"] = value;
        Call(request);
    }

    /// <summary>Toggles a check box or toggle button; the state it is in afterwards.</summary>
    internal string Toggle(Element element) => (string)Call(Handle("toggle", element))["toggle"]!;

    /// <summary>Puts a check box or toggle button <paramref name="on"/> or off, whichever it was.</summary>
    internal void SetToggle(Element element, bool on)
    {
        if ((Read(element).Toggle == "On") != on)
        {
            Toggle(element);
        }
    }

    internal void ScrollIntoView(Element element) => Call(Handle("scrollIntoView", element));

    /// <summary>Opens a combo box or a drop-down.</summary>
    internal void Expand(Element element) => Call(Handle("expand", element));

    /// <summary>Closes what <see cref="Expand"/> opened.</summary>
    internal void Collapse(Element element) => Call(Handle("collapse", element));

    internal void Focus(Element element) => Call(Handle("focus", element));

    /// <summary>The nearest ancestor of <paramref name="element"/> that has a peer, or null.</summary>
    internal Element? Parent(Element element) => Elements(Call(Handle("parent", element))).FirstOrDefault();

    /// <summary>The element that has the keyboard, or null when nothing in the app does.</summary>
    internal Element? Focused() => Elements(Call(Op("focused"))).FirstOrDefault();

    /// <summary>Scrolls the first vertically scrollable element at or under <paramref name="element"/>
    /// to its end; whether there was one.</summary>
    internal bool ScrollToEnd(Element element) => ScrollTo(element, 100);

    /// <summary>Scrolls the first vertically scrollable element at or under <paramref name="element"/>
    /// to <paramref name="percent"/> of its height; whether there was one.</summary>
    internal bool ScrollTo(Element element, double percent)
    {
        var request = Handle("scrollTo", element);
        request["percent"] = percent;
        return (bool)Call(request)["ok"]!;
    }

    /// <summary>The app's open windows, the mailbox first.</summary>
    internal IReadOnlyList<AppWindow> Windows() =>
        Call(Op("windows"))["windows"]!.AsArray()
            .Select(w => new AppWindow((long)w!["hwnd"]!, (string)w["title"]!, (string)w["kind"]!))
            .ToList();

    /// <summary>Every element on screen, one indented line each, for a failure to say what was there.</summary>
    internal IReadOnlyList<string> Dump() =>
        Call(Op("dump"))["lines"]!.AsArray().Select(l => (string)l!).ToList();

    private static JsonObject Handle(string op, Element element) => new() { ["op"] = op, ["h"] = element.Handle };

    private static List<Element> Elements(JsonObject answer) =>
        answer["elements"]!.AsArray().Select(e => Element.From(e!.AsObject())).ToList();

    public void Dispose()
    {
        // The app is usually gone by now, and with it the far end: closing a dead pipe is not a failure.
        try
        {
            _writer.Dispose();
        }
        catch (Exception e) when (e is IOException or ObjectDisposedException)
        {
        }
        _reader.Dispose();
        _pipe.Dispose();
    }
}
