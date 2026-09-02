// Asking a mail server what it accepts, and the browser sign-in when the answer is "sign in".
// The twin of MailboxModel.Jmap.cs, and the same two rules shape it:
//
//   - the setup form asks the server BEFORE it draws a credential field (ImapAuthOptionsAsync),
//     because a sign-in offered on any other evidence mints a token the mailbox refuses;
//   - a failure is NOT an error banner. "This provider does not do this" is an expected outcome
//     and the password field still works, so the outcome comes back to the view, which says so
//     inline and leaves that route alone.

using System;
using System.Threading;
using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// What this mail server accepts, the pre-flight that decides what the setup form asks for.
    /// Blocking in the core (it dials the mail server, then may read metadata), so it runs off the
    /// UI thread; it never throws, and anything that goes wrong reads as "ask for a password",
    /// which is what works everywhere.
    /// </summary>
    internal async Task<ImapAuthAnswer> ImapAuthOptionsAsync(ImapLoginRequest request)
    {
        var app = _app;
        if (app is null)
        {
            return ImapAuthAnswer.Password;
        }
        try
        {
            var offer = await Task.Run(() => app.ImapAuthOptions(request)).ConfigureAwait(false);
            return offer switch
            {
                ImapAuthOffer.SignIn signIn => signIn.PasswordAlsoWorks
                    ? ImapAuthAnswer.SignInOrPassword
                    : ImapAuthAnswer.SignInOnly,
                ImapAuthOffer.RegistrationNeeded => ImapAuthAnswer.RegistrationNeeded,
                _ => ImapAuthAnswer.Password,
            };
        }
        catch (Exception ex)
        {
            // The core answers `Password` for every ordinary failure, so nothing should land here,
            // but this runs fire-and-forget off a timer where an exception would otherwise be
            // swallowed unobserved. Log it and ask for a password.
            Log.Warn($"imap auth pre-flight failed: {CoreError.Describe(ex)}");
            return ImapAuthAnswer.Password;
        }
    }

    /// <summary>
    /// Runs the IMAP sign-in: finds and registers with the authorization server, opens the
    /// authorization URL in the browser, captures the redirect, exchanges the code, then adds and
    /// stores the account through the same path a password connect takes. Returns what happened,
    /// so the form can show its inline note without a raw protocol error.
    /// </summary>
    /// <remarks>
    /// Through the shared <see cref="SignInFlight"/> like every other browser flow, for the reason
    /// recorded on <see cref="SignInWithJmapAsync"/>: <c>ProtocolAuthCallback</c> holds a single
    /// static pending slot, so two flows genuinely compete for one redirect rendezvous.
    /// </remarks>
    internal Task<ImapSignInOutcome> SignInWithImapAsync(ImapLoginRequest request)
    {
        // A connect already in flight, or an engine that never opened: nothing to report (the
        // failed launch already surfaced its own error), so don't raise the note.
        if (_connecting || _app is null)
        {
            return Task.FromResult(ImapSignInOutcome.Cancelled);
        }
        return _signIn.RunAsync(cancel => SignInWithImapFlowAsync(request, cancel));
    }

    private async Task<ImapSignInOutcome> SignInWithImapFlowAsync(
        ImapLoginRequest request, CancellationToken cancelToken)
    {
        var app = _app!;
        IsSigningIn = true;
        IsSubmitting = true;
        SetupError = null;
        string configToml;
        try
        {
            // Arm the redirect rendezvous before opening the browser: the browser returns to our
            // custom scheme, delivered here as a protocol activation routed by Program into
            // ProtocolAuthCallback.
            using var callback = ProtocolAuthCallback.Expect(ImapOAuthConfig.CallbackHost);
            // Discovery and dynamic client registration are several network round trips, so they
            // block: off the UI thread with them.
            var start = await Task.Run(() => app.BeginImapLogin(request, ImapOAuthConfig.RedirectUri));
            // Open the default browser, where the person is usually already signed in.
            await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl));
            var callbackUrl = await callback.WaitAsync(cancelToken);
            // The token exchange blocks too.
            configToml = await Task.Run(() => app.CompleteImapLogin(start.Pending, callbackUrl));
        }
        catch (OperationCanceledException)
        {
            // Cancel was pressed while the browser step was outstanding: return to the form
            // quietly; the disposed callback slot ignores any late redirect.
            Log.Info("imap sign-in cancelled by user");
            return ImapSignInOutcome.Cancelled;
        }
        catch (Exception ex)
        {
            // A provider need not offer any of this, so the form says so in plain words and keeps
            // the password field; the specific cause stays in the diagnostic log rather than in
            // front of the person.
            Log.Error($"imap sign-in failed: {CoreError.Describe(ex)}");
            return ImapSignInOutcome.Failed;
        }
        finally
        {
            IsSigningIn = false;
            IsSubmitting = false;
        }
        // The TOML is the shape the password form produces, so this is the same
        // connect-then-persist path a manual connect takes, including its own error handling,
        // which keeps the form up if the freshly-signed-in account cannot actually connect.
        await AddAccountAsync(configToml);
        return ImapSignInOutcome.Added;
    }
}
