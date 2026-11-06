// The browser half of every sign-in: begin it in the core, open the browser, wait for the redirect.
// WinUI-free, so the retirement rule is reachable from Mailcal.Tests.
//
// An attempt is retired by Cancel or by a newer request superseding it (SignInFlight), and a
// retired attempt must not open a browser (docs/onboarding.md, "Nobody may be stranded on it"). A
// begin that dials (discovery and registration) runs on regardless of the token, so the token is
// read again once it returns. The caller checks it again after its own completion, where that
// completion only hands back what the caller would then store.

namespace Allodia.Mailcal.Services;

internal static class BrowserHop
{
    /// <summary>
    /// Begins, opens and waits, and returns what began with the redirect it ended on.
    /// <paramref name="arrived"/> runs once the redirect is in: the wait on somebody in another
    /// application is over, and what follows is a bounded call.
    /// </summary>
    /// <exception cref="OperationCanceledException">The attempt was retired.</exception>
    internal static async Task<(T Start, string Callback)> RunAsync<T>(
        Func<Task<T>> begin,
        Func<T, Task> open,
        Func<CancellationToken, Task<string>> wait,
        Action arrived,
        CancellationToken cancel)
    {
        var start = await begin();
        cancel.ThrowIfCancellationRequested();
        await open(start);
        var callback = await wait(cancel);
        arrived();
        return (start, callback);
    }
}
