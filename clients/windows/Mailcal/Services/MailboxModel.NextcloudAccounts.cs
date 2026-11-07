// A Nextcloud sign-in for an account that exists (docs/nextcloud-sign-in.md rules 2, 7 and 8): a
// calendar and contacts account signing in again, or instead of its typed password, from its page or
// from the banner of a refused sign-in; and a mailbox's calendar and contacts moved to an account of
// their own at a Nextcloud server, from the separate-server step. The browser half is
// MailboxModel.Nextcloud.cs's.
//
// A grant the core could not complete is kept for the next press, which tries it again without the
// browser, because the server hands it out once; leaving the surface withdraws it.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // A grant an account's page or the banner could not put on its account yet, held for it.
    private readonly NextcloudGrantHold _accountNextcloud = new();

    /// <summary>
    /// Signs the calendar and contacts account <paramref name="accountId"/> in with Nextcloud,
    /// again or in place of its typed password. <paramref name="step"/> hears where it is, for a
    /// page drawing its controls from it (<see cref="AccountSignInProgress"/>).
    /// </summary>
    internal Task<ConsentEnded> AccountNextcloudSignInAsync(string accountId, Action<SignInStep>? step = null)
    {
        if (_connecting || _app is not { } app)
        {
            return Task.FromResult(new ConsentEnded(false, L10n.AppUnavailable()));
        }
        return _signIn.RunAsync(cancel => AccountNextcloudFlowAsync(app, accountId, step ?? (_ => { }), cancel));
    }

    private async Task<ConsentEnded> AccountNextcloudFlowAsync(
        MailcalApp app, string accountId, Action<SignInStep> step, CancellationToken cancel)
    {
        IsSigningIn = true;
        try
        {
            if (_accountNextcloud.HeldFor(accountId) is not { } grant)
            {
                // A grant held for another account becomes no account now.
                AbandonNextcloudGrant(_accountNextcloud.Release());
                var end = await FlyNextcloudAsync(
                    app, core => core.BeginAccountNextcloudSignIn(accountId), cancel, () => step(SignInStep.Browser));
                if (end is not NextcloudFlightEnd.Granted granted)
                {
                    return new ConsentEnded(false, NextcloudSaid(end), (end as NextcloudFlightEnd.Failed)?.Error);
                }
                grant = granted.Grant;
            }
            // The connect and storing the new password block, and nothing after this retires it.
            step(SignInStep.Calling);
            _accountNextcloud.Complete(accountId, grant);
            try
            {
                await Task.Run(() => app.CompleteAccountNextcloudSignIn(accountId, grant));
                _accountNextcloud.Completed(spent: true);
            }
            catch (Exception ex)
            {
                // One for somebody else the core has withdrawn already; any other is kept.
                _accountNextcloud.Completed(spent: ex is MailcalException.DifferentAccount);
                Log.Error($"an account's Nextcloud sign-in failed: {CoreError.Describe(ex)}");
                return ConsentEnded.Failed(ex);
            }
            Log.Info($"an account was signed in with Nextcloud in place: {LogHandle(accountId)}");
            return new ConsentEnded(true, null);
        }
        finally
        {
            IsSigningIn = false;
        }
    }

    /// <summary>Withdraws the grant an account's page held for another try: Settings closed.</summary>
    internal void ReleaseAccountNextcloudGrant() => AbandonNextcloudGrant(_accountNextcloud.Release());

    /// <summary>Whether <paramref name="step"/>'s server is Nextcloud. Blocking in the core, so off
    /// the UI thread; never throws, and anything that goes wrong reads as "no".</summary>
    internal async Task<bool> SeparateServerOffersNextcloudSignInAsync(SeparateServerStep step)
    {
        if (_app is not { } app)
        {
            return false;
        }
        try
        {
            return await Task.Run(() => app.SeparateServerOffersNextcloudSignIn(step.AccountId, step.Uses.ToArray()));
        }
        catch (Exception ex)
        {
            Log.Warn($"asking whether a calendar server is Nextcloud failed: {CoreError.Describe(ex)}");
            return false;
        }
    }

    /// <summary>
    /// The separate-server step's sign-in when its server is Nextcloud: a grant the form held from
    /// the last press goes straight to the core; otherwise the browser is asked for one. The uses
    /// then move to an account of their own, linked to the mailbox, as a password's Connect does.
    /// </summary>
    private async Task<SeparateSignInEnd> SeparateServerNextcloudFlowAsync(
        MailcalApp app, SeparateServerStep step, NextcloudGrantHold hold, Action<SeparateSignInPhase> progress,
        CancellationToken cancel)
    {
        var key = SeparateServerSignIn.Key(step);
        var uses = step.Uses.ToArray();
        // Only what this flow set is cleared: a Connect pressed while the browser was open retires
        // this flow and sets IsSubmitting of its own.
        var submitting = false;
        IsSigningIn = true;
        try
        {
            if (hold.HeldFor(key) is not { } grant)
            {
                AbandonNextcloudGrant(hold.Release());
                var end = await FlyNextcloudAsync(
                    app,
                    core => core.BeginSeparateServerNextcloudSignIn(step.AccountId, uses),
                    cancel,
                    () => progress(SeparateSignInPhase.Waiting));
                if (end is not NextcloudFlightEnd.Granted granted)
                {
                    return NextcloudSaid(end) is { } said ? SeparateSignInEnd.Failed(said) : SeparateSignInEnd.Retired;
                }
                grant = granted.Grant;
            }
            // Two connects: bounded, so no way back.
            IsSigningIn = false;
            IsSubmitting = submitting = true;
            progress(SeparateSignInPhase.Completing);
            hold.Complete(key, grant);
            string moved;
            try
            {
                moved = await Task.Run(() => app.AddLinkedNextcloudAccount(step.AccountId, uses, grant));
                hold.Completed(spent: true);
            }
            catch (Exception ex)
            {
                // Both accounts are unchanged, and the grant is kept for the next press.
                hold.Completed(spent: false);
                Log.Error($"a calendar server's Nextcloud sign-in failed: {CoreError.Describe(ex)}");
                return SeparateSignInEnd.Failed(
                    SeparateServerSignIn.Said(ex, L10n.SetupImapSigninFailed(), CoreErrorCopy.Words()));
            }
            Log.Info($"an account's calendar and contacts signed in with Nextcloud as {LogHandle(moved)}, linked to {LogHandle(step.AccountId)}");
            SyncAfterAccountChange();
            return cancel.IsCancellationRequested ? SeparateSignInEnd.Retired : SeparateSignInEnd.Added(moved);
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
}
