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
/// A one-shot loopback redirect listener for the Google Desktop OAuth flow. Construction picks a
/// free ephemeral port on 127.0.0.1 and starts an <see cref="HttpListener"/> on it;
/// <see cref="RedirectUri"/> is the exact address to hand <c>begin_google_login</c> (it MUST match
/// what the browser returns to). <see cref="WaitForCallbackAsync"/> awaits the single inbound
/// redirect GET, writes back a tiny "you can close this tab" page, and returns the full callback
/// URL (with the code + state query). Dispose stops the listener, so an abandoned flow leaves no
/// socket open.
/// </summary>
internal sealed class GoogleLoopback : IDisposable
{
    private readonly HttpListener _listener = new();

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
        // Stopping the listener is the only way to break a pending GetContextAsync; on cancellation
        // the resulting listener/dispose fault is translated into a clean cancellation below.
        using var registration = cancel.Register(() =>
        {
            try
            {
                _listener.Stop();
            }
            catch (ObjectDisposedException)
            {
                // Already torn down (the flow finished as we cancelled), nothing to stop.
            }
        });

        HttpListenerContext context;
        try
        {
            context = await _listener.GetContextAsync();
        }
        catch (Exception) when (cancel.IsCancellationRequested)
        {
            // Stop()/dispose broke the wait because we were cancelled, report it as such.
            throw new OperationCanceledException(cancel);
        }

        // The full loopback URL: http://127.0.0.1:<port>/?code=...&state=...&scope=..., the core
        // validates state and exchanges the code.
        var callbackUrl = context.Request.Url?.ToString() ?? string.Empty;
        await WriteClosePageAsync(context.Response);
        return callbackUrl;
    }

    // The throwaway page the browser tab lands on after the redirect. Neutral, on-brand English,
    // there is no l10n key for this transient page (a known gap, noted in the PR). Self-contained
    // (no external assets), so it renders offline.
    private static async Task WriteClosePageAsync(HttpListenerResponse response)
    {
        const string html =
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">"
            + "<title>Allodia Mail &amp; Calendar</title></head>"
            + "<body style=\"font-family: system-ui, sans-serif; text-align: center; padding: 3rem;\">"
            + "<p>You can close this tab and return to Allodia Mail &amp; Calendar.</p>"
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
