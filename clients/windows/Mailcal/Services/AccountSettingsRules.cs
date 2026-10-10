// What Settings → Accounts says about an account, decided from its snapshot entry alone
// (docs/accounts.md rules 11 to 13). WinUI-free and L10n-free on purpose, so each rule is
// reachable from Mailcal.Tests: the words are the dialog's, the decisions are here.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which line, if any, sits under a use's switch.</summary>
internal enum UseNote
{
    None,
    /// <summary>The last of mail, calendar and contacts: the account is removed instead.</summary>
    Last,
    /// <summary>Colleagues, while contacts are off.</summary>
    NeedsContacts,
    /// <summary>Chosen, and the provider has not granted it.</summary>
    Withheld,
}

/// <summary>How one use's switch is drawn.</summary>
/// <param name="Active">On. A use waiting on a permission is drawn off: it is not working, and
/// switching it on is what asks the provider again.</param>
/// <param name="Asks">Whether switching it on asks the provider rather than the core.</param>
/// <param name="Enabled">Whether the person can flip it.</param>
internal sealed record UseSwitch(bool Active, bool Asks, bool Enabled, UseNote Note);

/// <summary>One link slot's picker: "none" first, then <see cref="Options"/>.</summary>
/// <param name="Selected">The index into <see cref="Options"/> of the account linked now; null
/// for "none".</param>
/// <param name="Suggested">The index of the account to suggest while nothing is linked: one whose
/// calendar server schedules as the mail account's address.</param>
internal sealed record LinkPicker(
    LinkSlot Slot, IReadOnlyList<LinkedAccount> Options, int? Selected, int? Suggested);

/// <summary>What the page does once switching a use has answered.</summary>
internal enum UseChangeOutcome
{
    /// <summary>Done; the page redraws from the next snapshot.</summary>
    Applied,
    /// <summary>The provider has not granted it: sign the account in again, asking for it.</summary>
    AskProvider,
    /// <summary>The account has no server for it.</summary>
    NeedsEndpoint,
}

internal static class AccountSettingsRules
{
    private static readonly AccountCapability[] Primary =
        [AccountCapability.Mail, AccountCapability.Calendar, AccountCapability.Contacts];

    /// <summary>The uses the account is used for, working or waiting on a permission, in the
    /// snapshot's order.</summary>
    internal static IReadOnlyList<AccountCapability> InUse(AccountEntry entry) =>
        entry.Uses.Where(use => use.State != CapabilityState.Off).Select(use => use.Capability).ToList();

    /// <summary>The links the account holds, as the list row names them.</summary>
    internal static IReadOnlyList<(AccountCapability Use, string Address)> Linked(AccountEntry entry)
    {
        var linked = new List<(AccountCapability, string)>();
        if (entry.Links.Calendar is { } calendar)
        {
            linked.Add((AccountCapability.Calendar, calendar.Address));
        }
        if (entry.Links.Contacts is { } contacts)
        {
            linked.Add((AccountCapability.Contacts, contacts.Address));
        }
        if (entry.Links.Mail is { } mail)
        {
            linked.Add((AccountCapability.Mail, mail.Address));
        }
        return linked;
    }

    /// <summary>Whether a use the account is used for waits on the provider's permission.</summary>
    internal static bool NeedsPermission(AccountEntry entry) =>
        entry.Uses.Any(use => use.State == CapabilityState.NeedsPermission);

    /// <summary>The uses waiting on the provider's permission, for the notice a sign-in that
    /// withheld some of them ends with.</summary>
    internal static IReadOnlyList<AccountCapability> Withheld(AccountEntry entry) =>
        entry.Uses.Where(use => use.State == CapabilityState.NeedsPermission)
            .Select(use => use.Capability).ToList();

    /// <summary>Whether the account signs in at its provider's own page, which "Sign in again"
    /// opens.</summary>
    internal static bool SignsInAtProvider(AccountKind kind) =>
        kind is AccountKind.Microsoft or AccountKind.Google;

    /// <summary>
    /// The switch for <paramref name="capability"/>. The last of mail, calendar and contacts
    /// cannot be switched off, and colleagues cannot be switched on without contacts: the core
    /// refuses both, so the switch says why rather than letting the person find out.
    /// </summary>
    internal static UseSwitch Switch(AccountEntry entry, AccountCapability capability)
    {
        bool Used(AccountCapability wanted) =>
            entry.Uses.Any(use => use.Capability == wanted && use.State != CapabilityState.Off);
        var state = entry.Uses.FirstOrDefault(use => use.Capability == capability)?.State
            ?? CapabilityState.Off;
        // A use waiting on a permission is drawn off, so it is never the one held on: switching it
        // on asks the provider, which the core does not refuse.
        var last = state == CapabilityState.On
            && Primary.Contains(capability)
            && Primary.Count(Used) == 1;
        var withoutContacts = capability == AccountCapability.Colleagues
            && state == CapabilityState.Off
            && !Used(AccountCapability.Contacts);
        var note = last ? UseNote.Last
            : withoutContacts ? UseNote.NeedsContacts
            : state == CapabilityState.NeedsPermission ? UseNote.Withheld
            : UseNote.None;
        return new UseSwitch(
            Active: state == CapabilityState.On,
            Asks: state == CapabilityState.NeedsPermission,
            Enabled: !last && !withoutContacts,
            Note: note);
    }

    /// <summary>
    /// The pickers an account's page offers: one per slot it can hold, which is a slot with
    /// candidates or one already linked. A link the candidates no longer offer is kept as the
    /// first option, so the picker shows what is stored rather than "none".
    /// </summary>
    internal static IReadOnlyList<LinkPicker> LinkPickers(AccountEntry entry)
    {
        var slots = new (LinkSlot Slot, LinkedAccount[] Candidates, LinkedAccount? Current)[]
        {
            (LinkSlot.Calendar, entry.LinkCandidates.Calendar, entry.Links.Calendar),
            (LinkSlot.Contacts, entry.LinkCandidates.Contacts, entry.Links.Contacts),
            (LinkSlot.Mail, entry.LinkCandidates.Mail, entry.Links.Mail),
        };
        var pickers = new List<LinkPicker>();
        foreach (var (slot, candidates, current) in slots)
        {
            if (candidates.Length == 0 && current is null)
            {
                continue;
            }
            var options = candidates.ToList();
            if (current is not null && !options.Contains(current))
            {
                options.Insert(0, current);
            }
            int? selected = current is null ? null : options.IndexOf(current);
            int? suggested = null;
            if (selected is null)
            {
                var at = options.FindIndex(option => entry.LinkCandidates.Suggested.Contains(option.Id));
                suggested = at < 0 ? null : at;
            }
            pickers.Add(new LinkPicker(slot, options, selected, suggested));
        }
        return pickers;
    }

    /// <summary>The accounts that lose their link when this one is removed, joined for the
    /// confirmation; null when none does.</summary>
    internal static string? Unlinked(AccountEntry entry) =>
        entry.LinkedFrom.Length == 0
            ? null
            : string.Join(", ", entry.LinkedFrom.Select(linked => linked.Address));

    /// <summary>What switching a use on or off answered.</summary>
    internal static UseChangeOutcome Outcome(CapabilityChange change) => change switch
    {
        CapabilityChange.NeedsConsent => UseChangeOutcome.AskProvider,
        CapabilityChange.NeedsEndpoint => UseChangeOutcome.NeedsEndpoint,
        _ => UseChangeOutcome.Applied,
    };

    /// <summary>
    /// What the page compares to decide whether a core signal changed anything on it: every field
    /// of every entry, which accounts' sign-in has expired, and <paramref name="drawnBeside"/>,
    /// what else the page draws from other calls (which accounts have a mailbox listed, how each
    /// is shared). A signal that changed none of it leaves the page, and whatever is being typed
    /// on it, alone.
    /// </summary>
    internal static string Fingerprint(
        AccountsSnapshot snapshot, IEnumerable<string> expired, IEnumerable<string>? drawnBeside = null)
    {
        var parts = new List<string>();
        foreach (var entry in snapshot.Accounts)
        {
            parts.Add(entry.Id);
            parts.Add(entry.Address);
            parts.Add(entry.Kind.ToString());
            parts.AddRange(entry.Uses.Select(use => $"{use.Capability}={use.State}"));
            parts.Add(Linked(entry.Links.Calendar));
            parts.Add(Linked(entry.Links.Contacts));
            parts.Add(Linked(entry.Links.Mail));
            parts.AddRange(entry.LinkedFrom.Select(Linked));
            parts.AddRange(entry.LinkCandidates.Calendar.Select(Linked));
            parts.AddRange(entry.LinkCandidates.Contacts.Select(Linked));
            parts.AddRange(entry.LinkCandidates.Mail.Select(Linked));
            parts.AddRange(entry.LinkCandidates.Suggested);
            parts.Add(entry.Endpoints is { } e
                ? $"{e.ImapHost}|{e.ImapSecurity}|{e.SmtpHost}|{e.SmtpSecurity}|{e.CaldavUrl}|{e.CarddavUrl}|{e.Username}"
                : "-");
        }
        parts.Add("expired");
        parts.AddRange(expired.Order(StringComparer.Ordinal));
        parts.Add("beside");
        parts.AddRange((drawnBeside ?? []).Order(StringComparer.Ordinal));
        return string.Join('\n', parts);
    }

    private static string Linked(LinkedAccount? linked) =>
        linked is null ? "-" : $"{linked.Id}={linked.Address}";
}
