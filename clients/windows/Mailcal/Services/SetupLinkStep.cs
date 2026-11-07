// The last setup step, and a skippable one: an account that lacks a calendar or contacts can use
// another account's, and a calendar without mail can name the account that sends its invitations
// (docs/onboarding.md, "After the connect"). The pickers are the core's (`setup_link_pickers`), the
// ones the account's page offers that nothing fills yet. WinUI-free so the rules are reachable from
// Mailcal.Tests.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The step for one account: its pickers, and what the person picked in each.</summary>
internal sealed class SetupLinkStep
{
    private SetupLinkStep(string account, IReadOnlyList<AccountLinkPicker> pickers, BesideStep? beside)
    {
        Account = account;
        Pickers = pickers;
        Beside = beside;
        // A suggestion starts picked, and the person confirms it by continuing.
        Picked = pickers.Select(picker => picker.Suggested is { } at ? (int?)at : null).ToArray();
    }

    /// <summary>The account the step links.</summary>
    public string Account { get; }

    public IReadOnlyList<AccountLinkPicker> Pickers { get; }

    /// <summary>The servers found beside the account's JMAP server, offered as an account of their
    /// own for what its session lacks; null when there are none, or they fill nothing.</summary>
    public BesideStep? Beside { get; }

    /// <summary>Per picker, the index into its options that is picked; null for none.</summary>
    public int?[] Picked { get; private set; }

    /// <summary>Whether the person has changed a pick, after which a late suggestion leaves the
    /// step alone.</summary>
    public bool Touched { get; private set; }

    /// <summary>
    /// The step for <paramref name="account"/>, or null when no other account can fill anything
    /// it lacks and its server offers its calendar and contacts, or its kind is not offered linking
    /// at setup (<paramref name="offersLinks"/>, the core's <c>offers_setup_links</c>: a Microsoft
    /// or Google account holds everything itself, and linking one is for Settings). A server
    /// without one gets the step anyway, for "Add another account" and for
    /// <paramref name="beside"/>, which is offered only then. <paramref name="pickers"/> is the
    /// core's <c>setup_link_pickers</c>.
    /// </summary>
    public static SetupLinkStep? For(
        AccountsSnapshot snapshot, string account, Func<AccountKind, bool> offersLinks,
        Func<AccountEntry, IReadOnlyList<AccountLinkPicker>> pickers, BesideOffer? beside = null)
    {
        var entry = snapshot.Accounts.FirstOrDefault(candidate => candidate.Id == account);
        if (entry is null || !offersLinks(entry.Kind))
        {
            return null;
        }
        var offered = pickers(entry);
        bool Lacks(AccountCapability capability) =>
            entry.Uses.Any(use => use.Capability == capability && use.State == CapabilityState.NotOffered);
        var (calendar, contacts) = (Lacks(AccountCapability.Calendar), Lacks(AccountCapability.Contacts));
        var found = beside is not null && BesideStep.Of(beside.Found, calendar, contacts) is { } fills
            ? new BesideStep(beside, fills)
            : null;
        return offered.Count == 0 && !calendar && !contacts ? null : new SetupLinkStep(account, offered, found);
    }

    /// <summary>
    /// The same step read again, for a suggestion that arrived after it was drawn. A step the
    /// person has already changed keeps what they picked, carried across by account, slot by
    /// slot; the fresh pickers stand, since they may offer an account added since.
    /// </summary>
    public SetupLinkStep Refreshed(SetupLinkStep fresh)
    {
        if (!Touched)
        {
            return fresh;
        }
        for (var i = 0; i < fresh.Pickers.Count; i++)
        {
            var slot = fresh.Pickers[i].Slot;
            var earlier = Pickers.Select((picker, index) => (picker, index)).FirstOrDefault(pair => pair.picker.Slot == slot);
            if (earlier.picker is null)
            {
                continue;
            }
            var chose = Picked[earlier.index] is { } at ? earlier.picker.Options[at].Id : null;
            var position = chose is null ? -1 : fresh.Pickers[i].Options.ToList().FindIndex(option => option.Id == chose);
            fresh.Picked[i] = position < 0 ? null : position;
        }
        fresh.Touched = true;
        return fresh;
    }

    /// <summary>Records a pick.</summary>
    public void Pick(int picker, int? option)
    {
        if (picker >= 0 && picker < Picked.Length)
        {
            Picked[picker] = option;
            Touched = true;
        }
    }

    /// <summary>The links to set: each slot picked, and the account it names.</summary>
    public IReadOnlyList<(LinkSlot Slot, string Target)> Links() =>
        Pickers.Select((picker, index) => (picker, picked: Picked[index]))
            .Where(pair => pair.picked is not null)
            .Select(pair => (pair.picker.Slot, pair.picker.Options[pair.picked!.Value].Id))
            .ToList();
}
