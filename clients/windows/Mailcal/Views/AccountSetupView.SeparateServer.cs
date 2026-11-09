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

    // Why the step's last sign-in in the browser failed; a Connect's or Skip's failure is the
    // core's.
    private string? _separateError;

    // Whether the separate-server step was on screen, which its end leaves for the address.
    private bool _separateShown;

    // The update question waiting, answered as Cancel when the form moves on without it.
    private UnreachedPrompt? _updateAsking;

    // The step's sign-in at its server's own provider: the attempt out.
    private readonly SeparateServerSignIn _separateSignIn = new();

    private void DrawSeparateServer(SeparateServerStep step)
    {
        _separateShown = true;
        if (ReferenceEquals(_separateDrawn, step))
        {
            return;
        }
        var same = _separateDrawn?.AccountId == step.AccountId && _separateForm is not null;
        _separateDrawn = step;
        if (!same)
        {
            RetireSeparateSignIn();
            _separateError = null;
            SeparatePanel.Children.Clear();
            var form = UnreachedUsesViews.Form(step);
            _separateForm = form;
            // A step of the form, titled as the link step is.
            form.Title.Style = (Style)Application.Current.Resources["TitleTextBlockStyle"];
            AutomationProperties.SetHeadingLevel(form.Title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
            form.SignIn.Button.Click += async (_, _) =>
            {
                if (Model is not { } model)
                {
                    return;
                }
                _separateError = null;
                ShowSeparate();
                var next = await model.SetupSeparateServerSignInAsync(_separateSignIn, _ => ShowSeparate());
                if (next is { GoesOn: false, Error: { } error } && ReferenceEquals(_separateForm, form))
                {
                    _separateError = error;
                    ShowSeparate();
                }
            };
            form.SignIn.Cancel.Click += (_, _) =>
            {
                RetireSeparateSignIn();
                ShowSeparate();
            };
            SeparatePanel.Children.Add(form.Panel);
            form.Password.Loaded += (_, _) => form.Password.Focus(FocusState.Programmatic);
        }
        ShowSeparate();
    }

    // Draws the step as the core last drew it, its buttons from the core's controls.
    private void ShowSeparate()
    {
        if (_separateForm is not { } form || _separateDrawn is not { } step)
        {
            return;
        }
        form.Show(step, _separateSignIn, cancelOwn: true, L10n.SeparateServerSkip(), () =>
        {
            if (Model is { } model && form.Password.Password.Length > 0)
            {
                RetireSeparateSignIn();
                _separateError = null;
                model.ConnectSeparateServer(step.AccountId, form.Username.Text.Trim(), form.Password.Password);
            }
        }, () =>
        {
            if (Model is { } model)
            {
                RetireSeparateSignIn();
                _separateError = null;
                model.SkipSeparateServer(step.AccountId);
            }
        });
        form.Say(_separateError ?? (step.Section.Failed is { } failed ? AccountUseText.Refused(failed) : null));
    }

    // The way back from the step's sign-in: the attempt out opens no browser and adds no account,
    // and a Nextcloud grant the step held for another try is withdrawn.
    private void RetireSeparateSignIn()
    {
        if (_separateSignIn.Retire())
        {
            Model?.CancelSeparateServerSignIn();
        }
        Model?.ReleaseSeparateNextcloudGrant(_separateSignIn);
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
