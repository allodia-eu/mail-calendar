// The suites' side of the app's test agent (Mailcal/Services/TestAgent.cs), which speaks the
// protocol every client's agent does (docs/test-agent.md): one JSON object a line each way over a
// named pipe. Finding and acting happen inside the app, on its UI thread, through the automation
// peers UI Automation reads.

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
        if (answer["ok"] is not JsonValue ok || !(bool)ok)
        {
            throw new AgentRefusal($"the test agent refused '{request["verb"]}': {answer["error"]}", (string?)answer["code"]);
        }
        return answer;
    }

    private static JsonObject Verb(string verb, Filter? filter = null)
    {
        var request = filter?.ToJson() ?? new JsonObject();
        request["verb"] = verb;
        return request;
    }

    private static JsonObject Handle(string verb, Element element) => new() { ["verb"] = verb, ["h"] = element.Handle };

    /// <summary>The first element the filter accepts, or null.</summary>
    internal Element? Find(Filter filter)
    {
        var request = Verb("query", filter);
        request["index"] = 0;
        return Elements(Call(request)).FirstOrDefault();
    }

    /// <summary>Every element the filter accepts, in the order UI Automation reports them.</summary>
    internal IReadOnlyList<Element> FindAll(Filter filter) => Elements(Call(Verb("query", filter)));

    /// <summary>Waits for the filter to accept something, checked on every turn of the app's UI
    /// thread; null when <paramref name="timeout"/> runs out first.</summary>
    internal Element? Wait(Filter filter, TimeSpan timeout)
    {
        var request = Verb("wait", filter);
        request["index"] = 0;
        request["timeoutMs"] = (int)timeout.TotalMilliseconds;
        try
        {
            return Elements(Call(request)).First();
        }
        catch (AgentRefusal e) when (e.Code == "timeout")
        {
            return null;
        }
    }

    /// <summary>Waits for the filter to accept nothing; whether it did within <paramref name="timeout"/>.</summary>
    internal bool WaitGone(Filter filter, TimeSpan timeout)
    {
        var request = Verb("wait", filter);
        request["absent"] = true;
        request["timeoutMs"] = (int)timeout.TotalMilliseconds;
        try
        {
            Call(request);
            return true;
        }
        catch (AgentRefusal e) when (e.Code == "timeout")
        {
            return false;
        }
    }

    /// <summary>Waits until the windows have looked the same for <paramref name="quiet"/>, and at
    /// least <paramref name="floor"/> has passed; whether that happened within <paramref name="cap"/>.</summary>
    internal bool Settle(TimeSpan? quiet = null, TimeSpan? floor = null, TimeSpan? cap = null)
    {
        var request = Verb("settle");
        request["quietMs"] = (int)(quiet ?? TimeSpan.FromMilliseconds(150)).TotalMilliseconds;
        request["floorMs"] = (int)(floor ?? TimeSpan.Zero).TotalMilliseconds;
        request["capMs"] = (int)(cap ?? TimeSpan.FromSeconds(2)).TotalMilliseconds;
        return (bool)Call(request)["settled"]!;
    }

    /// <summary>Has the app collect, as the release probe does while a suite runs.</summary>
    internal void Collect() => Call(Verb("collect"));

    /// <summary>The element's state now: its value, toggle, selection and focus as well.</summary>
    internal ElementState Read(Element element) => ElementState.From(Call(Handle("read", element)));

    /// <summary>Presses the element: Invoke, or Select where it cannot be invoked. A control that is
    /// disabled is waited for, as a person waits for one, because the agent refuses to press it.</summary>
    internal void Invoke(Element element)
    {
        if (!App.Eventually(() => Read(element).Element.Enabled, TimeSpan.FromSeconds(5)))
        {
            throw new InvalidOperationException($"'{element.Name}' (#{element.Id}) stayed disabled, so it cannot be pressed");
        }
        Call(Handle("activate", element));
    }

    internal void Select(Element element) => Call(Handle("select", element));

    /// <summary>Sets a text box's text, which the box takes as an edit.</summary>
    internal void SetText(Element element, string text)
    {
        var request = Handle("set-text", element);
        request["text"] = text;
        Call(request);
    }

    /// <summary>Toggles a check box or toggle button; whether it is on afterwards.</summary>
    internal bool Toggle(Element element) => (bool)Call(Handle("toggle", element))["active"]!;

    /// <summary>Puts a check box or toggle button <paramref name="on"/> or off, whichever it was.</summary>
    internal void SetToggle(Element element, bool on)
    {
        var request = Handle("toggle", element);
        request["active"] = on;
        Call(request);
    }

    internal void ScrollIntoView(Element element) => Call(Handle("scrollIntoView", element));

    /// <summary>Opens a combo box, a drop-down or a tree row. A tree row has nothing to open until it
    /// has rows under it, and an account's arrive from its server after the account's own, so a
    /// row the agent refuses as not expandable is waited for, as <see cref="Invoke"/> waits for a
    /// disabled control. Anything else that does not expand is refused at once.</summary>
    internal void Expand(Element element)
    {
        if (element.Type is not ("ListItem" or "TreeItem"))
        {
            Call(Handle("expand", element));
            return;
        }
        AgentRefusal? refused = null;
        bool Opened()
        {
            try
            {
                Call(Handle("expand", element));
                return true;
            }
            catch (AgentRefusal e) when (e.Code == "not-expandable")
            {
                refused = e;
                return false;
            }
        }
        if (!App.Eventually(Opened, TimeSpan.FromSeconds(30)))
        {
            throw refused!;
        }
    }

    /// <summary>Closes what <see cref="Expand"/> opened.</summary>
    internal void Collapse(Element element) => Call(Handle("collapse", element));

    internal void Focus(Element element) => Call(Handle("focus", element));

    /// <summary>The nearest ancestor of <paramref name="element"/> that has a peer, or null.</summary>
    internal Element? Parent(Element element) => Elements(Call(Handle("parent", element))).FirstOrDefault();

    /// <summary>The element that has the keyboard, or null when nothing in the app does.</summary>
    internal Element? Focused() => Elements(Call(Verb("focused"))).FirstOrDefault();

    /// <summary>Scrolls the first vertically scrollable element at or under <paramref name="element"/>
    /// to its end; whether there was one.</summary>
    internal bool ScrollToEnd(Element element) => ScrollTo(element, 100);

    /// <summary>Scrolls the first vertically scrollable element at or under <paramref name="element"/>
    /// to <paramref name="percent"/> of its height; whether there was one.</summary>
    internal bool ScrollTo(Element element, double percent)
    {
        var request = Handle("scrollTo", element);
        request["percent"] = percent;
        return (bool)Call(request)["scrolled"]!;
    }

    /// <summary>The app's open windows, the mailbox first.</summary>
    internal IReadOnlyList<AppWindow> Windows() =>
        Call(Verb("windows"))["windows"]!.AsArray()
            .Select(w => new AppWindow((long)w!["h"]!, (string)w["title"]!, (string)w["kind"]!))
            .ToList();

    /// <summary>Every element, offscreen ones included, one indented line each, for a failure to
    /// say what was there.</summary>
    internal IReadOnlyList<string> Dump()
    {
        var request = Verb("dump");
        request["hidden"] = true;
        return ((string)Call(request)["text"]!).Split('\n', StringSplitOptions.RemoveEmptyEntries);
    }

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

/// <summary>A request the test agent refused. <see cref="Code"/> names the refusal
/// (docs/test-agent.md, "Refusals"), so a suite never matches on the words.</summary>
internal sealed class AgentRefusal(string message, string? code) : InvalidOperationException(message)
{
    internal string? Code { get; } = code;
}
