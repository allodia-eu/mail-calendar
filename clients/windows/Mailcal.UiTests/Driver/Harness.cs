// The local Stalwart harness (docker/stalwart, scripts/dev/harness.sh): whether it is up, and what a
// suite reads or writes on it as another client would. A meeting's real outcome is mail in somebody
// else's mailbox, which no surface of this app shows for an account it is not signed in to.

using System.Diagnostics;
using System.Net.Http.Headers;
using System.Net.Sockets;
using System.Text;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace Allodia.Mailcal.UiTests.Driver;

internal static class Harness
{
    internal const string Base = "http://127.0.0.1:28080";
    internal const string Alice = "alice@test.local";
    internal const string Bob = "bob@test.local";
    internal const string Dave = "dave@test.local";

    /// <summary>Whether the harness answers on its JMAP port. Asked once a run: every harness case
    /// asks while the run is being discovered, and a harness that is down costs the whole timeout
    /// each time.</summary>
    internal static bool IsUp() => Up.Value;

    private static readonly Lazy<bool> Up = new(() =>
    {
        try
        {
            using var client = new TcpClient();
            return client.ConnectAsync("127.0.0.1", 28080).Wait(1500);
        }
        catch (AggregateException)
        {
            return false;
        }
    });

    /// <summary>Stores <paramref name="message"/> in <paramref name="role"/> of alice's account,
    /// with <paramref name="keywords"/>.</summary>
    internal static async Task ImportAsync(string message, string role, params string[] keywords)
    {
        using var jmap = await Jmap.SignInAsync(Alice);
        var blob = await jmap.UploadAsync(Encoding.UTF8.GetBytes(message), "message/rfc822");
        var mailbox = (string)(await jmap.CallAsync("Mailbox/query", new JsonObject { ["filter"] = new JsonObject { ["role"] = role } }))["ids"]![0]!;
        var flags = new JsonObject();
        foreach (var keyword in keywords)
        {
            flags[keyword] = true;
        }
        var imported = await jmap.CallAsync("Email/import", new JsonObject
        {
            ["emails"] = new JsonObject
            {
                ["d"] = new JsonObject { ["blobId"] = blob, ["mailboxIds"] = new JsonObject { [mailbox] = true }, ["keywords"] = flags },
            },
        });
        if (imported["created"]?["d"] is null)
        {
            throw new InvalidOperationException($"the harness refused the message: {imported.ToJsonString()}");
        }
    }

    /// <summary>The subjects in <paramref name="address"/>'s mailbox whose message carries a
    /// text/calendar part.</summary>
    internal static IReadOnlyList<string> SchedulingSubjects(string address) => Run(async () =>
    {
        using var jmap = await Jmap.SignInAsync(address);
        var ids = (await jmap.CallAsync("Email/query", new JsonObject { ["limit"] = 100 }))["ids"]!.AsArray().DeepClone();
        var mails = await jmap.CallAsync("Email/get", new JsonObject { ["ids"] = ids, ["properties"] = new JsonArray("subject", "bodyStructure") });
        return (IReadOnlyList<string>)mails["list"]!.AsArray()
            .Where(mail => mail!["bodyStructure"]!.ToJsonString().Contains("text/calendar", StringComparison.OrdinalIgnoreCase))
            .Select(mail => (string?)mail!["subject"] ?? "")
            .ToList();
    });

    /// <summary>
    /// Waits until <paramref name="address"/> holds a scheduling message whose subject starts with
    /// <paramref name="prefix"/> and names <paramref name="title"/>. The prefix ("Invitation:",
    /// "Cancelled:", "Tentative:") is the server's own, in English whatever language the app runs in.
    /// </summary>
    internal static void WaitSchedulingMail(string address, string prefix, string title, int timeoutSec = 60)
    {
        if (!App.Eventually(() => HasSchedulingMail(address, prefix, title), TimeSpan.FromSeconds(timeoutSec)))
        {
            throw new InvalidOperationException($"no '{prefix} {title}' with a calendar part reached {address} within {timeoutSec} s");
        }
    }

    internal static bool HasSchedulingMail(string address, string prefix, string title) =>
        SchedulingSubjects(address).Any(subject => subject.StartsWith(prefix, StringComparison.Ordinal) && subject.Contains(title, StringComparison.Ordinal));

    /// <summary>
    /// Destroys every message in <paramref name="address"/>'s mailbox whose subject names
    /// <paramref name="title"/>. Other suites find their rows by sight, and a reply left at the top
    /// of an inbox pushes a seeded conversation below the fold, so a suite removes the mail it caused.
    /// </summary>
    internal static void RemoveMail(string address, string title) => Run(async () =>
    {
        using var jmap = await Jmap.SignInAsync(address);
        var ids = (await jmap.CallAsync("Email/query", new JsonObject { ["filter"] = new JsonObject { ["subject"] = title } }))["ids"]!.AsArray().DeepClone();
        await jmap.CallAsync("Email/set", new JsonObject { ["destroy"] = ids });
        return 0;
    });

    /// <summary>
    /// Sets the password the server holds for a seeded user, the way a person changing it elsewhere
    /// does: the account's stored secret stops working on its next request. A case that changes it
    /// puts the seeded one back in its cleanup, because every other harness suite signs in with it;
    /// a run that died first leaves <c>scripts/dev/harness.sh reset</c> as the way back.
    /// </summary>
    internal static void SetPassword(string user, string secret)
    {
        var description = user switch
        {
            "alice" => "Alice Tester",
            "bob" => "Bob Tester",
            _ => throw new ArgumentException($"no seeded harness user '{user}'", nameof(user)),
        };
        var account = new JsonObject
        {
            ["@type"] = "upsert",
            ["object"] = "Account",
            ["matchOn"] = new JsonArray("name"),
            ["value"] = new JsonObject
            {
                [user] = new JsonObject
                {
                    ["@type"] = "User",
                    ["name"] = user,
                    ["domainId"] = "#domain",
                    ["description"] = description,
                    ["credentials"] = new JsonObject { ["0"] = new JsonObject { ["@type"] = "Password", ["secret"] = secret } },
                    ["roles"] = new JsonObject { ["@type"] = "User" },
                },
            },
        };
        var plan = """{"@type":"upsert","object":"Domain","matchOn":["name"],"value":{"domain":{"name":"test.local"}}}"""
            + "\n" + account.ToJsonString();
        var (code, said) = Docker(plan,
            "exec", "-i", "-e", "HOME=/tmp", "mailcal-core-harness-stalwart-1",
            "stalwart-cli", "--no-color", "--url", "http://127.0.0.1:8080", "--user", "admin", "--password", "harness-admin-pw",
            "apply", "--quiet", "--stdin");
        if (code != 0)
        {
            throw new InvalidOperationException($"the harness refused {user}'s new password: {said}");
        }
    }

    /// <summary>Stops or starts the HTTPS front that serves the harness's sign-in discovery
    /// (docker/stalwart/README.md): stopped, a sign-in fails before any browser opens.</summary>
    internal static void SignInFront(string verb)
    {
        var (code, said) = Docker(null, "compose", "-p", "mailcal-core-harness", verb, "stalwart-oauth-front");
        if (code != 0)
        {
            throw new InvalidOperationException($"docker compose {verb} stalwart-oauth-front failed: {said}");
        }
    }

    /// <summary>Writes <paramref name="ics"/> straight into <paramref name="address"/>'s default
    /// calendar as <paramref name="name"/>.ics, the way seed-calendar-week.sh seeds a meeting.</summary>
    internal static void PutEvent(string address, string name, string ics) => Run(async () =>
    {
        using var dav = Dav(address);
        using var body = new StringContent(ics, Encoding.UTF8, "text/calendar");
        (await dav.PutAsync(EventUrl(address, name), body)).EnsureSuccessStatusCode();
        return 0;
    });

    /// <summary>Removes what <see cref="PutEvent"/> wrote; nothing when it is gone already.</summary>
    internal static void DeleteEvent(string address, string name) => Run(async () =>
    {
        using var dav = Dav(address);
        await dav.DeleteAsync(EventUrl(address, name));
        return 0;
    });

    private static string EventUrl(string address, string name) => $"{Base}/dav/cal/{Uri.EscapeDataString(address)}/default/{name}.ics";

    private static HttpClient Dav(string address)
    {
        var http = new HttpClient();
        http.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue(
            "Basic", Convert.ToBase64String(Encoding.ASCII.GetBytes($"{address}:harness-{address.Split('@')[0]}-pw")));
        return http;
    }

    private static (int Code, string Said) Docker(string? input, params string[] arguments)
    {
        var docker = new ProcessStartInfo("docker")
        {
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
        };
        foreach (var argument in arguments)
        {
            docker.ArgumentList.Add(argument);
        }
        using var process = Process.Start(docker) ?? throw new InvalidOperationException("docker did not start");
        process.StandardInput.Write(input ?? "");
        process.StandardInput.Close();
        var said = process.StandardOutput.ReadToEndAsync();
        var complained = process.StandardError.ReadToEndAsync();
        if (!process.WaitForExit(60_000))
        {
            process.Kill();
            return (-1, "docker did not finish within 60 s");
        }
        return (process.ExitCode, said.Result + complained.Result);
    }

    private static T Run<T>(Func<Task<T>> work) => Task.Run(work).GetAwaiter().GetResult();

    /// <summary>One seeded user's JMAP session. Its URLs are advertised under a hostname this machine
    /// does not resolve, so only their paths are used.</summary>
    private sealed class Jmap : IDisposable
    {
        private readonly HttpClient _http = new();
        private string _account = "";
        private string _api = "";
        private string _upload = "";

        internal static async Task<Jmap> SignInAsync(string address)
        {
            var jmap = new Jmap();
            var user = address.Split('@')[0];
            jmap._http.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue(
                "Basic", Convert.ToBase64String(Encoding.ASCII.GetBytes($"{address}:harness-{user}-pw")));
            var session = JsonNode.Parse(await jmap._http.GetStringAsync($"{Base}/jmap/session"))!;
            jmap._account = (string)session["primaryAccounts"]!["urn:ietf:params:jmap:mail"]!;
            jmap._api = jmap.Local((string)session["apiUrl"]!);
            jmap._upload = jmap.Local((string)session["uploadUrl"]!);
            return jmap;
        }

        private string Local(string url) => Regex.Replace(url, "^https?://[^/]+", Base).Replace("{accountId}", _account);

        internal async Task<JsonNode> CallAsync(string method, JsonObject arguments)
        {
            arguments["accountId"] = _account;
            var body = new JsonObject
            {
                ["using"] = new JsonArray("urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"),
                ["methodCalls"] = new JsonArray(new JsonArray(method, arguments, "c")),
            };
            using var content = new StringContent(body.ToJsonString(), Encoding.UTF8, "application/json");
            var answer = await _http.PostAsync(_api, content);
            return JsonNode.Parse(await answer.Content.ReadAsStringAsync())!["methodResponses"]![0]![1]!;
        }

        internal async Task<string> UploadAsync(byte[] bytes, string type)
        {
            using var upload = new ByteArrayContent(bytes);
            upload.Headers.ContentType = new MediaTypeHeaderValue(type);
            var uploaded = await _http.PostAsync(_upload, upload);
            return (string)JsonNode.Parse(await uploaded.Content.ReadAsStringAsync())!["blobId"]!;
        }

        public void Dispose() => _http.Dispose();
    }
}
