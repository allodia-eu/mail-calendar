// Keeping this device's mail-account list in step with the person's other devices. The core does
// the deciding and the writing; what is here is when to ask it, and what to do with the part it
// cannot answer alone.
//
// The pass BLOCKS on the network, so it runs off the UI thread, like every other core call that
// reaches a server.

using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// What the person's other devices have to say, or <c>null</c> until a pass has run, which is
    /// not the same as a pass that found nothing.
    /// </summary>
    internal AllodiaSyncReport? AllodiaSync { get; private set; }

    /// <summary>The last pass's failure, in the service's own words, or <c>null</c>.</summary>
    /// <remarks>
    /// It says a pass failed; it does not decide what the person is told. That is
    /// <see cref="AllodiaGrantHealth"/>'s, because a failure's text is the core's own diagnostic
    /// and putting it on screen is how a generated field name became product copy.
    /// </remarks>
    internal string? AllodiaSyncFailure { get; private set; }

    /// <summary>
    /// What the core knows about this device's Allodia sign-in, the typed answer a screen draws
    /// from.
    /// </summary>
    /// <remarks>
    /// Read through rather than cached: it is a local field on the core, it changes underneath
    /// whenever a call learns something, and a copy here would be one more thing to keep in step.
    /// <see cref="AllodiaGrantHealth.Ok"/> when there is no app yet, which draws nothing.
    /// </remarks>
    internal AllodiaGrantHealth AllodiaGrantHealth =>
        _app?.AllodiaGrantHealth() ?? AllodiaGrantHealth.Ok;

    /// <summary>A pass is in flight, so a second must not start and race its writes.</summary>
    internal bool AllodiaSyncing { get; private set; }

    /// <summary>
    /// Hands the core somewhere to remember what it has synced. Called once the app is built and
    /// before anything can ask for a pass; unlike the Credential Manager writer it is not racing a
    /// dial, because nothing syncs until somebody asks.
    /// </summary>
    private void InstallAllodiaSyncStore(MailcalApp app)
    {
        try
        {
            app.UseAllodiaSyncStateStore(new FileSyncStateStore());
        }
        catch (Exception e)
        {
            // The blob could not be read. Syncing is off for this launch rather than starting from
            // nothing, which would re-adopt every record and re-offer every account.
            Log.Warn($"allodia: the sync state could not be read ({CoreError.Describe(e)}); not syncing");
        }
    }

    /// <summary>
    /// Runs one pass, if there is any point in running one: nobody signed in, or a harness launch
    /// on canned accounts, and there is nothing worth syncing.
    /// </summary>
    internal async Task SyncAllodiaAccountsAsync()
    {
        if (_app is null || AllodiaSyncing || UsesCannedAccounts)
        {
            return;
        }
        if (SignedInAllodiaAccount() is null)
        {
            return;
        }
        AllodiaSyncing = true;
        AllodiaSyncFailure = null;
        try
        {
            var report = await Task.Run(() => _app!.SyncAllodiaAccounts());
            Log.Info(
                $"allodia: sync done, {report.Sent} sent, {report.Offers.Length} offered, "
                + $"{report.ChangedElsewhere.Length} changed elsewhere, "
                + $"{report.RemovedElsewhere.Length} removed elsewhere");
            AllodiaSync = report;
        }
        catch (Exception e)
        {
            Log.Warn($"allodia: the sync pass did not finish ({CoreError.Describe(e)})");
            AllodiaSyncFailure = CoreError.Describe(e);
        }
        finally
        {
            AllodiaSyncing = false;
        }
    }

    /// <summary>
    /// How each account is shared with the other devices, keyed by account id, what the
    /// per-account three-position control draws.
    /// </summary>
    internal IReadOnlyDictionary<string, AllodiaAccountSyncMode> AccountsSyncMode
    { get; private set; } = new Dictionary<string, AllodiaAccountSyncMode>();

    /// <summary>
    /// Moves one account to a sync position. Returns the failure text, or <c>null</c> on success.
    /// </summary>
    /// <remarks>
    /// The core does everything the position takes, including reaching the service, so this runs
    /// off the UI thread. The position is re-read from the core rather than assumed, so a change
    /// the service refused leaves the control where it was instead of lying about what happened.
    /// </remarks>
    internal async Task<string?> SetAllodiaAccountSyncModeAsync(
        string accountId, AllodiaAccountSyncMode mode)
    {
        if (_app is null)
        {
            return null;
        }
        string? failure = null;
        try
        {
            await Task.Run(() => _app!.SetAllodiaAccountSyncMode(accountId, mode));
            if (AllodiaSync is { } report)
            {
                AllodiaSync = new AllodiaSyncReport(
                    report.Offers,
                    report.ChangedElsewhere.Where(c => c.AccountId != accountId).ToArray(),
                    report.RemovedElsewhere.Where(c => c.AccountId != accountId).ToArray(),
                    report.Sent);
            }
        }
        catch (Exception e)
        {
            Log.Warn($"allodia: the account's sync position could not be set ({CoreError.Describe(e)})");
            failure = CoreError.Describe(e);
        }
        ReadAccountsSynced();
        return failure;
    }

    /// <summary>
    /// Re-reads how each account is shared. A local read per account; it never asks the service.
    /// </summary>
    internal void ReadAccountsSynced()
    {
        // Empty in a build with no Allodia registration, which is what draws no control at all.
        // The bookkeeping store is installed either way, so it would otherwise answer for every
        // account and the whole block would appear.
        if (_app is null || !MailcalBindingsMethods.AllodiaSignInAvailable())
        {
            AccountsSyncMode = new Dictionary<string, AllodiaAccountSyncMode>();
            return;
        }
        var accounts = GetSyncSettings()?.Accounts ?? Array.Empty<AccountSyncChoice>();
        AccountsSyncMode = accounts
            .ToDictionary(a => a.AccountId, a => _app.AllodiaAccountSyncMode(a.AccountId));
    }

    /// <summary>
    /// The account list changed, so the person's other devices should hear about it now rather
    /// than at the next launch. A no-op when nobody is signed in.
    /// </summary>
    internal void SyncAfterAccountChange()
    {
        ReadAccountsSynced();
        _ = SyncAllodiaAccountsAsync();
    }

    /// <summary>
    /// Forgets what the other devices said. Called on sign-out: there is nothing left to say about
    /// them once this device leaves the account that linked them.
    /// </summary>
    internal void ForgetAllodiaSync()
    {
        AllodiaSync = null;
        AllodiaSyncFailure = null;
    }

    /// <summary>
    /// Whether this launch connected canned harness accounts rather than the person's own. Such a
    /// launch must not sync: sending a harness mailbox up would put it on the developer's own
    /// phone.
    /// </summary>
    /// <remarks>
    /// The question is what was injected, not which namespace this is. <c>first-run</c> is a dev
    /// namespace that injects nothing, and it is the only way to reach the screen where signing in
    /// runs a pass, so a check on the namespace alone would make that screen untestable.
    /// </remarks>
    private static bool UsesCannedAccounts
    {
#if DEBUG
        get => IsHarnessDevAccount;
#else
        get => false;
#endif
    }
}
