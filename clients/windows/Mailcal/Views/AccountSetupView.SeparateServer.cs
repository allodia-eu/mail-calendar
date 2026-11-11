// The setup form's answers to a calendar and contacts the account's sign-in does not reach
// (docs/accounts.md rule 18, docs/onboarding.md "After the connect"): the step after the connect
// that gives them a sign-in of their own, drawn from the core's section, and the question the
// refusal's "Update the existing account" asks first. Both are drawn in the form, because adding another account puts the form in
// a ContentDialog and WinUI allows one at a time. What they decide is UnreachedUsesRules' and
// SetupFlow's, which the unit suite drives.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    private ThemeBrushes? _brushes;

    // The step drawn and its form, so a redraw of the same step updates it in place and what is
    // typed stands.
    private SeparateServerStep? _separateDrawn;
    private SeparateServerForm? _separateForm;

    // Whether the separate-server step was on screen, which its end leaves for the address.
    private bool _separateShown;

    // The update question waiting, answered as Cancel when the form moves on without it.
    private UnreachedPrompt? _updateAsking;

    private void DrawSeparateServer(SeparateServerStep step)
    {
        _separateShown = true;
        if (ReferenceEquals(_separateDrawn, step))
        {
            return;
        }
        var same = _separateDrawn?.AccountId == step.AccountId && _separateForm is not null;
        if (!same)
        {
            if (_separateDrawn is { } left)
            {
                CancelSeparateSignIn(left.AccountId);
            }
            SeparatePanel.Children.Clear();
            var form = UnreachedUsesViews.Form(step);
            _separateForm = form;
            // A step of the form, titled as the link step is.
            form.Title.Style = (Style)Application.Current.Resources["TitleTextBlockStyle"];
            AutomationProperties.SetHeadingLevel(form.Title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
            SeparatePanel.Children.Add(form.Panel);
            form.Password.Loaded += (_, _) => form.Password.Focus(FocusState.Programmatic);
        }
        _separateDrawn = step;
        ShowSeparate();
    }

    // Draws the step as the core last drew it, its buttons and its sign-in from the core's
    // controls.
    private void ShowSeparate()
    {
        if (_separateForm is not { } form || _separateDrawn is not { } step || Model is not { } model)
        {
            return;
        }
        form.Show(step, step.Section.StopUsing, L10n.SeparateServerSkip(), () =>
        {
            if (form.Password.Password.Length > 0)
            {
                model.ConnectSeparateServer(step.AccountId, form.Username.Text.Trim(), form.Password.Password);
            }
        }, () => model.SkipSeparateServer(step.AccountId),
        () => model.SignInSeparately(step.AccountId), model.CancelSignIn);
        form.Say(step.Section.Failed is { } failed ? AccountUseText.Refused(failed) : null);
    }

    /// <summary>Cancels the sign-in the browser holds for <paramref name="accountId"/>'s separate
    /// server, if one does: the step it was started from is being left.</summary>
    private void CancelSeparateSignIn(string accountId)
    {
        if (Model is { } model && SeparateServerSignIn.ToCancel(model.SeparateServerStepOf(accountId)?.Section) is { } browser)
        {
            model.CancelSignIn(browser);
        }
    }

    /// <summary>Asks, over the form, what becomes of <paramref name="unreached"/> before the
    /// update.</summary>
    private Task<UnreachedAnswer> AskUpdateQuestionAsync(IReadOnlyList<AccountCapability> unreached)
    {
        ClearUpdateQuestion();
        _brushes ??= new ThemeBrushes(this);
        _updateAsking = UnreachedUsesViews.Ask(this, unreached, _brushes.Of);
        return _updateAsking.Answer;
    }

    /// <summary>Takes the update question away, answered as Cancel if it was still open.</summary>
    private void ClearUpdateQuestion()
    {
        _updateAsking?.Dismiss();
        _updateAsking = null;
    }
}
