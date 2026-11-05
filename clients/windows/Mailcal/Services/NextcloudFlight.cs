// The browser half of a Nextcloud sign-in, which every surface offering it shares
// (docs/nextcloud-sign-in.md rules 5 and 6). The core starts the sign-in, the browser opens on the
// server's own login page, and the flight asks the core whether the person has finished: every poll
// interval, and at once when the window is active again. There is no redirect back, so the poll is
// all the waiting there is.
//
// What a surface does with the grant is its own: each completion is a different core call. What
// they share is that the server hands a grant out once, so one that arrives for an attempt nobody is
// waiting on any more is withdrawn rather than dropped.
//
// WinUI-free, with every core call and the browser passed in, so Mailcal.Tests drives the loop.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>How a flight ended.</summary>
internal abstract record NextcloudFlightEnd
{
    /// <summary>The person granted access. Never printed: the grant carries the app password.</summary>
    internal sealed record Granted(string Grant) : NextcloudFlightEnd
    {
        public override string ToString() => "Granted(..)";
    }

    /// <summary>The server let the sign-in go before the person finished.</summary>
    internal sealed record Expired : NextcloudFlightEnd;

    /// <summary>The browser did not open on the login page.</summary>
    internal sealed record BrowserFailed : NextcloudFlightEnd;

    /// <summary>The core refused to start, or to poll.</summary>
    internal sealed record Failed(Exception Error) : NextcloudFlightEnd;

    /// <summary>The attempt was retired. A grant that arrived after that has been withdrawn.</summary>
    internal sealed record Cancelled : NextcloudFlightEnd;
}

/// <summary>
/// Wakes every flight waiting on the person in the browser, when the app's window is active again:
/// somebody who just granted access there is back, so a flight asks now rather than at its next
/// interval. Thread-safe; a wake with no flight waiting is lost, which is right.
/// </summary>
internal sealed class NextcloudWake
{
    private TaskCompletionSource _next = new(TaskCreationOptions.RunContinuationsAsynchronously);

    /// <summary>Completes at the next <see cref="Returned"/>.</summary>
    internal Task Next => Volatile.Read(ref _next).Task;

    /// <summary>The window is active again.</summary>
    internal void Returned() =>
        Interlocked.Exchange(ref _next, new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously))
            .TrySetResult();
}

internal static class NextcloudFlight
{
    /// <summary>
    /// Flies one sign-in: <paramref name="begin"/> starts it (the surface's own start, a blocking
    /// core call the caller runs off the UI thread), <paramref name="openBrowser"/> opens the login
    /// page and says whether it did, then <paramref name="poll"/> is asked every
    /// <paramref name="interval"/>, or sooner when <paramref name="wake"/> fires, until the sign-in
    /// ends or <paramref name="cancel"/> is raised. <paramref name="opened"/> hears that the browser
    /// holds the sign-in now.
    /// </summary>
    /// <remarks>A grant answered after <paramref name="cancel"/> was raised is handed to
    /// <paramref name="abandon"/> and the flight ends <see cref="NextcloudFlightEnd.Cancelled"/>.</remarks>
    internal static async Task<NextcloudFlightEnd> FlyAsync(
        Func<Task<NextcloudSignInStart>> begin,
        Func<string, Task<bool>> openBrowser,
        Func<string, Task<NextcloudSignInPoll>> poll,
        Action<string> abandon,
        TimeSpan interval,
        NextcloudWake wake,
        CancellationToken cancel,
        Action? opened = null)
    {
        NextcloudSignInStart start;
        try
        {
            start = await begin();
        }
        catch (Exception ex)
        {
            return cancel.IsCancellationRequested ? new NextcloudFlightEnd.Cancelled() : new NextcloudFlightEnd.Failed(ex);
        }
        // A begin that answers after the way back opens no browser.
        if (cancel.IsCancellationRequested)
        {
            return new NextcloudFlightEnd.Cancelled();
        }
        if (!await openBrowser(start.LoginUrl))
        {
            return new NextcloudFlightEnd.BrowserFailed();
        }
        opened?.Invoke();
        while (true)
        {
            if (cancel.IsCancellationRequested)
            {
                return new NextcloudFlightEnd.Cancelled();
            }
            // Taken before the ask, so a window activated while it is out still wakes the wait.
            var woken = wake.Next;
            NextcloudSignInPoll answer;
            try
            {
                answer = await poll(start.Pending);
            }
            catch (Exception ex)
            {
                return cancel.IsCancellationRequested ? new NextcloudFlightEnd.Cancelled() : new NextcloudFlightEnd.Failed(ex);
            }
            switch (answer)
            {
                case NextcloudSignInPoll.Granted granted when cancel.IsCancellationRequested:
                    abandon(granted.Grant);
                    return new NextcloudFlightEnd.Cancelled();
                case NextcloudSignInPoll.Granted granted:
                    return new NextcloudFlightEnd.Granted(granted.Grant);
                case NextcloudSignInPoll.Expired:
                    return new NextcloudFlightEnd.Expired();
            }
            // An interval, a wake or the way back: each means "look again", and the loop's head
            // tells the last apart.
            await Task.WhenAny(Task.Delay(interval, cancel), woken);
        }
    }
}

/// <summary>
/// A grant a surface could not make an account of yet, held for the next press because the server
/// hands it out once (docs/nextcloud-sign-in.md rule 6), and what it is held for: a server, or an
/// account. One per surface. Never printed.
/// </summary>
internal sealed class NextcloudGrantHold
{
    private (string For, string Grant)? _held;

    /// <summary>Whether the core is completing the held grant now, which nothing may withdraw.</summary>
    internal bool Completing { get; private set; }

    /// <summary>The grant held for <paramref name="key"/>, when there is one.</summary>
    internal string? HeldFor(string key) => _held is { } held && held.For == key ? held.Grant : null;

    /// <summary>The core is about to complete <paramref name="grant"/> for <paramref name="key"/>:
    /// held, and out of reach of <see cref="Release"/> until <see cref="Completed"/>.</summary>
    internal void Complete(string key, string grant)
    {
        _held = (key, grant);
        Completing = true;
    }

    /// <summary>The completion answered. A grant that became an account, or that the core
    /// withdrew, is <paramref name="spent"/> and forgotten; any other stays for the next press.</summary>
    internal void Completed(bool spent)
    {
        Completing = false;
        if (spent)
        {
            _held = null;
        }
    }

    /// <summary>Takes the held grant to withdraw it, because its surface was left or a sign-in for
    /// something else starts; null when none is held, or the core is completing it.</summary>
    internal string? Release()
    {
        if (Completing || _held is not { } held)
        {
            return null;
        }
        _held = null;
        return held.Grant;
    }
}
