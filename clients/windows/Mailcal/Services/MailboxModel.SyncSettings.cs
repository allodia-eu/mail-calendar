// The per-account sync-settings surface of the model: projects the core's (internal) sync
// snapshot into the public SyncSettingsChoices the dialog renders, and forwards the setters
// to the Rust app. Split into its own partial to keep MailboxModel.cs under the 500-line
// limit. State lives in Rust; the core re-signals Surface.Settings after each setter.

using System.Linq;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// The current per-account sync-behaviour settings, projected from the core snapshot, or
    /// <c>null</c> before the app has connected.
    /// </summary>
    public SyncSettingsChoices? GetSyncSettings()
    {
        if (_app is not { } app)
        {
            return null;
        }
        var snapshot = app.SyncSettings();
        var accounts = snapshot.Accounts.Select(account => new AccountSyncChoice
        {
            AccountId = account.AccountId,
            Email = account.Email,
            IdleSupported = account.IdleSupported,
            Strategy = account.Strategy == SyncStrategyKind.Push
                ? SyncStrategyChoice.Push
                : SyncStrategyChoice.Poll,
            PollIntervalMins = account.PollIntervalMins,
            SyncDepthMonths = account.SyncDepthMonths,
            MessageSizeLimitMb = account.MessageSizeLimitMb,
            AtPushLimit = account.AtPushLimit,
            Folders = account.Folders.Select(folder => new SyncFolderChoice
            {
                Key = folder.Key,
                Name = FolderLabel.For(folder.Role, folder.Name),
                Subscribed = folder.Subscribed,
            }).ToList(),
        }).ToList();
        return new SyncSettingsChoices
        {
            Accounts = accounts,
            MaxPushFolders = snapshot.MaxPushFolders,
            PollIntervals = snapshot.PollIntervals.ToList(),
            SyncDepths = snapshot.SyncDepths.ToList(),
            MessageSizeLimitsMb = snapshot.MessageSizeLimitsMb.ToList(),
        };
    }

    /// <summary>Sets one account's fetch depth (a month count; <c>0</c> = all mail) and reconnects
    /// that account with the new window (widening fetches older mail, narrowing stops fetching it).</summary>
    public void SetAccountSyncDepthChoice(string account, ushort months) =>
        _app?.SetAccountSyncDepth(account, months);

    /// <summary>Sets one account's message-size cap (a megabyte count; <c>0</c> = no limit).
    /// Raising it downloads what the lower cap skipped; lowering it forgets the cached copies it
    /// may no longer keep. The mail itself is never removed either way.</summary>
    public void SetAccountMessageSizeChoice(string account, ushort megabytes) =>
        _app?.SetAccountMessageSizeLimit(account, megabytes);

    /// <summary>Switches an account between push (IMAP IDLE) and interval polling.</summary>
    public void SetSyncStrategyChoice(string account, bool push) =>
        _app?.SetSyncStrategy(account, push ? SyncStrategyKind.Push : SyncStrategyKind.Poll);

    /// <summary>Sets an account's background-poll interval (minutes).</summary>
    public void SetPollIntervalChoice(string account, ushort minutes) =>
        _app?.SetPollInterval(account, minutes);

    /// <summary>Subscribes or unsubscribes one folder for push on an account.</summary>
    public void SetPushFolderChoice(string account, string folder, bool subscribed) =>
        _app?.SetPushFolder(account, folder, subscribed);
}
