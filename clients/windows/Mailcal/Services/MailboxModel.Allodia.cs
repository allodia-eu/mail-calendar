// The Allodia-account half of MailboxModel (split out to keep each file under the 500-line limit):
// the browser sign-in, who is signed in, and signing out. The twin of the Microsoft sign-in in
// MailboxModel.Accounts.cs, same rendezvous, same off-the-UI-thread rule.
//
// The core owns discovery, PKCE, the exchange, the identity lookup and the Credential Manager
// write, so nothing here ever holds a token.

using System;
using System.Threading;
using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // Cancels a sign-in that is waiting on the browser redirect. A custom-scheme flow gets no
    // signal when the user simply closes the browser, unlike a loopback listener, whose socket
    // surfaces one, so without this the card would sit on "Signing in…" until the five-minute cap.
    private CancellationTokenSource? _allodiaSignIn;

    /// <summary>
    /// Who is signed in to an Allodia account, or <c>null</c>. Cheap and local, it reads what the
    /// last launch restored or the last sign-in wrote, and never asks the service.
    /// </summary>
    // `internal`, like every signature naming a generated UniFFI type: they are emitted internal,
    // so a public one is a CS0051 accessibility error.
    internal AllodiaAccount? SignedInAllodiaAccount() => _app?.AllodiaAccount();

    /// <summary>
    /// Runs a sign-in and returns <c>null</c> on success or when the user gave up, and the failure
    /// text otherwise. The core reads the service's own OAuth metadata, mints the authorization
    /// URL, exchanges the redirect, asks whose account it is and stores the grant.
    /// </summary>
    internal async Task<string?> SignInToAllodiaAsync(bool create = false)
    {
        if (_app is null)
        {
            return "Could not open the app. Please relaunch.";
        }
        using var cancel = new CancellationTokenSource();
        _allodiaSignIn = cancel;
        try
        {
            // Arm the redirect rendezvous BEFORE opening the browser: the browser returns to our
            // custom scheme, delivered here as a protocol activation routed by Program.
            using var callback = ProtocolAuthCallback.Expect(AllodiaOAuthConfig.CallbackHost);
            // Discovery is two network round trips and the core call blocks on them. `_app!`, like
            // the Microsoft flow's: the null check above does not survive into a lambda over a
            // field, so the compiler needs telling.
            var start = await Task.Run(
                () => create
                    ? _app!.BeginAllodiaRegistration(AllodiaOAuthConfig.RedirectUri)
                    : _app!.BeginAllodiaSignIn(AllodiaOAuthConfig.RedirectUri));
            await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl));
            var callbackUrl = await callback.WaitAsync(cancel.Token);
            // The exchange and the identity lookup block too.
            await Task.Run(() => _app!.CompleteAllodiaSignIn(start.Pending, callbackUrl));
            // Never who: a log line describes the user's mail and never names an address
            // (docs/logging.md), and this file is what a support request arrives with.
            Log.Info("allodia: signed in; the grant is stored");
            return null;
        }
        catch (OperationCanceledException)
        {
            // The user pressed Cancel while the browser step was outstanding, return to the card
            // quietly; the disposed callback slot ignores any late redirect.
            Log.Info("allodia: sign-in cancelled by user");
            return null;
        }
        catch (Exception ex)
        {
            Log.Error($"allodia: sign-in failed: {CoreError.Describe(ex)}");
            return CoreError.Describe(ex);
        }
        finally
        {
            _allodiaSignIn = null;
        }
    }

    /// <summary>Aborts a sign-in waiting on the browser redirect. Safe when none is in flight.</summary>
    internal void CancelAllodiaSignIn() => _allodiaSignIn?.Cancel();

    /// <summary>
    /// Opens the service's own account page, where someone changes their details or deletes the
    /// account. A page, not a flow: nothing is pending and nothing comes back.
    /// </summary>
    internal void OpenAllodiaAccountPage()
    {
        if (_app?.AllodiaAccountUrl() is { } url)
        {
            _ = Windows.System.Launcher.LaunchUriAsync(new Uri(url));
        }
    }

    /// <summary>
    /// Signs out: the core forgets the account and erases its stored grant. Returns the failure
    /// text when the Credential Manager refused the delete, and <c>null</c> on success.
    /// </summary>
    /// <remarks>
    /// The erase is local, which is what removing a mail account is too: the grant stays alive at
    /// the service until it expires or the person revokes it there, and the end-session hop below
    /// closes the browser's session rather than the grant.
    /// </remarks>
    internal string? SignOutOfAllodia()
    {
        try
        {
            // Best-effort and deliberately unreported: this device is signed out whatever happens
            // to the browser. What opening it buys is the next sign-in asking who you are rather
            // than completing silently against a session someone thought they had left. It does
            // not end the grant at the service, a refresh token carrying offline_access outlives
            // it by design.
            var endSession = _app?.SignOutOfAllodia();
            // Nothing left to say about other devices once this one leaves the account that linked
            // them. Cleared whatever the browser hop below does.
            ForgetAllodiaSync();
            if (endSession is { } url)
            {
                _ = Windows.System.Launcher.LaunchUriAsync(new Uri(url));
            }
            return null;
        }
        catch (Exception ex)
        {
            Log.Error($"allodia: sign-out failed: {CoreError.Describe(ex)}");
            return CoreError.Describe(ex);
        }
    }
}
