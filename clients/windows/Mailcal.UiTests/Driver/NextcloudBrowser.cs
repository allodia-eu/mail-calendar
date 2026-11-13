// What a person does in the browser during a Nextcloud sign-in against the engine's fixture: open
// the login page, sign in, grant access. A suite cannot drive the default browser, and its launch
// keeps the app from opening one (MAILCAL_NO_EXTERNAL_OPEN), so this does the requests that page
// makes, as the core's live suite does (crates/mailcal-bindings/tests/live/live_nextcloud_sign_in.rs,
// grant_in_the_browser). The page to
// open is read from the fixture's own database: the app logs no URL (docs/logging.md).

using System.Diagnostics;
using System.Net;
using System.Text.Json;

namespace Allodia.Mailcal.UiTests.Driver;

internal static class NextcloudBrowser
{
    private const string Database = "pim-nextcloud-harness-db";

    /// <summary>The newest sign-in the fixture has started, by its login token; null when none.</summary>
    internal static string? NewestLoginToken()
    {
        var (exit, token) = Sql("select login_token from oc_login_flow_v2 order by id desc limit 1");
        return exit == 0 && token.Length > 0 ? token : null;
    }

    /// <summary>
    /// Clears the passwords the fixture has refused. Nextcloud counts every refusal from an address
    /// and, past a handful, delays and then refuses (HTTP 429) everything from it, and every
    /// request here comes from Docker's one gateway address. A suite refuses passwords on purpose,
    /// so without this a case fails for the cases that ran before it.
    /// </summary>
    internal static void ForgetRefusals()
    {
        if (Sql("delete from oc_bruteforce_attempts").Exit != 0)
        {
            throw new InvalidOperationException("the fixture's refused sign-ins could not be cleared");
        }
    }

    private static (int Exit, string Output) Sql(string statement)
    {
        var psql = Process.Start(new ProcessStartInfo("docker",
            ["exec", Database, "psql", "-U", "nextcloud", "-d", "nextcloud", "-tAc", statement])
        {
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        })!;
        var output = psql.StandardOutput.ReadToEnd().Trim();
        psql.WaitForExit();
        return (psql.ExitCode, output);
    }

    /// <summary>Waits for a sign-in newer than <paramref name="before"/> and grants it as
    /// <paramref name="user"/>.</summary>
    internal static void GrantNext(string root, string? before, string user, string password) =>
        GrantToken(root, NextLoginToken(before), user, password);

    /// <summary>Waits for a sign-in newer than <paramref name="before"/> to start at the fixture and
    /// returns its login token.</summary>
    internal static string NextLoginToken(string? before)
    {
        string? token = null;
        var deadline = DateTime.UtcNow.AddSeconds(20);
        while ((token = NewestLoginToken()) == before || token is null)
        {
            if (DateTime.UtcNow > deadline)
            {
                throw new InvalidOperationException("the app started no sign-in at the fixture");
            }
            Thread.Sleep(250);
        }
        return token;
    }

    /// <summary>Grants the sign-in <paramref name="token"/> names as <paramref name="user"/>,
    /// whether or not the app still waits on it.</summary>
    internal static void GrantToken(string root, string token, string user, string password) =>
        Grant($"{root}/login/v2/flow/{token}", root, user, password).GetAwaiter().GetResult();

    private static async Task Grant(string loginUrl, string root, string user, string password)
    {
        using var handler = new HttpClientHandler { CookieContainer = new CookieContainer(), AllowAutoRedirect = true };
        using var http = new HttpClient(handler) { Timeout = TimeSpan.FromSeconds(30) };
        var index = $"{root}/index.php";
        var picker = await http.GetStringAsync(loginUrl);
        var redirect = new Uri(InitialState(picker, "loginFlowAuth").GetProperty("loginRedirectUrl").GetString()!);
        var back = redirect.PathAndQuery;
        var form = await http.GetStringAsync($"{index}/login?redirect_url={Uri.EscapeDataString(back)}");
        var granting = await Post(http, $"{index}/login", root, null, new()
        {
            ["user"] = user,
            ["password"] = password,
            ["requesttoken"] = RequestToken(form),
            ["redirect_url"] = back,
            ["timezone"] = "Europe/Amsterdam",
            ["timezone_offset"] = "1",
        });
        var grant = InitialState(granting, "loginFlowGrant");
        var done = await Post(http, grant.GetProperty("actionUrl").GetString()!, root, RequestToken(granting), new()
        {
            ["stateToken"] = grant.GetProperty("stateToken").GetString()!,
        });
        if (!done.Contains("loginFlowState", StringComparison.Ordinal))
        {
            throw new InvalidOperationException("the fixture did not accept the grant");
        }
    }

    private static async Task<string> Post(HttpClient http, string url, string origin, string? token, Dictionary<string, string> form)
    {
        using var request = new HttpRequestMessage(HttpMethod.Post, url) { Content = new FormUrlEncodedContent(form) };
        request.Headers.Add("Origin", origin);
        if (token is not null)
        {
            request.Headers.Add("requesttoken", token);
        }
        using var response = await http.SendAsync(request);
        response.EnsureSuccessStatusCode();
        return await response.Content.ReadAsStringAsync();
    }

    private static JsonElement InitialState(string page, string key)
    {
        var marker = $"id=\"initial-state-core-{key}\" value=\"";
        var start = page.IndexOf(marker, StringComparison.Ordinal);
        if (start < 0)
        {
            throw new InvalidOperationException($"no {key} on the fixture's page");
        }
        start += marker.Length;
        var value = WebUtility.HtmlDecode(page[start..page.IndexOf('"', start)]);
        return JsonDocument.Parse(Convert.FromBase64String(value)).RootElement.Clone();
    }

    private static string RequestToken(string page)
    {
        const string marker = "data-requesttoken=\"";
        var start = page.IndexOf(marker, StringComparison.Ordinal) + marker.Length;
        return WebUtility.HtmlDecode(page[start..page.IndexOf('"', start)]);
    }
}
