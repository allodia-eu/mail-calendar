// What a person does in the browser during a Nextcloud sign-in against the engine's fixture: open
// the login page, sign in, grant access. The app opens the page in the default browser, which a
// suite cannot drive, so this does the same requests that page makes, as the core's live suite does
// (crates/mailcal-bindings/tests/live_nextcloud_sign_in.rs, grant_in_the_browser). The page to
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
        var psql = Process.Start(new ProcessStartInfo("docker",
            ["exec", Database, "psql", "-U", "nextcloud", "-d", "nextcloud", "-tAc",
             "select login_token from oc_login_flow_v2 order by id desc limit 1"])
        {
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        })!;
        var token = psql.StandardOutput.ReadToEnd().Trim();
        psql.WaitForExit();
        return psql.ExitCode == 0 && token.Length > 0 ? token : null;
    }

    /// <summary>Waits for a sign-in newer than <paramref name="before"/> and grants it as
    /// <paramref name="user"/>.</summary>
    internal static void GrantNext(string root, string? before, string user, string password)
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
        Grant($"{root}/login/v2/flow/{token}", root, user, password).GetAwaiter().GetResult();
    }

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
