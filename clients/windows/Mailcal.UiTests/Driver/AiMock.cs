// The canned AI endpoint the writing-style suites learn against (scripts/dev/ai-mock.py): an
// OpenAI-compatible server on loopback whose answers are fixed, so nothing a suite learns from
// leaves the machine. Started with the repository's Python, stopped with the suite.

using System.Diagnostics;
using System.Net.Sockets;

namespace Allodia.Mailcal.UiTests.Driver;

internal sealed class AiMock : IDisposable
{
    internal const int Port = 28434;

    /// <summary>The address the endpoint form is given: plain HTTP, which the core admits only to
    /// this device.</summary>
    internal const string Address = "http://127.0.0.1:28434/v1";

    private Process? _process;

    /// <summary>Starts the endpoint with <paramref name="arguments"/> (<c>--delay SECONDS</c>,
    /// <c>--status CODE</c>), in place of one already running, and waits until it listens.</summary>
    internal void Start(params string[] arguments)
    {
        Stop();
        var script = Path.Combine(App.Repo, "scripts", "dev", "ai-mock.py");
        var start = new ProcessStartInfo("python") { UseShellExecute = false, RedirectStandardOutput = true, RedirectStandardError = true };
        foreach (var argument in new[] { "-I", script, "--port", Port.ToString(System.Globalization.CultureInfo.InvariantCulture) }.Concat(arguments))
        {
            start.ArgumentList.Add(argument);
        }
        _process = Process.Start(start) ?? throw new InvalidOperationException("the canned AI endpoint did not start");
        _process.OutputDataReceived += (_, _) => { };
        _process.ErrorDataReceived += (_, _) => { };
        _process.BeginOutputReadLine();
        _process.BeginErrorReadLine();
        if (!App.Eventually(Listening, TimeSpan.FromSeconds(15)))
        {
            throw new InvalidOperationException($"the canned AI endpoint never listened on 127.0.0.1:{Port}");
        }
    }

    internal void Stop()
    {
        if (_process is { HasExited: false } running)
        {
            running.Kill(entireProcessTree: true);
            running.WaitForExit(5000);
        }
        _process?.Dispose();
        _process = null;
    }

    public void Dispose() => Stop();

    private static bool Listening()
    {
        try
        {
            using var client = new TcpClient();
            client.Connect("127.0.0.1", Port);
            return true;
        }
        catch (SocketException)
        {
            return false;
        }
    }
}
