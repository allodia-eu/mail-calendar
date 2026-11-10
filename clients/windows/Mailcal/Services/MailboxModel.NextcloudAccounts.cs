// A mailbox's calendar and contacts moved to an account of their own at a Nextcloud server, from
// the separate-server step (docs/nextcloud-sign-in.md rule 2). The browser half is
// MailboxModel.Nextcloud.cs's. A calendar and contacts account that signs in with Nextcloud again,
// or instead of its typed password, is the core's browser sign-in (MailboxModel.BrowserSignIn.cs).
//
// A grant the core could not complete is kept for the next press, which tries it again without the
// browser, because the server hands it out once; leaving the surface withdraws it.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
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
