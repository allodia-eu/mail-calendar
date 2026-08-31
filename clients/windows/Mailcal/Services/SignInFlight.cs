// The one browser sign-in in flight, where the newest request wins, factored out of the WinUI
// model so it can be unit-tested without a renderer, like JmapSignInGate / AccountDetectForm.
//
// The problem it exists to solve is invisible from the code that had the bug. A sign-in waiting on
// the browser CANNOT TELL that the user closed the tab: neither the loopback listener (Google) nor
// the protocol-activation rendezvous (Microsoft, JMAP) receives any signal, so the wait simply runs
// to its five-minute cap. Guarding a second attempt behind "one is already running" therefore left
// the reconnect banner's button dead for minutes with no feedback, and the only escape a user found
// was restarting the app.
//
// So a fresh request SUPERSEDES the outstanding one: cancel it, wait for it to unwind, then start.
// That preserves the invariant the guard was there for, never two flows racing for one rendezvous
// That lets the second click do the obvious thing. It is the only correct reading of the
// gesture: a user who clicks "sign in again" while an invisible attempt is pending means the new
// one.
//
// Threading: UI thread only. Every entry point runs on the WinUI dispatcher and each continuation
// resumes there, so the cancel → await → start sequence cannot interleave with another caller's.
using System;
using System.Threading;
using System.Threading.Tasks;

namespace Allodia.Mailcal.Services;

/// <summary>
/// Serializes browser sign-ins so only one is ever outstanding, superseding rather than refusing a
/// second request. See the file header for why refusing is wrong.
/// </summary>
internal sealed class SignInFlight
{
    // The outstanding flow's cancellation source, so Cancel (and a superseding request) can break
    // the otherwise unbounded wait on the browser redirect. Null when nothing is running.
    private CancellationTokenSource? _cancel;

    // The outstanding flow itself, so a superseding request can await its unwind before starting.
    // Completed, never null, so the first call needs no special case.
    private Task _running = Task.CompletedTask;

    /// <summary>
    /// Cancels the outstanding sign-in, if any (the setup form's Cancel button). Safe to call when
    /// none is running. The awaiting flow unwinds cleanly and re-enables the form.
    /// </summary>
    public void Cancel() => _cancel?.Cancel();

    /// <summary>
    /// Runs <paramref name="flow"/> as the only sign-in in flight: any outstanding one is cancelled
    /// and awaited first, so two flows never race for the same redirect rendezvous.
    /// </summary>
    /// <remarks>
    /// The superseded flow's failure is swallowed deliberately, it owns its own error reporting
    /// (each sign-in method catches and surfaces its outcome), and being cancelled is expected here
    /// rather than exceptional. Letting it propagate would report the *old* attempt's cancellation
    /// as the *new* attempt's error.
    /// </remarks>
    public Task RunAsync(Func<CancellationToken, Task> flow) =>
        RunAsync<object?>(async cancel =>
        {
            await flow(cancel);
            return null;
        });

    /// <summary>
    /// The result-returning form of <see cref="RunAsync(Func{CancellationToken, Task})"/>, for the
    /// JMAP sign-in, which reports an outcome its caller renders as an inline note.
    /// </summary>
    public async Task<T> RunAsync<T>(Func<CancellationToken, Task<T>> flow)
    {
        _cancel?.Cancel();
        try
        {
            await _running;
        }
        catch
        {
            // See the remarks: the superseded flow reports itself; its cancellation is expected.
        }

        using var cancel = new CancellationTokenSource();
        // Install BOTH signals before invoking the flow. Assigning _running from flow()'s return
        // value would be too late: invoking an async lambda runs its prologue synchronously, and a
        // continuation resuming inline in there can re-enter this method, which would then see the
        // PREVIOUS run's completed task, skip the wait, and let two flows run at once. A completion
        // proxy closes that window, because it exists before the flow can do anything at all.
        var finished = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        _cancel = cancel;
        _running = finished.Task;
        try
        {
            return await flow(cancel.Token);
        }
        finally
        {
            // Only clear the shared slot if it is still ours: a request that superseded this one
            // has already installed its own, and nulling it would leave that flow uncancellable.
            if (ReferenceEquals(_cancel, cancel))
            {
                _cancel = null;
            }
            finished.SetResult();
        }
    }
}
