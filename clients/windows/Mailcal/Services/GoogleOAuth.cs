// The OS-secure-store sink and the loopback redirect listener for the Google (Gmail + Google
// Calendar) OAuth sign-in on Windows. The Desktop OAuth client itself, its id and its
// non-confidential secret, is injected into the core at build time and never appears here;
// there is nothing about the redirect to configure either, because that client type accepts any
// http://127.0.0.1:<port> loopback and the listener picks a free port per sign-in. Unlike the
// Microsoft path, which returns through a custom-scheme protocol activation (see
// MicrosoftOAuth.cs / Program.cs), Google's recommended Desktop flow redirects to an
// http://127.0.0.1 loopback address, so this host runs a one-shot HttpListener on a free
// ephemeral port and reads the redirect straight off it. The Rust core owns the OAuth state
// machine (PKCE, code exchange, refresh); this host owns only picking the port, opening the
// authorization URL in the user's default browser, and catching the loopback redirect.

using System;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace Allodia.Mailcal.Services;

/// <summary>
/// What the loopback port says to the browser, from the catalog: the page's title, the page after
/// the redirect, and the page once the wait ended without one.
/// </summary>
internal sealed record LoopbackPages(string Title, string Done, string Ended);

/// <summary>
/// A one-shot loopback redirect listener for the Google Desktop OAuth flow. Construction picks a
/// free ephemeral port on 127.0.0.1 and starts an <see cref="HttpListener"/> on it;
/// <see cref="RedirectUri"/> is the exact address to hand <c>begin_google_login</c> (it MUST match
/// what the browser returns to). <see cref="WaitForCallbackAsync"/> awaits the redirect, the one
/// request <c>isRedirect</c> (the core's <c>is_oauth_callback</c>) accepts, answers it with
/// <see cref="LoopbackPages.Done"/> and returns its full URL; any other request is answered with
/// a 400. Dispose keeps the port answering for the lingering time with a page saying the sign-in
/// finished or is no longer waiting, so a browser that comes back late reads that rather than a
/// refused connection, and then stops the listener.
/// </summary>
internal sealed class GoogleLoopback : IDisposable
{
    private static readonly TimeSpan DefaultLingering = TimeSpan.FromMinutes(3);

    private readonly HttpListener _listener = new();
    private readonly LoopbackPages _pages;
    private readonly Func<string, bool> _isRedirect;
    private readonly TimeSpan _lingering;

    // The accept still outstanding when a wait ends, so the lingering answer takes it over.
    private Task<HttpListenerContext>? _pending;

    // What the port says once the wait is over: finished after a redirect, else no longer waiting.
    private string _afterwards;

    private bool _disposed;

    /// <summary>
    /// The loopback redirect URI the browser returns to, pass this verbatim to
    /// <c>begin_google_login</c>. A trailing slash is required by HttpListener's prefix grammar,
    /// which Google accepts for a Desktop client's loopback redirect.
    /// </summary>
    public string RedirectUri { get; }

    public GoogleLoopback(LoopbackPages pages, Func<string, bool> isRedirect, TimeSpan? lingering = null)
    {
        _pages = pages;
        _isRedirect = isRedirect;
        _lingering = lingering ?? DefaultLingering;
        _afterwards = pages.Ended;
        var port = PickFreePort();
        RedirectUri = $"http://127.0.0.1:{port}/";
        _listener.Prefixes.Add(RedirectUri);
        _listener.Start();
    }

    /// <summary>
    /// Awaits the browser's loopback redirect and returns its full URL (the code + state query).
    /// Cancelling <paramref name="cancel"/> ends the wait with an
    /// <see cref="OperationCanceledException"/> and leaves the listener up, so
    /// <see cref="Dispose"/> can still answer the browser if it comes back.
    /// </summary>
    public async Task<string> WaitForCallbackAsync(CancellationToken cancel)
    {
        var cancelled = Task.Delay(Timeout.Infinite, cancel);

        // The first request is not always the redirect: a browser can ask the port for something
        // else first, such as /favicon.ico, and taking that fails a sign-in the person completed.
        while (true)
        {
            _pending ??= _listener.GetContextAsync();
            if (await Task.WhenAny(_pending, cancelled) != _pending)
            {
                throw new OperationCanceledException(cancel);
            }

            var context = await _pending;
            _pending = null;

            // The full loopback URL: http://127.0.0.1:<port>/?code=...&state=...&scope=..., the
            // core validates state and exchanges the code. AbsoluteUri keeps the query as the
            // browser escaped it; ToString unescapes some of it, a space among them.
            var callbackUrl = context.Request.Url?.AbsoluteUri ?? string.Empty;
            if (!_isRedirect(callbackUrl))
            {
                var path = context.Request.Url?.AbsolutePath ?? string.Empty;
                Log.Info($"google sign-in: answered a request for {Truncate(path)} that was not the redirect");
                await AnswerAsync(context.Response, 400, "Bad request");
                continue;
            }

            Log.Info("google sign-in: the browser came back with the redirect");
            _afterwards = _pages.Done;
            // The redirect is in hand: a browser that stopped listening for the page does not
            // undo a sign-in the person completed.
            await AnswerAsync(context.Response, 200, _afterwards);
            return callbackUrl;
        }
    }

    // Answers every request with the closing page until the lingering time has passed, then stops.
    private async Task LingerAsync()
    {
        using var timer = new CancellationTokenSource(_lingering);
        var over = Task.Delay(Timeout.Infinite, timer.Token);
        try
        {
            while (true)
            {
                _pending ??= _listener.GetContextAsync();
                if (await Task.WhenAny(_pending, over) != _pending)
                {
                    break;
                }

                var context = await _pending;
                _pending = null;
                await AnswerAsync(context.Response, 200, _afterwards);
            }
        }
        catch (Exception e) when (e is HttpListenerException or ObjectDisposedException)
        {
            // The listener went away underneath the answer; nothing is left to answer.
        }
        finally
        {
            Stop();
        }
    }

    // Writes a self-contained page saying the catalog's message, with no external assets so it
    // renders offline. The message carries the app's name, so it is encoded as text. A browser
    // that hung up before reading it costs nothing but the page.
    private async Task AnswerAsync(HttpListenerResponse response, int status, string message)
    {
        var html =
            "<!doctype html><html><head><meta charset=\"utf-8\">"
            + $"<title>{WebUtility.HtmlEncode(_pages.Title)}</title></head>"
            + "<body style=\"font-family: system-ui, sans-serif; text-align: center; padding: 3rem;\">"
            + $"<p>{WebUtility.HtmlEncode(message)}</p>"
            + "</body></html>";
        var bytes = Encoding.UTF8.GetBytes(html);
        try
        {
            response.StatusCode = status;
            response.ContentType = "text/html; charset=utf-8";
            response.ContentEncoding = Encoding.UTF8;
            response.ContentLength64 = bytes.LongLength;
            await response.OutputStream.WriteAsync(bytes);
            response.Close();
        }
        catch (Exception e) when (e is HttpListenerException or ObjectDisposedException or InvalidOperationException)
        {
            response.Abort();
        }
    }

    private static string Truncate(string path) => path.Length <= 64 ? path : path[..64];

    // Ask the OS for a free ephemeral loopback port by binding a throwaway socket to port 0, then
    // releasing it. HttpListener cannot bind port 0 itself, so we probe first and reuse the number;
    // the tiny window between release and HttpListener.Start is the standard, accepted race for a
    // loopback OAuth listener.
    private static int PickFreePort()
    {
        var probe = new TcpListener(IPAddress.Loopback, 0);
        probe.Start();
        try
        {
            return ((IPEndPoint)probe.LocalEndpoint).Port;
        }
        finally
        {
            probe.Stop();
        }
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _ = LingerAsync();
    }

    private void Stop()
    {
        // An accept still outstanding faults once the listener stops; observe it so it is not
        // reported as unobserved.
        _pending?.ContinueWith(t => _ = t.Exception, TaskContinuationOptions.OnlyOnFaulted);
        try
        {
            if (_listener.IsListening)
            {
                _listener.Stop();
            }
        }
        catch (ObjectDisposedException)
        {
            // Already disposed, nothing to do.
        }

        _listener.Close();
    }
}
