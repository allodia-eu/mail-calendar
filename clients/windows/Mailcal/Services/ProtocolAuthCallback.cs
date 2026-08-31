// The custom-scheme redirect rendezvous shared by every browser sign-in that comes back through a
// protocol activation: Microsoft 365 (eu.allodia.mailcal://auth) and JMAP (…://jmap-oauth). The
// Google flow does not use it, Google's Desktop client redirects to an http://127.0.0.1 loopback
// listener instead (GoogleOAuth.cs).
//
// The OS delivers the redirect by ACTIVATING this app, which `Program` routes into Deliver on a
// background thread; the in-flight sign-in awaits the armed slot. One slot is enough because the
// model only ever lets one browser sign-in run at a time, but the slot is armed for a specific
// redirect host, so a stray or late activation from the other flow can never complete this one.

using System;
using System.Threading;
using System.Threading.Tasks;

namespace Allodia.Mailcal.Services;

/// <summary>
/// A one-shot rendezvous for an OAuth redirect delivered as a protocol activation. The sign-in
/// flow <see cref="Expect"/>s a callback before opening the browser, then awaits
/// <see cref="Registration.WaitAsync"/>; the protocol-activation handler in <c>Program</c> calls
/// <see cref="Deliver"/> with the redirect URI to complete it. An activation with nothing pending,
/// or for a different redirect host, is ignored.
/// </summary>
internal static class ProtocolAuthCallback
{
    // How long a browser sign-in may stay outstanding before the wait gives up. Generous: the user
    // may have to find a password manager, or complete an MFA prompt on another device.
    private static readonly TimeSpan Deadline = TimeSpan.FromMinutes(5);

    private static readonly object Gate = new();
    private static TaskCompletionSource<string>? _pending;
    private static string? _host;

    /// <summary>
    /// Arms the wait for the next redirect to <paramref name="host"/> (the redirect URI's host,
    /// <c>auth</c> for Microsoft, <c>jmap-oauth</c> for JMAP). Dispose the result to disarm, so an
    /// abandoned sign-in leaves no stale slot to swallow the next flow's redirect.
    /// </summary>
    public static Registration Expect(string host)
    {
        lock (Gate)
        {
            // RunContinuationsAsynchronously: Deliver runs on the activation background thread, so
            // don't resume the awaiting sign-in inline on it.
            _pending = new TaskCompletionSource<string>(TaskCreationOptions.RunContinuationsAsynchronously);
            _host = host;
            return new Registration(_pending);
        }
    }

    /// <summary>
    /// Completes a waiting sign-in with the redirect URI. No-op if nothing is pending or the URI's
    /// host isn't the one that was armed.
    /// </summary>
    public static void Deliver(Uri uri)
    {
        lock (Gate)
        {
            if (_pending is null)
            {
                return;
            }
            if (!string.Equals(_host, uri.Host, StringComparison.OrdinalIgnoreCase))
            {
                // Never the query (it carries the one-time auth code), only which host arrived,
                // so a redirect that silently goes nowhere is diagnosable rather than invisible.
                Log.Warn($"protocol activation for '{uri.Host}' ignored; awaiting '{_host}'");
                return;
            }
            _pending.TrySetResult(uri.ToString());
        }
    }

    /// <summary>The armed slot; hold it for the flow's duration and dispose when done.</summary>
    internal sealed class Registration : IDisposable
    {
        private readonly TaskCompletionSource<string> _tcs;

        internal Registration(TaskCompletionSource<string> tcs) => _tcs = tcs;

        /// <summary>
        /// Awaits the browser's redirect and returns its full callback URL. Unlike a loopback
        /// listener, whose socket surfaces an error when the browser is dismissed, a
        /// custom-scheme flow gets no signal if the user abandons sign-in, so this waits on three
        /// outcomes: the redirect arrives, the user cancels (<paramref name="cancel"/>), or a
        /// generous cap elapses. Without the cancel path the form would stay pinned on "Signing
        /// in…" with no way out until the cap; without the cap a silently-dropped redirect would
        /// hang it for good.
        /// </summary>
        /// <exception cref="OperationCanceledException">The user cancelled.</exception>
        /// <exception cref="TimeoutException">The redirect never arrived.</exception>
        public async Task<string> WaitAsync(CancellationToken cancel)
        {
            using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancel);
            deadline.CancelAfter(Deadline);
            // A task that completes only when the user cancels or the cap elapses; race it against
            // the redirect. Swallow its own cancellation, it's the loser of the race, not an error.
            var abort = Task.Delay(Timeout.Infinite, deadline.Token).ContinueWith(
                _ => { }, TaskScheduler.Default);
            if (await Task.WhenAny(_tcs.Task, abort) != _tcs.Task)
            {
                cancel.ThrowIfCancellationRequested(); // user pressed Cancel -> OperationCanceledException
                throw new TimeoutException("Sign-in timed out. Please try again.");
            }
            return await _tcs.Task;
        }

        public void Dispose()
        {
            lock (Gate)
            {
                if (ReferenceEquals(_pending, _tcs))
                {
                    _pending = null;
                    _host = null;
                }
            }
        }
    }
}
