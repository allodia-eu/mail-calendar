// The local Stalwart harness (docker/stalwart, scripts/dev/harness.sh): whether it is up, and the
// writes a suite makes on it as another client would.

using System.Net.Http.Headers;
using System.Net.Sockets;
using System.Text;
using System.Text.Json.Nodes;

namespace Allodia.Mailcal.UiTests.Driver;

internal static class Harness
{
    internal const string Base = "http://127.0.0.1:28080";
    internal const string Alice = "alice@test.local";
    internal const string AlicePassword = "harness-alice-pw";
    internal const string Bob = "bob@test.local";

    /// <summary>Whether the harness answers on its JMAP port.</summary>
    internal static bool IsUp()
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
    }

    /// <summary>
    /// Stores <paramref name="message"/> in <paramref name="role"/> of alice's account over JMAP,
    /// with <paramref name="keywords"/>. The harness advertises its URLs under a hostname this
    /// machine does not resolve, so only their paths are used.
    /// </summary>
    internal static async Task ImportAsync(string message, string role, params string[] keywords)
    {
        using var http = new HttpClient();
        http.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue(
            "Basic", Convert.ToBase64String(Encoding.ASCII.GetBytes($"{Alice}:{AlicePassword}")));
        var session = JsonNode.Parse(await http.GetStringAsync($"{Base}/jmap/session"))!;
        var account = (string)session["primaryAccounts"]!["urn:ietf:params:jmap:mail"]!;
        string Local(string url) =>
            System.Text.RegularExpressions.Regex.Replace(url, "^https?://[^/]+", Base).Replace("{accountId}", account);

        using var upload = new ByteArrayContent(Encoding.UTF8.GetBytes(message));
        upload.Headers.ContentType = new MediaTypeHeaderValue("message/rfc822");
        var uploaded = await http.PostAsync(Local((string)session["uploadUrl"]!), upload);
        var blob = (string)JsonNode.Parse(await uploaded.Content.ReadAsStringAsync())!["blobId"]!;

        async Task<JsonNode> Call(string method, JsonObject arguments)
        {
            var body = new JsonObject
            {
                ["using"] = new JsonArray("urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"),
                ["methodCalls"] = new JsonArray(new JsonArray(method, arguments, "c")),
            };
            using var content = new StringContent(body.ToJsonString(), Encoding.UTF8, "application/json");
            var answer = await http.PostAsync(Local((string)session["apiUrl"]!), content);
            return JsonNode.Parse(await answer.Content.ReadAsStringAsync())!["methodResponses"]![0]![1]!;
        }

        var mailbox = (string)(await Call("Mailbox/query", new JsonObject
        {
            ["accountId"] = account,
            ["filter"] = new JsonObject { ["role"] = role },
        }))["ids"]![0]!;
        var flags = new JsonObject();
        foreach (var keyword in keywords)
        {
            flags[keyword] = true;
        }
        var imported = await Call("Email/import", new JsonObject
        {
            ["accountId"] = account,
            ["emails"] = new JsonObject
            {
                ["d"] = new JsonObject
                {
                    ["blobId"] = blob,
                    ["mailboxIds"] = new JsonObject { [mailbox] = true },
                    ["keywords"] = flags,
                },
            },
        });
        if (imported["created"]?["d"] is null)
        {
            throw new InvalidOperationException($"the harness refused the message: {imported.ToJsonString()}");
        }
    }
}
