// What follows a connect on the setup form (docs/onboarding.md, "After the connect"): a calendar on
// a server the sign-in does not reach, then the link step, the accounts an "Add another account" adds on the way, and which of them are asked their name
// once the flow ends. WinUI-free so the flow is reachable from Mailcal.Tests; MailboxModel holds one
// and the setup form draws what it says.

namespace Allodia.Mailcal.Services;

internal sealed class SetupFlow
{
    private readonly List<string> _added = [];

    // The account the link step is for, kept while the separate-server step comes first.
    private string? _linkTarget;

    /// <summary>The step that gives a calendar or contacts the sign-in did not reach a sign-in of
    /// their own; shown before the link step, and null otherwise.</summary>
    public SeparateServerStep? SeparateServer { get; private set; }

    /// <summary>The link step on screen; null while the form is on the address or a route.</summary>
    public SetupLinkStep? Links { get; private set; }

    /// <summary>The account whose link step "Add another account" left, to come back to.</summary>
    public string? ReturningTo { get; private set; }

    /// <summary>The accounts this flow has added so far, in the order added.</summary>
    public IReadOnlyList<string> Added => _added;

    /// <summary>
    /// An account was added: on to <paramref name="separate"/> when its sign-in did not reach its
    /// calendar or contacts, then back to the link step it was added from, on to its own link step
    /// when another account can fill something it lacks, or the end of the flow.
    /// </summary>
    /// <returns>Whether a step is to be shown; otherwise the flow ends.</returns>
    public bool AccountAdded(string account, Func<string, SetupLinkStep?> stepFor, SeparateServerStep? separate = null)
    {
        _added.Add(account);
        _linkTarget = ReturningTo ?? Links?.Account ?? account;
        ReturningTo = null;
        if (separate is not null)
        {
            SeparateServer = separate;
            Links = null;
            return true;
        }
        return ToLinks(stepFor);
    }

    /// <summary>The separate-server step answered, connected or skipped: on to the link step.</summary>
    /// <returns>Whether a link step is to be shown; otherwise the flow ends.</returns>
    public bool SeparateServerDone(Func<string, SetupLinkStep?> stepFor)
    {
        if (SeparateServer is null)
        {
            return false;
        }
        SeparateServer = null;
        return ToLinks(stepFor);
    }

    private bool ToLinks(Func<string, SetupLinkStep?> stepFor)
    {
        var target = _linkTarget;
        _linkTarget = null;
        Links = target is null ? null : stepFor(target);
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
        SeparateServer = null;
        _linkTarget = null;
        Links = null;
        ReturningTo = null;
        return added;
    }

    private static string Shape(SetupLinkStep step) => string.Join(
        ';',
        step.Pickers.Select((picker, index) =>
            $"{picker.Slot}:{string.Join(',', picker.Options.Select(option => option.Id))}:{step.Picked[index]}"));
}
