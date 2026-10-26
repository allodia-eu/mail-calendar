// Which sentence the contacts list carries about the most recent save (docs/contacts.md §3, "What
// the status means"). WinUI-free and L10n-free, so every value of the core's ContactWriteStatus is
// pinned in Mailcal.Tests; the words are MailboxModel.ContactWrites.cs.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The line beside the contacts list.</summary>
internal enum ContactWriteLine
{
    /// <summary>Nothing to say. <c>Invalid</c> lands here too: it is stated under the form.</summary>
    None,
    Saving,
    Saved,
    /// <summary>"Couldn't save this contact": nothing changed on the server.</summary>
    NotSaved,
    /// <summary>"Couldn't confirm this was saved": the change may have landed.</summary>
    Unconfirmed,
}

internal static class ContactWriteLines
{
    internal static ContactWriteLine Of(ContactWriteStatus status) => status switch
    {
        ContactWriteStatus.Saving => ContactWriteLine.Saving,
        ContactWriteStatus.Saved => ContactWriteLine.Saved,
        ContactWriteStatus.NotSaved => ContactWriteLine.NotSaved,
        ContactWriteStatus.Unconfirmed => ContactWriteLine.Unconfirmed,
        ContactWriteStatus.Idle or ContactWriteStatus.Invalid => ContactWriteLine.None,
        _ => ContactWriteLine.None,
    };
}
