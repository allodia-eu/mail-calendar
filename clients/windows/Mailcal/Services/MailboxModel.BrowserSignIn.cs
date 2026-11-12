// An existing account's browser sign-in, which the core holds from the press to the account it
// changes (docs/waiting.md): signing in again, a switch from a password to the provider or to
// Nextcloud, a use the provider is asked for, the banner's Sign in again, and the sign-in a
// calendar and contacts on a server of their own are given (docs/accounts.md rule 18), and the
// Allodia account's sign-in (MailboxModel.Allodia.cs). This host starts it, opens the browser once
// for each attempt, listens where the browser returns and hands that back; which step it is at,
// what it asks, how long it waits and what it came to are the core's.
//
// A custom-scheme redirect arrives as a protocol activation, which Program hands to
// TakeSignInRedirect before any setup flow sees it. A Google redirect arrives on a loopback port
// bound before the sign-in starts, since the core needs its address to start it.

using System.Net;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // The attempt the browser was opened for, so a redraw never opens it twice.
    private ulong? _browserOpenedFor;

    // The loopback a Google sign-in returns to, the attempt it is listened on for, and that wait.
    private GoogleLoopback? _signInLoopback;
    private ulong? _signInListenedFor;
    private CancellationTokenSource? _signInListening;

    /// <summary>Signs <paramref name="accountId"/> in through its browser sign-in, whichever its
    /// page offers, asking its provider for <paramref name="adding"/> as well. Supersedes a
    /// sign-in still out.</summary>
    internal void SignIn(string accountId, AccountCapability[] adding)
    {
        if (_app is null)
        {
            return;
        }
        if (AccountsSnapshot().Accounts.FirstOrDefault(entry => entry.Id == accountId) is not { } entry)
        {
            Log.Warn("sign-in: asked for an account that is gone");
            return;
        }
        // A loopback belongs to the sign-in it was bound for, which this one supersedes.
        StopListening();
        string redirect;
        try
        {
            redirect = BrowserSignInRules.RedirectOf(entry.Kind) switch
            {
                SignInRedirect.Microsoft => MicrosoftOAuthConfig.RedirectUri,
                SignInRedirect.Google => (_signInLoopback = NewGoogleLoopback()).RedirectUri,
                SignInRedirect.Imap => ImapOAuthConfig.RedirectUri,
                SignInRedirect.Jmap => JmapOAuthConfig.RedirectUri,
                _ => string.Empty,
            };
        }
        catch (HttpListenerException ex)
        {
            // Sent with nowhere to return to, which the core refuses and the page says.
            Log.Error($"sign-in: no loopback port could be opened for the browser to return to ({ex.ErrorCode})");
            redirect = string.Empty;
        }
        Log.Info($"sign-in: started for {LogHandle(accountId)}");
        ChangeAccount(new AccountPageIntent.SignIn(accountId, adding, redirect));
    }

    /// <summary>Signs what <paramref name="accountId"/> needs a sign-in of its own for in through
    /// the browser, the way its server offers: at the server's own provider, which returns under
    /// the app's scheme as a standards account's does, or with Nextcloud, which returns nothing and
    /// for which the core ignores the redirect. Supersedes a sign-in still out.</summary>
    internal void SignInSeparately(string accountId)
    {
        if (_app is null)
        {
            return;
        }
        StopListening();
        Log.Info($"sign-in: started for {LogHandle(accountId)}'s separate server");
        ChangeAccount(new AccountPageIntent.SignInSeparateServer(accountId, ImapOAuthConfig.RedirectUri));
    }

    /// <summary>Cancels <paramref name="browser"/>, from its section's Cancel.</summary>
    internal void CancelSignIn(BrowserSignIn browser) =>
        ChangeAccount(new AccountPageIntent.CancelSignIn(browser.Account, browser.Attempt));

    /// <summary>Answers the question <paramref name="browser"/> stands at; null cancels it.</summary>
    internal void AnswerUnreached(BrowserSignIn browser, UnreachedUses? answer) =>
        ChangeAccount(new AccountPageIntent.AnswerUnreached(browser.Account, browser.Attempt, answer));

    /// <summary>The browser sign-in out, as the core holds it; null when none is.</summary>
    internal BrowserSignIn? BrowserSignInNow() => _app?.BrowserSignIn();

    /// <summary>
    /// Hands <paramref name="uri"/>, a redirect the OS activated the app with, to the browser
    /// sign-in when it is that sign-in's. Run on the UI thread.
    /// </summary>
    /// <returns>Whether it was; one that was not is a setup flow's.</returns>
    internal bool TakeSignInRedirect(Uri uri)
    {
        if (BrowserSignInNow() is not { } record || !BrowserSignInRules.Returns(record, uri))
        {
            return false;
        }
        Log.Info("sign-in: the browser came back");
        ChangeAccount(new AccountPageIntent.SignInBrowserEnded(
            record.Account, record.Attempt, new SignInBrowserEnd.Redirected(uri.ToString())));
        return true;
    }

    /// <summary>Follows the core's browser sign-in on a Settings or notices signal: opens the
    /// browser for an attempt that waits on it, and lets go of a loopback the sign-in no longer
    /// returns to.</summary>
    private void DriveBrowserSignIn()
    {
        if (_app is null)
        {
            return;
        }
        var record = BrowserSignInNow();
        if (_signInLoopback is { } loopback
            && !BrowserSignInRules.KeepsLoopback(record, loopback.RedirectUri, _signInListenedFor))
        {
            StopListening();
        }
        if (BrowserSignInRules.ToOpen(record, _browserOpenedFor) is not { } browser)
        {
            return;
        }
        var (account, attempt) = (record!.Account, record.Attempt);
        _browserOpenedFor = attempt;
        if (_signInLoopback is { } listener && listener.RedirectUri == browser.RedirectUri)
        {
            _signInListenedFor = attempt;
            _signInListening = new CancellationTokenSource();
            _ = ListenForSignInAsync(listener, account, attempt, _signInListening.Token);
        }
        _ = OpenSignInBrowserAsync(account, attempt, browser.Url);
    }

    private async Task OpenSignInBrowserAsync(string account, ulong attempt, string url)
    {
        bool opened;
        try
        {
            opened = await ExternalLauncher.Open(new Uri(url));
        }
        catch (Exception ex)
        {
            Log.Warn($"sign-in: the browser could not be asked to open ({ex.GetType().Name})");
            opened = false;
        }
        if (!opened)
        {
            Log.Warn("sign-in: the browser did not open");
            ChangeAccount(new AccountPageIntent.SignInBrowserEnded(account, attempt, new SignInBrowserEnd.NotOpened()));
        }
    }

    private async Task ListenForSignInAsync(GoogleLoopback loopback, string account, ulong attempt, CancellationToken cancel)
    {
        string callbackUrl;
        try
        {
            callbackUrl = await loopback.WaitForCallbackAsync(cancel);
        }
        catch (OperationCanceledException)
        {
            return;
        }
        catch (Exception ex) when (ex is HttpListenerException or ObjectDisposedException)
        {
            Log.Warn($"sign-in: the loopback port stopped listening ({ex.GetType().Name})");
            return;
        }
        ChangeAccount(new AccountPageIntent.SignInBrowserEnded(
            account, attempt, new SignInBrowserEnd.Redirected(callbackUrl)));
    }

    // Lets go of the loopback: a browser that comes back to it later reads that the sign-in is
    // over rather than a refused connection.
    private void StopListening()
    {
        _signInListening?.Cancel();
        _signInListening?.Dispose();
        _signInListening = null;
        _signInListenedFor = null;
        _signInLoopback?.Dispose();
        _signInLoopback = null;
    }
}
