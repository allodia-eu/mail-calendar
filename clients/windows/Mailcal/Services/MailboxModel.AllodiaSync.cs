// Keeping this device's account list in step with the person's other devices. The core runs the
// pass and keeps what it found (docs/settings.md category 10, docs/waiting.md); what is here is the
// store it remembers in, and reading what it says.

using System;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// What the person's other devices have to say about this device's accounts; null until a pass
    /// has answered, and while nobody is signed in. Read again on every Settings signal: the core
    /// raises one when a pass starts and when it ends.
    /// </summary>
    internal AllodiaDevices? AllodiaDevices() => _app?.AllodiaDevices();

    /// <summary>A pass started or ended (<c>Surface.OtherDevices</c>): what
    /// <see cref="AllodiaDevices"/> answers may have moved, and nothing else has. A Keep on a
    /// question is a page change, so it moves on <see cref="AccountsChanged"/> as before.</summary>
    public event Action? OtherDevicesChanged;

    /// <summary>
    /// Hands the core somewhere to remember what it has synced, which is also what lets it run a
    /// pass. Never on a launch whose accounts are not the person's own: a harness mailbox sent up
    /// would land on the developer's own phone.
    /// </summary>
    private void InstallAllodiaSyncStore(MailcalApp app)
    {
        if (UsesCannedAccounts)
        {
            Log.Info("allodia: a harness launch shares nothing with the other devices");
            return;
        }
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
    /// Whether this launch connected canned harness accounts rather than the person's own. The
    /// showcase never reaches <see cref="InstallAllodiaSyncStore"/> at all.
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
