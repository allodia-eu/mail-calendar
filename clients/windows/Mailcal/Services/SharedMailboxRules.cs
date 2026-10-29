// What the Shared mailboxes section of an account's page draws for the server's answer
// (docs/shared-mailboxes.md rules 3, 7 and 8), and when the page asks the server again.
// WinUI-free and L10n-free on purpose, so each rule is reachable from Mailcal.Tests: the words are
// the dialog's, the decisions are here.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which line, if any, sits under the section's switches.</summary>
internal enum SharedNote
{
    /// <summary>Every mailbox the server listed is a switch above it.</summary>
    None,
    /// <summary>The server lists them, and lists none.</summary>
    NoneShared,
    /// <summary>The server lists none and opens one at an address the person types.</summary>
    ByAddress,
    /// <summary>The account is not connected yet, so there is nothing to ask.</summary>
    NotConnected,
    /// <summary>The server was asked and did not answer.</summary>
    Unreachable,
}

/// <summary>One mailbox's switch.</summary>
/// <param name="Title">Its address, or the server's label where it has none.</param>
/// <param name="Label">The server's label beside the address, when it says something else.</param>
/// <param name="Withdrawn">Chosen, and no longer listed by the server: the line says so in place
/// of the label.</param>
internal sealed record SharedRow(string Title, string? Label, bool Withdrawn, bool Active);

internal static class SharedMailboxRules
{
    /// <summary>The switch for <paramref name="mailbox"/>.</summary>
    internal static SharedRow Row(SharedMailboxEntry mailbox)
    {
        var title = mailbox.Address ?? mailbox.Name;
        return new SharedRow(
            title,
            mailbox.Name != title ? mailbox.Name : null,
            Withdrawn: !mailbox.Listed,
            Active: mailbox.Selected);
    }

    /// <summary>The line under the switches.</summary>
    internal static SharedNote Note(SharedMailboxesView view) => view.Support switch
    {
        SharedMailboxSupport.Listed => view.Mailboxes.Length == 0 ? SharedNote.NoneShared : SharedNote.None,
        SharedMailboxSupport.ByAddress => SharedNote.ByAddress,
        SharedMailboxSupport.NotConnected => SharedNote.NotConnected,
        SharedMailboxSupport.Unreachable => SharedNote.Unreachable,
        _ => SharedNote.None,
    };

    /// <summary>Whether the page draws the section: once the server has answered, for an account
    /// whose sign-in can open a shared mailbox or that still has one chosen, so an account that
    /// cannot has no section that says nothing.</summary>
    internal static bool Shown(SharedMailboxesView? view) =>
        view is not null && (view.Support != SharedMailboxSupport.Unsupported || view.Mailboxes.Length > 0);

    /// <summary>Whether the section offers a field for an address: on a server that lists none.</summary>
    internal static bool TakesAddress(SharedMailboxesView view) => view.Support == SharedMailboxSupport.ByAddress;

    /// <summary>
    /// Whether asking again may answer something new: the account was not connected yet, or could
    /// not open a shared mailbox, which a use switched on or a sign-in completed changes; or its
    /// uses changed since it was asked, since switching mail or the Microsoft use off removes every
    /// chosen mailbox (docs/shared-mailboxes.md rule 2). A listing is a request to the server, so a
    /// page that has one does not ask on every signal.
    /// </summary>
    internal static bool MayHaveChanged(SharedMailboxesView? answer, bool asking, bool usesChanged) =>
        !asking
            && (usesChanged || answer?.Support is null or SharedMailboxSupport.NotConnected or SharedMailboxSupport.Unsupported);

    /// <summary>What the account is used for, as the page compares it from one signal to the next.</summary>
    internal static string Uses(AccountEntry entry) => string.Join(",", AccountSettingsRules.InUse(entry));

    /// <summary>Whether an answer replaces the one kept: only when its ask started after the kept
    /// one's. A switch or an add asks again and applies its answer while the page's own ask may
    /// still be out, and that older answer landing later would put back what the switch changed.</summary>
    /// <param name="ticket">When this answer's ask started, from a counter taken before each ask.</param>
    /// <param name="kept">When the kept answer's ask started; null when none is kept.</param>
    internal static bool Fresher(long ticket, long? kept) => kept is null || ticket > kept;

    /// <summary>Whether two answers draw the same section; the generated record compares its array
    /// by reference.</summary>
    internal static bool Same(SharedMailboxesView? a, SharedMailboxesView? b) =>
        a is null || b is null
            ? a is null && b is null
            : a.Support == b.Support && a.Mailboxes.SequenceEqual(b.Mailboxes);
}
