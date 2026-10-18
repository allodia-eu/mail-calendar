// The separate-server form's sign-in at the server's own provider (docs/mail-oauth.md rule 13):
// whether the server offers one, and the browser flow that moves the uses to an account of their
// own holding the grant, as a password's Connect does. Through the shared SignInFlight, because
// every protocol-activation flow shares one redirect slot.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Whether <paramref name="step"/>'s server signs in at its own provider. Blocking in
    /// the core (it reads the server's metadata and its issuer's), so off the UI thread; never
    /// throws, and anything that goes wrong reads as "no", which leaves the password form.</summary>
    internal async Task<bool> SeparateServerOffersSignInAsync(SeparateServerStep step)
    {
        if (_app is not { } app)
        {
            return false;
        }
        try
        {
            return await Task.Run(() => app.SeparateServerOffersSignIn(step.AccountId, step.Uses.ToArray()));
        }
        catch (Exception ex)
        {
            // The core answers false for every ordinary failure; a panic lands here.
            Log.Warn($"asking whether a calendar server signs in at its own provider failed: {CoreError.Describe(ex)}");
            return false;
        }
    }

    /// <summary>
    /// Signs <paramref name="step"/>'s uses in at their server's own provider, as a new attempt of
    /// <paramref name="signIn"/> that supersedes any still out: the core finds the issuer and
    /// registers, the browser opens, and the redirect completes the move. <paramref name="progress"/>
    /// hears each phase of the current attempt, on the UI thread, so the form disables itself only
    /// during the blocking core calls.
    /// </summary>
    /// <returns>What the form does; null for an attempt superseded or retired.</returns>
    internal async Task<SeparateServerNext?> SeparateServerSignInAsync(
        SeparateServerStep step, SeparateServerSignIn signIn, Action<SeparateSignInPhase> progress)
    {
        var attempt = signIn.Start();
        progress(SeparateSignInPhase.Starting);
        var end = _app is { } app
            ? await _signIn.RunAsync(cancel => SeparateServerSignInFlowAsync(app, step, Advance, cancel))
            : SeparateSignInEnd.Failed(L10n.SetupImapSigninFailed());
        var next = signIn.Ended(attempt, end);
        progress(signIn.Phase);
        return next;

        void Advance(SeparateSignInPhase phase)
        {
            if (signIn.Advanced(attempt, phase))
            {
                progress(phase);
            }
        }
    }

    /// <summary>Retires the separate-server sign-in still out: the form's way back.</summary>
    internal void CancelSeparateServerSignIn() => _signIn.Cancel();

    private async Task<SeparateSignInEnd> SeparateServerSignInFlowAsync(
        MailcalApp app, SeparateServerStep step, Action<SeparateSignInPhase> progress, CancellationToken cancel)
    {
        // Only what this flow set is cleared: a Connect pressed while the browser was open
        // retires this flow and sets IsSubmitting of its own.
        var submitting = false;
        IsSigningIn = true;
        try
        {
            using var callback = ProtocolAuthCallback.Expect(ImapOAuthConfig.CallbackHost);
            var start = await Task.Run(() =>
                app.BeginSeparateServerSignIn(step.AccountId, step.Uses.ToArray(), ImapOAuthConfig.RedirectUri));
            // A begin that answers after the way back opens no browser.
            cancel.ThrowIfCancellationRequested();
            progress(SeparateSignInPhase.Waiting);
            if (!await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl)))
            {
                Log.Warn("the browser did not open for a calendar server's sign-in");
                return SeparateSignInEnd.Failed(L10n.SetupImapSigninFailed());
            }
            var callbackUrl = await callback.WaitAsync(cancel);
            // A redirect that lands after the way back adds no account.
            cancel.ThrowIfCancellationRequested();
            // The exchange and two connects: bounded, so no way back, and Dismiss keeps the form.
            IsSigningIn = false;
            IsSubmitting = submitting = true;
            progress(SeparateSignInPhase.Completing);
            var moved = await Task.Run(() =>
                app.CompleteSeparateServerSignIn(step.AccountId, start.Pending, callbackUrl));
            Log.Info($"an account's calendar and contacts signed in at their own provider as {LogHandle(moved)}, linked to {LogHandle(step.AccountId)}");
            SyncAfterAccountChange();
            if (cancel.IsCancellationRequested)
            {
                // Superseded by a sign-in elsewhere while it completed: the move stands, and the
                // snapshot shows it, but the form this attempt belonged to has moved on.
                return SeparateSignInEnd.Retired;
            }
            return SeparateSignInEnd.Added(moved);
        }
        catch (OperationCanceledException)
        {
            Log.Info("a calendar server's sign-in was cancelled");
            return SeparateSignInEnd.Retired;
        }
        catch (TimeoutException)
        {
            Log.Warn("a calendar server's sign-in timed out waiting for the browser");
            return SeparateSignInEnd.Failed(L10n.SetupImapSigninFailed());
        }
        catch (Exception ex)
        {
            Log.Error($"a calendar server's sign-in failed: {CoreError.Describe(ex)}");
            return SeparateSignInEnd.Failed(
                SeparateServerSignIn.Said(ex, L10n.SetupImapSigninFailed(), L10n.SigninDifferentAccount()));
        }
        finally
        {
            IsSigningIn = false;
            if (submitting)
            {
                IsSubmitting = false;
            }
        }
    }

    /// <summary>The sign-in on the setup's separate-server step: on to the link step once it has
    /// moved the uses, exactly as Connect's success does.</summary>
    /// <returns>What the step does; null for an attempt superseded or retired.</returns>
    internal async Task<SeparateServerNext?> SetupSeparateServerSignInAsync(
        SeparateServerSignIn signIn, Action<SeparateSignInPhase> progress)
    {
        if (_setupFlow.SeparateServer is not { } step || IsSubmitting)
        {
            return null;
        }
        var next = await SeparateServerSignInAsync(step, signIn, progress);
        if (next is { GoesOn: true } && ReferenceEquals(_setupFlow.SeparateServer, step))
        {
            SeparateServerDone();
        }
        return next;
    }
}
