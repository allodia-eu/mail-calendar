// The last setup step, and a skippable one: an account that lacks a calendar or contacts can use
// another account's, and a calendar without mail can name the account that sends its invitations
// (docs/onboarding.md, "After the connect"). The pickers are Settings' own, from the same snapshot,
// so the step offers exactly what the account's page would. WinUI-free so the rules are reachable
// from Mailcal.Tests.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The step for one account: its pickers, and what the person picked in each.</summary>
internal sealed class SetupLinkStep
{
    private SetupLinkStep(string account, IReadOnlyList<LinkPicker> pickers)
    {
        Account = account;
        Pickers = pickers;
        // A suggestion starts picked, and the person confirms it by continuing.
        Picked = pickers.Select(picker => picker.Suggested).ToArray();
    }

    /// <summary>The account the step links.</summary>
    public string Account { get; }

    public IReadOnlyList<LinkPicker> Pickers { get; }

    /// <summary>Per picker, the index into its options that is picked; null for none.</summary>
    public int?[] Picked { get; private set; }

    /// <summary>Whether the person has changed a pick, after which a late suggestion leaves the
    /// step alone.</summary>
    public bool Touched { get; private set; }

    /// <summary>
    /// The step for <paramref name="account"/>, or null when no other account can fill anything
    /// it lacks, or its kind is not offered linking at setup (<paramref name="offersLinks"/>, the
    /// core's <c>offers_setup_links</c>: a Microsoft, Google or JMAP account holds everything
    /// itself, and linking one is for Settings).
    /// </summary>
    public static SetupLinkStep? For(AccountsSnapshot snapshot, string account, Func<AccountKind, bool> offersLinks)
    {
        var entry = snapshot.Accounts.FirstOrDefault(candidate => candidate.Id == account);
        if (entry is null || !offersLinks(entry.Kind))
        {
            return null;
        }
        var pickers = AccountSettingsRules.LinkPickers(entry).Where(picker => picker.Selected is null).ToList();
        return pickers.Count == 0 ? null : new SetupLinkStep(account, pickers);
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
