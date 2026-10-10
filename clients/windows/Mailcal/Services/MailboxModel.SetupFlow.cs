// What follows a connect, for every way an account is added: the link step (docs/onboarding.md,
// "After the connect"), the accounts an "Add another account" adds on the way, the name each of
// them is asked for once the form closes (docs/sending.md), and, last, what a provider's sign-in
// withheld, said on that account's page in Settings (docs/accounts.md rule 10).

using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>A sign-in that did not grant every use the person chose: the account, and the uses
/// waiting on the provider.</summary>
internal sealed record WithheldUses(string AccountId, IReadOnlyList<AccountCapability> Uses);

public sealed partial class MailboxModel
{
    private readonly SetupFlow _setupFlow = new();
    private readonly Queue<string> _senderNamesWaiting = new();
    private WithheldUses? _withheldWaiting;

    // Whether a name prompt is under way, from the moment it is raised until the shell says it
    // was answered. SenderNamePrompt cannot say so: the shell clears it the moment it takes the
    // request, and then asks the provider for a suggestion before the dialog is up.
    private bool _askingSenderName;

    // Whether the link step's picks are being stored. Each stored link signals the core, which
    // would otherwise redraw the step under the person with its buttons live again.
    private bool _savingLinks;

    /// <summary>The link step on the setup form; null while the form is on the address or a
    /// route.</summary>
    internal SetupLinkStep? SetupLinks => _setupFlow.Links;

    /// <summary>Raised when the setup form moves onto or off its link step.</summary>
    public event Action? SetupLinksChanged;

    private WithheldUses? _withheldNotice;

    /// <summary>A sign-in's withheld uses, raised once setup and its name prompts are done, so it
    /// is not one of several dialogs competing for the same moment. The shell opens Settings on the
    /// account with it.</summary>
    internal WithheldUses? WithheldNotice
    {
        get => _withheldNotice;
        set => Set(ref _withheldNotice, value);
    }

    /// <summary>Whether the form's Cancel has somewhere to go: an add over the running app, or a
    /// link step an "Add another account" left.</summary>
    private bool SetupCancellable => AddingAccount || _setupFlow.ReturningTo is not null;

    /// <summary>
    /// An account was added, by any route. The form moves on to the link step when another account
    /// can fill something it lacks, back to the step an "Add another account" left, or closes.
    /// </summary>
    private void AccountAdded(string accountId)
    {
        SetupError = null;
        var snapshot = AccountsSnapshot();
        if (snapshot.Accounts.FirstOrDefault(entry => entry.Id == accountId) is { } entry
            && AccountSettingsRules.Withheld(entry) is { Count: > 0 } withheld)
        {
            _withheldWaiting = new WithheldUses(accountId, withheld);
        }
        var linking = _setupFlow.AccountAdded(accountId, LinkStepFor);
        RaiseSetupFlow();
        if (!linking)
        {
            FinishSetup();
        }
    }

    private SetupLinkStep? LinkStepFor(string accountId) =>
        SetupLinkStep.For(AccountsSnapshot(), accountId, MailcalBindingsMethods.OffersSetupLinks);

    /// <summary>The link step answered: the picks are linked when <paramref name="link"/>, and the
    /// form closes either way.</summary>
    internal async Task SetupLinksDoneAsync(bool link)
    {
        if (_savingLinks)
        {
            return;
        }
        _savingLinks = true;
        try
        {
            if (link && _setupFlow.Links is { } step)
            {
                foreach (var (slot, target) in step.Links())
                {
                    if (await SetAccountLinkAsync(step.Account, slot, target) is { } error)
                    {
                        Log.Warn($"a link chosen at setup was not stored: {error}");
                    }
                }
            }
        }
        finally
        {
            _savingLinks = false;
        }
        FinishSetup();
    }

    /// <summary>"Add another account": the form starts again from the address, and comes back to
    /// this step, cancelled or done.</summary>
    internal void SetupAddLinkedAccount()
    {
        if (_savingLinks)
        {
            return;
        }
        _setupFlow.AddLinked();
        SetupError = null;
        SetupStartEmail = string.Empty;
        SetupStartOffer = null;
        ClearSetupAttempt();
        RaiseSetupFlow();
    }

    /// <summary>Cancel on the form while a flow is under way. Returns whether the flow consumed
    /// it: back to the link step "Add another account" left, or the end of a flow that added an
    /// account, which ends as a skipped link step does.</summary>
    private bool CancelSetupFlow()
    {
        if (_setupFlow.Cancelled(LinkStepFor))
        {
            RaiseSetupFlow();
            return true;
        }
        if (_setupFlow.Added.Count > 0 || _setupFlow.Links is not null)
        {
            FinishSetup();
            return true;
        }
        return false;
    }

    /// <summary>Closes the form and asks each added account's name, one after another.</summary>
    private void FinishSetup()
    {
        foreach (var account in _setupFlow.Finish())
        {
            _senderNamesWaiting.Enqueue(account);
        }
        RaiseSetupFlow();
        NeedsSetup = false;
        AddingAccount = false;
        SetupStartEmail = string.Empty;
        SetupStartOffer = null;
        if (!_askingSenderName)
        {
            AskNextSenderName();
        }
    }

    /// <summary>The name step answered or skipped: the next account waiting is asked, and once
    /// none is, what a sign-in's grant withheld is said.</summary>
    internal void SenderNameAnswered()
    {
        _askingSenderName = false;
        AskNextSenderName();
    }

    private void AskNextSenderName()
    {
        if (_senderNamesWaiting.TryDequeue(out var next))
        {
            _askingSenderName = true;
            SenderNamePrompt = next;
            return;
        }
        if (_withheldWaiting is { } withheld)
        {
            _withheldWaiting = null;
            WithheldNotice = withheld;
        }
    }

    /// <summary>A suggestion the core found after the link step was drawn reaches it, unless the
    /// person has changed the step already.</summary>
    private void RefreshSetupLinks()
    {
        if (!_savingLinks && _setupFlow.Links is { } step && _setupFlow.Refresh(LinkStepFor(step.Account)))
        {
            SetupLinksChanged?.Invoke();
        }
    }

    private void RaiseSetupFlow()
    {
        SetupLinksChanged?.Invoke();
        Raise(nameof(AddingAccountVisibility));
        Raise(nameof(CancelVisibility));
    }
}
