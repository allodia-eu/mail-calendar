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
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>
/// A one-shot loopback redirect listener for the Google Desktop OAuth flow. Construction picks a
/// free ephemeral port on 127.0.0.1 and starts an <see cref="HttpListener"/> on it;
/// <see cref="RedirectUri"/> is the exact address to hand <c>begin_google_login</c> (it MUST match
/// what the browser returns to). <see cref="WaitForCallbackAsync"/> awaits the single inbound
/// redirect GET, writes back a tiny "you can close this tab" page, and returns the full callback
/// URL (with the code + state query); any other request is answered with a 400. Dispose keeps the
/// port answering for <see cref="Lingering"/> with a page saying the sign-in finished or is no
/// longer waiting, so a browser that comes back late reads that rather than a refused connection,
/// and then stops the listener.
/// </summary>
internal sealed class GoogleLoopback : IDisposable
{
    private static readonly TimeSpan Lingering = TimeSpan.FromMinutes(3);

    private readonly HttpListener _listener = new();

    // The accept still outstanding when a wait ends, so the lingering answer takes it over.
    private Task<HttpListenerContext>? _pending;

    // What the port says once the wait is over: finished after a redirect, else no longer waiting.
    private string _afterwards = L10n.SignInPageEnded();

    private bool _disposed;

    /// <summary>
    /// The loopback redirect URI the browser returns to, pass this verbatim to
    /// <c>begin_google_login</c>. A trailing slash is required by HttpListener's prefix grammar,
    /// which Google accepts for a Desktop client's loopback redirect.
    /// </summary>
    public string RedirectUri { get; }

    public GoogleLoopback()
    {
        var port = PickFreePort();
        RedirectUri = $"http://127.0.0.1:{port}/";
        _listener.Prefixes.Add(RedirectUri);
        _listener.Start();
    }

    /// <summary>
    /// Awaits the browser's loopback redirect and returns its full URL (the code + state query).
    /// Honours <paramref name="cancel"/>: cancelling stops the listener, which unblocks the pending
    /// accept and surfaces here as an <see cref="OperationCanceledException"/> rather than a hang.
    /// </summary>
    public async Task<string> WaitForCallbackAsync(CancellationToken cancel)
    {
        // Cancelling ends the wait but leaves the listener up, so Dispose can still answer the
        // browser if it comes back.
        var cancelled = Task.Delay(Timeout.Infinite, cancel);

        // The first request is not always the redirect: a browser can ask the port for something
        // else first, such as /favicon.ico, and taking that fails a sign-in the person completed.
        // Anything else is answered with a 400 while the wait goes on.
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
            // core validates state and exchanges the code.
            var callbackUrl = context.Request.Url?.ToString() ?? string.Empty;
            if (!MailcalBindingsMethods.IsOauthCallback(callbackUrl))
            {
                context.Response.StatusCode = 400;
                context.Response.Close();
                continue;
            }

            _afterwards = L10n.SignInPageDone();
            await WritePageAsync(context.Response, _afterwards);
            return callbackUrl;
        }
    }

    // Answers every request with the closing page until Lingering has passed, then stops.
    private async Task LingerAsync()
    {
        using var timer = new CancellationTokenSource(Lingering);
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
                await WritePageAsync(context.Response, _afterwards);
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

    // A self-contained page saying the catalog's message, with no external assets so it renders
    // offline. The message carries the app's name, so it is encoded as text.
    private static async Task WritePageAsync(HttpListenerResponse response, string message)
    {
        var html =
            "<!doctype html><html><head><meta charset=\"utf-8\">"
            + $"<title>{WebUtility.HtmlEncode(L10n.AppTitle())}</title></head>"
            + "<body style=\"font-family: system-ui, sans-serif; text-align: center; padding: 3rem;\">"
            + $"<p>{WebUtility.HtmlEncode(message)}</p>"
            + "</body></html>";
        var bytes = Encoding.UTF8.GetBytes(html);
        response.ContentType = "text/html; charset=utf-8";
        response.ContentEncoding = Encoding.UTF8;
        response.ContentLength64 = bytes.LongLength;
        try
        {
            await response.OutputStream.WriteAsync(bytes);
        }
        finally
        {
            response.Close();
        }
    }

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
