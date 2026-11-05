// A loopback stand-in for a Nextcloud server, for the suites about signing in with Nextcloud
// (docs/nextcloud-sign-in.md): it answers status.php as Nextcloud, starts Login Flow v2, answers the
// poll as waiting or with a grant, refuses every DAV request, and records each request line, so a
// case can say what the app asked and when. The shapes are the core's own stand-in's
// (crates/mailcal-bindings/src/nextcloud_mock.rs).
//
// Plain TCP rather than HttpListener, which needs a URL reservation to bind 127.0.0.1 without
// elevation.

using System.Net;
using System.Net.Sockets;
using System.Text;

namespace Allodia.Mailcal.UiTests.Driver;

internal sealed class NextcloudStandIn : IDisposable
{
    private readonly TcpListener _listener = new(IPAddress.Loopback, 0);
    private readonly List<string> _requests = [];
    private readonly CancellationTokenSource _stop = new();

    /// <param name="nextcloud">Whether status.php names Nextcloud; otherwise it is not found, as on
    /// any other DAV server.</param>
    internal NextcloudStandIn(bool nextcloud = true)
    {
        Nextcloud = nextcloud;
        _listener.Start();
        Base = $"http://127.0.0.1:{((IPEndPoint)_listener.LocalEndpoint).Port}";
        _ = Task.Run(ServeAsync);
    }

    /// <summary>The server's root, <c>http://127.0.0.1:port</c>.</summary>
    internal string Base { get; }

    /// <summary>The DAV endpoint a person would type.</summary>
    internal string Dav => $"{Base}/remote.php/dav/";

    private bool Nextcloud { get; }

    /// <summary>Whether the poll grants; until then it answers that the person has not finished.</summary>
    internal bool Granting { get; set; }

    /// <summary>The request lines read so far that start with <paramref name="start"/>.</summary>
    internal int Count(string start)
    {
        lock (_requests)
        {
            return _requests.Count(request => request.StartsWith(start, StringComparison.Ordinal));
        }
    }

    internal const string Status = "GET /status.php ";
    internal const string Begin = "POST /index.php/login/v2 ";
    internal const string Poll = "POST /login/v2/poll ";
    internal const string Withdraw = "DELETE /ocs/v2.php/core/apppassword ";

    private async Task ServeAsync()
    {
        while (!_stop.IsCancellationRequested)
        {
            TcpClient client;
            try
            {
                client = await _listener.AcceptTcpClientAsync(_stop.Token);
            }
            catch (Exception)
            {
                return;
            }
            _ = Task.Run(() => Answer(client));
        }
    }

    private void Answer(TcpClient client)
    {
        using var _ = client;
        var stream = client.GetStream();
        var line = ReadRequestLine(stream);
        lock (_requests)
        {
            _requests.Add(line);
        }
        var (status, body) = Route(line);
        var bytes = Encoding.UTF8.GetBytes(body);
        var head = $"HTTP/1.1 {status} {(status == 200 ? "OK" : status == 401 ? "Unauthorized" : "Not Found")}\r\n"
            + $"Content-Length: {bytes.Length}\r\nConnection: close\r\n"
            + (status == 401 ? "WWW-Authenticate: Basic realm=\"Nextcloud\"\r\n" : "")
            + (body.StartsWith('<') ? "Content-Type: text/html\r\n" : "Content-Type: application/json\r\n")
            + "\r\n";
        try
        {
            stream.Write(Encoding.ASCII.GetBytes(head));
            stream.Write(bytes);
        }
        catch (IOException)
        {
            // The app gave up on the answer; what it asked is recorded.
        }
    }

    private (int Status, string Body) Route(string line)
    {
        if (line.StartsWith(Status, StringComparison.Ordinal))
        {
            return Nextcloud
                ? (200, """{"installed":true,"maintenance":false,"version":"35.0.2.2","productname":"Nextcloud"}""")
                : (404, "");
        }
        if (!Nextcloud)
        {
            return (404, "");
        }
        if (line.StartsWith(Begin, StringComparison.Ordinal))
        {
            return (200, $$"""{"poll":{"token":"poll-token","endpoint":"{{Base}}/login/v2/poll"},"login":"{{Base}}/login/v2/flow/login-token"}""");
        }
        if (line.StartsWith(Poll, StringComparison.Ordinal))
        {
            return Granting
                ? (200, $$"""{"server":"{{Base}}","loginName":"f3a9c1","appPassword":"stand-in-app-password"}""")
                : (404, "[]");
        }
        if (line.StartsWith("GET /login/v2/flow/", StringComparison.Ordinal))
        {
            return (200, "<!doctype html><title>Stand-in</title><p>A test server standing in for Nextcloud. Close this tab.</p>");
        }
        if (line.StartsWith("GET /ocs/v2.php/cloud/user", StringComparison.Ordinal))
        {
            return (200, """{"ocs":{"data":{"id":"alice","email":"alice@example.org"}}}""");
        }
        if (line.StartsWith(Withdraw, StringComparison.Ordinal))
        {
            return (200, "");
        }
        // Every DAV request: the app password is refused, so no account is made of a grant.
        return (401, "");
    }

    /// <summary>Reads the request line and drains the rest of the request, body included.</summary>
    private static string ReadRequestLine(NetworkStream stream)
    {
        var data = new List<byte>();
        var buffer = new byte[4096];
        stream.ReadTimeout = 5000;
        try
        {
            while (true)
            {
                var read = stream.Read(buffer, 0, buffer.Length);
                if (read == 0)
                {
                    break;
                }
                data.AddRange(buffer.AsSpan(0, read).ToArray());
                var text = Encoding.ASCII.GetString(data.ToArray());
                var end = text.IndexOf("\r\n\r\n", StringComparison.Ordinal);
                if (end < 0)
                {
                    continue;
                }
                var length = text[..end].Split("\r\n")
                    .Select(header => header.Split(':', 2))
                    .Where(pair => pair.Length == 2 && pair[0].Trim().Equals("content-length", StringComparison.OrdinalIgnoreCase))
                    .Select(pair => int.Parse(pair[1].Trim(), System.Globalization.CultureInfo.InvariantCulture))
                    .FirstOrDefault();
                if (data.Count >= end + 4 + length)
                {
                    break;
                }
            }
        }
        catch (IOException)
        {
            // A request cut short is still recorded by its line.
        }
        var all = Encoding.ASCII.GetString(data.ToArray());
        var first = all.IndexOf("\r\n", StringComparison.Ordinal);
        return (first < 0 ? all : all[..first]) + " ";
    }

    public void Dispose()
    {
        _stop.Cancel();
        _listener.Stop();
        _stop.Dispose();
    }
}
