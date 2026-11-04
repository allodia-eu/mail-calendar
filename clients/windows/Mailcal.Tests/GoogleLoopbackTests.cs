// The Google sign-in's loopback listener, against a real HttpListener on 127.0.0.1. A browser can
// ask the port for something else before it delivers the redirect, such as /favicon.ico, and
// taking that request failed a sign-in the person completed. And a browser can come back after the
// wait is over, which must read a page rather than a refused connection.
//
// The rule for what counts as the redirect is the core's (is_oauth_callback, tested in
// mailcal-oauth); no cdylib is loaded here, so a stand-in for it is passed, and what is pinned is
// what the listener does with its answer.
using System;
using System.Net;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class GoogleLoopbackTests
{
    private static readonly LoopbackPages Pages = new("Mail & Calendar", "Done <here>", "Ended");

    private static GoogleLoopback NewLoopback(TimeSpan? lingering = null) =>
        new(Pages, url => url.Contains("state=", StringComparison.Ordinal), lingering);

    private static HttpClient NewBrowser() => new(new HttpClientHandler { UseProxy = false })
    {
        Timeout = TimeSpan.FromSeconds(10),
    };

    // A listener that stops answering hangs a test rather than failing it, so every wait is bounded.
    private static Task<T> Bounded<T>(Task<T> task) => task.WaitAsync(TimeSpan.FromSeconds(10), Token);

    private static CancellationToken Token => TestContext.Current.CancellationToken;

    private static string Page(string message) =>
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Mail &amp; Calendar</title></head>"
        + "<body style=\"font-family: system-ui, sans-serif; text-align: center; padding: 3rem;\">"
        + $"<p>{message}</p></body></html>";

    [Fact]
    public async Task A_request_that_is_not_the_redirect_is_answered_and_the_wait_goes_on()
    {
        using var loopback = NewLoopback();
        using var browser = NewBrowser();
        var wait = loopback.WaitForCallbackAsync(CancellationToken.None);

        var favicon = await Bounded(browser.GetAsync(loopback.RedirectUri + "favicon.ico", Token));
        Assert.Equal(HttpStatusCode.BadRequest, favicon.StatusCode);
        Assert.False(wait.IsCompleted);

        var redirect = await Bounded(browser.GetAsync(loopback.RedirectUri + "?state=s&code=c", Token));
        Assert.Equal(HttpStatusCode.OK, redirect.StatusCode);
        Assert.Equal(Page("Done &lt;here&gt;"), await redirect.Content.ReadAsStringAsync(Token));
        Assert.Equal(loopback.RedirectUri + "?state=s&code=c", await Bounded(wait));
    }

    [Fact]
    public async Task The_redirect_reaches_the_core_as_the_browser_escaped_it()
    {
        using var loopback = NewLoopback();
        using var browser = NewBrowser();
        var wait = loopback.WaitForCallbackAsync(CancellationToken.None);

        // Uri.ToString() would hand over `scope=email openid`, a URL with a space in it.
        const string query = "?state=s&code=4%2F0A%2Bb&scope=email%20openid";
        await Bounded(browser.GetAsync(loopback.RedirectUri + query, Token));
        Assert.Equal(loopback.RedirectUri + query, await Bounded(wait));
    }

    [Fact]
    public async Task A_redirect_after_the_sign_in_was_cancelled_reads_that_it_is_no_longer_waiting()
    {
        using var browser = NewBrowser();
        string redirectUri;
        using (var loopback = NewLoopback())
        {
            redirectUri = loopback.RedirectUri;
            using var cancel = new CancellationTokenSource();
            var wait = loopback.WaitForCallbackAsync(cancel.Token);
            await cancel.CancelAsync();
            await Assert.ThrowsAsync<OperationCanceledException>(() => Bounded(wait));
        }

        var late = await Bounded(browser.GetAsync(redirectUri + "?state=s&code=c", Token));
        Assert.Equal(HttpStatusCode.OK, late.StatusCode);
        Assert.Equal(Page("Ended"), await late.Content.ReadAsStringAsync(Token));
    }

    [Fact]
    public async Task A_browser_that_comes_back_after_the_redirect_reads_that_the_sign_in_finished()
    {
        using var browser = NewBrowser();
        string redirectUri;
        using (var loopback = NewLoopback())
        {
            redirectUri = loopback.RedirectUri;
            var wait = loopback.WaitForCallbackAsync(CancellationToken.None);
            await Bounded(browser.GetAsync(redirectUri + "?state=s&code=c", Token));
            await Bounded(wait);
        }

        var again = await Bounded(browser.GetAsync(redirectUri + "?state=s&code=c", Token));
        Assert.Equal(Page("Done &lt;here&gt;"), await again.Content.ReadAsStringAsync(Token));
    }

    [Fact]
    public async Task The_port_closes_once_the_lingering_time_has_passed()
    {
        using var browser = NewBrowser();
        string redirectUri;
        using (var loopback = NewLoopback(TimeSpan.FromMilliseconds(200)))
        {
            redirectUri = loopback.RedirectUri;
        }

        await Task.Delay(TimeSpan.FromSeconds(1), Token);
        await Assert.ThrowsAnyAsync<HttpRequestException>(() => Bounded(browser.GetAsync(redirectUri, Token)));
    }
}
