// Public, render-ready types for the per-account "New mail" settings, the Windows mirror of
// the core's sync-settings snapshot. The generated UniFFI types are internal (and carry
// lowercase Rust field names), so MailboxModel projects the snapshot into these public POCOs,
// keeping the FFI types confined to the service layer (mirroring RowViewModels.cs).

namespace Allodia.Mailcal.ViewModels;

/// <summary>How an account receives new mail.</summary>
public enum SyncStrategyChoice
{
    /// <summary>Receive mail as it arrives via IMAP IDLE (only when the server supports it).</summary>
    Push,

    /// <summary>Check for new mail on a timer.</summary>
    Poll,
}

/// <summary>One folder of an account, with its push-subscription state.</summary>
public sealed class SyncFolderChoice
{
    /// <summary>The mailbox's provider key (passed back to the setter).</summary>
    public required string Key { get; init; }

    /// <summary>The folder's display name.</summary>
    public required string Name { get; init; }

    /// <summary>Whether this folder is watched for push.</summary>
    public bool Subscribed { get; init; }
}

/// <summary>One account's synchronisation-behaviour row.</summary>
public sealed class AccountSyncChoice
{
    /// <summary>The account's id (passed back to the setters).</summary>
    public required string AccountId { get; init; }

    /// <summary>The account's email address (display label).</summary>
    public required string Email { get; init; }

    /// <summary>Whether the server advertises IMAP IDLE, gates the push option.</summary>
    public bool IdleSupported { get; init; }

    /// <summary>The strategy currently in effect.</summary>
    public SyncStrategyChoice Strategy { get; init; }

    /// <summary>The poll interval in minutes (one of the snapshot's intervals).</summary>
    public ushort PollIntervalMins { get; init; }

    /// <summary>How far back this account syncs mail as a month count (<c>0</c> = all mail),
    /// one of the snapshot's <see cref="SyncSettingsChoices.SyncDepths"/>.</summary>
    public ushort SyncDepthMonths { get; init; }

    /// <summary>The largest message this account downloads in full in the background, as a
    /// megabyte count (<c>0</c> = no limit), one of the snapshot's
    /// <see cref="SyncSettingsChoices.MessageSizeLimitsMb"/>.</summary>
    public ushort MessageSizeLimitMb { get; init; }

    /// <summary>Whether the push-folder cap is reached (disables further selection).</summary>
    public bool AtPushLimit { get; init; }

    /// <summary>Every folder of the account, with its push-subscription state.</summary>
    public required IReadOnlyList<SyncFolderChoice> Folders { get; init; }
}

/// <summary>The whole per-account sync-settings snapshot for the settings dialog.</summary>
public sealed class SyncSettingsChoices
{
    /// <summary>One row per configured account.</summary>
    public required IReadOnlyList<AccountSyncChoice> Accounts { get; init; }

    /// <summary>The maximum folders an account may watch for push.</summary>
    public int MaxPushFolders { get; init; }

    /// <summary>The selectable poll intervals in minutes, in display order.</summary>
    public required IReadOnlyList<ushort> PollIntervals { get; init; }

    /// <summary>The selectable per-account fetch-depth options as month counts, in display order
    /// (<c>0</c> = all mail), the account cards build their depth picker from this.</summary>
    public required IReadOnlyList<ushort> SyncDepths { get; init; }

    /// <summary>The selectable per-account message-size options as megabyte counts, in display
    /// order (<c>0</c> = no limit), a client builds its picker from this.</summary>
    public required IReadOnlyList<ushort> MessageSizeLimitsMb { get; init; }
}
