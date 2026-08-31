// The JMAP "sign in with your provider" half of MailboxModel, split out to keep each file under
// the 500-line limit and to sit beside the Microsoft/Google flows (MailboxModel.Accounts.cs /
// MailboxModel.Google.cs). It mirrors SignInWithMicrosoft, same custom-scheme protocol
// activation, same bounded wait, with two differences that come from JMAP having no integration:
//
//   - there is a PRE-FLIGHT. Sign-in is discovered, not guaranteed, so the form asks the core
//     whether this server offers it at all before showing a button (JmapOAuthAvailableAsync);
//   - a failure is NOT an error banner. "This server doesn't do this" is an expected outcome, and
//     the password/API-token field still works, so the outcome comes back to the view, which says
//     so inline and leaves the manual path alone.
//
// What completion returns is the same `[jmap]` config TOML the manual form produces, so it goes
// through the very same add + Credential Manager write as SubmitJmapSetup, no second storage path.

using System.Threading.Tasks;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Whether this JMAP server offers discoverable OAuth sign-in, the pre-flight that decides
    /// whether the setup form shows a sign-in button at all. Blocking in the core (it makes
    /// network round trips), so it runs off the UI thread; it never throws, and anything that goes
    /// wrong reads as "no sign-in here", which is exactly the right answer for a button.
    /// </summary>
    internal async Task<bool> JmapOAuthAvailableAsync(string email, string serverUrl)
    {
        var app = _app;
        if (app is null)
        {
            return false;
        }
        // A blank server means "derive it from the email domain", never an empty URL, the same
        // rule the manual form follows (JmapSetupForm.Build).
        var server = string.IsNullOrWhiteSpace(serverUrl) ? null : serverUrl;
        try
        {
            return await Task.Run(() => app.JmapOauthAvailable(email, server)).ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            // The core answers false for every ordinary failure, so nothing should land here, but
            // this runs fire-and-forget off a timer, where an exception would otherwise be
            // swallowed unobserved. Log it and show no button; the secret field still works.
            Log.Warn($"jmap oauth pre-flight failed: {CoreError.Describe(ex)}");
            return false;
        }
    }

    /// <summary>
    /// Runs the JMAP sign-in: discovers + registers with the provider, opens the authorization URL
    /// in the user's browser, captures the redirect, exchanges the code, then adds and stores the
    /// resulting account through the same path as a manual JMAP connect. Returns what happened, so
    /// the form can show its inline "signing in didn't work" note without a raw protocol error.
    /// </summary>
    /// <remarks>
    /// Runs through the shared <see cref="SignInFlight"/> like the Microsoft and Google flows. That
    /// is not tidiness: <c>ProtocolAuthCallback</c> holds a SINGLE static pending slot, so a JMAP
    /// and a Microsoft sign-in genuinely compete for one redirect rendezvous, the second
    /// <c>Expect</c> orphans the first, which then waits out its cap. Serializing all three keeps
    /// that impossible now that the flows no longer refuse each other via <see cref="IsSubmitting"/>.
    /// </remarks>
    internal Task<JmapSignInOutcome> SignInWithJmapAsync(string email, string serverUrl)
    {
        // A connect already in flight, or an engine that never opened: nothing to report to the
        // user (the failed launch already surfaced its own error), so don't raise the note.
        if (_connecting || _app is null)
        {
            return Task.FromResult(JmapSignInOutcome.Cancelled);
        }
        return _signIn.RunAsync(cancel => SignInWithJmapFlowAsync(email, serverUrl, cancel));
    }

    private async Task<JmapSignInOutcome> SignInWithJmapFlowAsync(
        string email, string serverUrl, CancellationToken cancelToken)
    {
        var app = _app!;
        var server = string.IsNullOrWhiteSpace(serverUrl) ? null : serverUrl;
        // IsSigningIn drives the (enabled) Cancel; IsSubmitting keeps the setup form's own buttons
        // from firing twice (they bind to it) while the browser step is outstanding.
        IsSigningIn = true;
        IsSubmitting = true;
        SetupError = null;
        string configToml;
        try
        {
            // Arm the redirect rendezvous before opening the browser: the browser returns to our
            // custom scheme (eu.allodia.mailcal://jmap-oauth), delivered here as a protocol
            // activation routed by Program into ProtocolAuthCallback.
            using var callback = ProtocolAuthCallback.Expect(JmapOAuthConfig.CallbackHost);
            // Discovery + dynamic client registration are several network round trips, so they
            // block, off the UI thread with them.
            var start = await Task.Run(() => app.BeginJmapLogin(email, server, JmapOAuthConfig.RedirectUri));
            // Open the default browser (where the user is usually already signed in).
            await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl));
            var callbackUrl = await callback.WaitAsync(cancelToken);
            // The token exchange blocks too.
            configToml = await Task.Run(() => app.CompleteJmapLogin(start.Pending, callbackUrl));
        }
        catch (OperationCanceledException)
        {
            // The user pressed Cancel while the browser step was outstanding, return to the form
            // quietly; the disposed callback slot ignores any late redirect.
            Log.Info("jmap sign-in cancelled by user");
            return JmapSignInOutcome.Cancelled;
        }
        catch (Exception ex)
        {
            // Discovery is allowed to fail, a JMAP server need not offer any of this, so the
            // form says so in plain words and keeps the password/API-token field; the specific
            // cause stays in the diagnostic log rather than in front of the user.
            Log.Error($"jmap sign-in failed: {CoreError.Describe(ex)}");
            return JmapSignInOutcome.Failed;
        }
        finally
        {
            IsSigningIn = false;
            IsSubmitting = false;
        }
        // The TOML is the shape SubmitJmapSetup produces, so this is the same connect-then-persist
        // path a manual JMAP connect takes, including its own error handling, which keeps the
        // form up if the freshly-signed-in account can't actually connect.
        await AddAccountAsync(configToml);
        return JmapSignInOutcome.Added;
    }

    /// <summary>
    /// Signs an existing OAuth JMAP account back in, from the expired-sign-in banner. Unlike
    /// <see cref="SignInWithJmapAsync"/> there is no discovery and no registration: the core builds
    /// the authorization URL from that account's own persisted grant, and on completion connects,
    /// writes the Credential Manager and retracts the prompt itself, so there is no
    /// <c>AddAccountAsync</c> and no store write here. The account already exists.
    /// </summary>
    /// <remarks>
    /// Runs through the shared <see cref="SignInFlight"/> for the same reason the setup-form flow
    /// does: <c>ProtocolAuthCallback</c> holds a single static pending slot, and this button lives
    /// on the mail list, a screen that outlives any form, so a sign-in abandoned in the browser
    /// must be superseded by the next click rather than wedging it
    /// (<c>docs/provider-oauth.md</c> rule 13).
    /// </remarks>
    internal Task ReconnectJmapAsync(string accountId)
    {
        if (_connecting || _app is null)
        {
            return Task.CompletedTask;
        }
        return _signIn.RunAsync(cancel => ReconnectJmapFlowAsync(accountId, cancel));
    }

    private async Task ReconnectJmapFlowAsync(string accountId, CancellationToken cancelToken)
    {
        var app = _app!;
        IsSigningIn = true;
        try
        {
            using var callback = ProtocolAuthCallback.Expect(JmapOAuthConfig.CallbackHost);
            var start = await Task.Run(() => app.BeginJmapReauth(accountId));
            await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl));
            var callbackUrl = await callback.WaitAsync(cancelToken);
            // The exchange, the connect and the catch-up sync all block.
            await Task.Run(() => app.CompleteJmapReauth(accountId, start.Pending, callbackUrl));
            Log.Info("jmap re-authentication complete; the account is connected again");
        }
        catch (OperationCanceledException)
        {
            // Superseded by another sign-in, or cancelled. The banner is still up to try again.
            Log.Info("jmap re-authentication cancelled");
        }
        catch (Exception ex)
        {
            // Say so on the banner in plain words; the OAuth cause goes to the diagnostic log.
            // The core leaves the prompt raised on every failure, so the retry is one click away.
            Log.Error($"jmap re-authentication failed: {CoreError.Describe(ex)}");
            SetSignInReauthFailed(true);
        }
        finally
        {
            IsSigningIn = false;
        }
    }

    /// <summary>
    /// Aborts a JMAP sign-in that's waiting on the browser redirect (the user pressed Cancel).
    /// Safe to call when none is in flight.
    /// </summary>
    internal void CancelJmapSignIn() => _signIn.Cancel();
}
