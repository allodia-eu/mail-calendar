// What follows a connect on the setup form (docs/onboarding.md, "After the connect"): the link step,
// the accounts an "Add another account" adds on the way, and which of them are asked their name
// once the flow ends. WinUI-free so the flow is reachable from Mailcal.Tests; MailboxModel holds one
// and the setup form draws what it says.

namespace Allodia.Mailcal.Services;

internal sealed class SetupFlow
{
    private readonly List<string> _added = [];

    /// <summary>The link step on screen; null while the form is on the address or a route.</summary>
    public SetupLinkStep? Links { get; private set; }

    /// <summary>The account whose link step "Add another account" left, to come back to.</summary>
    public string? ReturningTo { get; private set; }

    /// <summary>The accounts this flow has added so far, in the order added.</summary>
    public IReadOnlyList<string> Added => _added;

    /// <summary>
    /// An account was added: back to the link step it was added from, on to its own link step when
    /// another account can fill something it lacks, or the end of the flow.
    /// </summary>
    /// <returns>Whether a link step is to be shown; otherwise the flow ends.</returns>
    public bool AccountAdded(string account, Func<string, SetupLinkStep?> stepFor)
    {
        _added.Add(account);
        var target = ReturningTo ?? Links?.Account ?? account;
        ReturningTo = null;
        Links = stepFor(target);
        return Links is not null;
    }

    /// <summary>"Add another account": setup runs again from the address, and comes back here.</summary>
    public void AddLinked()
    {
        ReturningTo = Links?.Account;
        Links = null;
    }

    /// <summary>
    /// Cancel, or the form dismissed: back to the link step an "Add another account" left.
    /// </summary>
    /// <returns>Whether a link step is to be shown; otherwise the flow ends, and a flow that added
    /// an account ends as a skipped link step does, so each account added is still asked its
    /// name.</returns>
    public bool Cancelled(Func<string, SetupLinkStep?> stepFor)
    {
        if (ReturningTo is not { } account)
        {
            return false;
        }
        ReturningTo = null;
        Links = stepFor(account);
        return Links is not null;
    }

    /// <summary>A suggestion the core found after the step was drawn reaches it, unless the
    /// person has changed the step already.</summary>
    /// <returns>Whether what the step shows changed.</returns>
    public bool Refresh(SetupLinkStep? fresh)
    {
        if (Links is not { } step || fresh is null || fresh.Account != step.Account)
        {
            return false;
        }
        var before = Shape(step);
        Links = step.Refreshed(fresh);
        return Shape(Links) != before;
    }

    /// <summary>The end of the flow: the accounts to ask a name for, in the order added.</summary>
    public IReadOnlyList<string> Finish()
    {
        var added = _added.ToList();
        _added.Clear();
        Links = null;
        ReturningTo = null;
        return added;
    }

    private static string Shape(SetupLinkStep step) => string.Join(
        ';',
        step.Pickers.Select((picker, index) =>
            $"{picker.Slot}:{string.Join(',', picker.Options.Select(option => option.Id))}:{step.Picked[index]}"));
}
