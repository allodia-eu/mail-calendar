// Stands in for the core's account page and setup pickers, which this assembly cannot ask: it does
// not load the cdylib. Only the shape a test reads is filled in; the decisions behind each record
// are the core's, tested in mailcal-bindings (docs/waiting.md, "Enforcement").

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Tests;

internal static class CorePage
{
    private static readonly ControlState Free = new(true, false);

    /// <summary>A page for <paramref name="entry"/> with nothing running and nothing blocked.</summary>
    internal static AccountPage Of(AccountEntry entry) => new(
        entry.Id,
        [.. entry.Uses.Select(use => new AccountUseSwitch(
            use.Capability, use.State == CapabilityState.On, Free, null, null, false, true, false))],
        [],
        null,
        null,
        Free,
        null,
        null);

    /// <summary>One picker per slot with candidates and nothing linked, the suggestion marked, as
    /// <c>setup_link_pickers</c> answers for the entries these tests build.</summary>
    internal static IReadOnlyList<AccountLinkPicker> SetupPickers(AccountEntry entry)
    {
        var slots = new (LinkSlot Slot, LinkedAccount[] Candidates, LinkedAccount? Current)[]
        {
            (LinkSlot.Calendar, entry.LinkCandidates.Calendar, entry.Links.Calendar),
            (LinkSlot.Contacts, entry.LinkCandidates.Contacts, entry.Links.Contacts),
            (LinkSlot.Mail, entry.LinkCandidates.Mail, entry.Links.Mail),
        };
        return [.. slots
            .Where(slot => slot.Candidates.Length > 0 && slot.Current is null)
            .Select(slot =>
            {
                var at = Array.FindIndex(slot.Candidates, option => entry.LinkCandidates.Suggested.Contains(option.Id));
                return new AccountLinkPicker(slot.Slot, slot.Candidates, null, at < 0 ? null : (uint)at, Free);
            })];
    }
}
