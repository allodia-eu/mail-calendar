// The setup form's answers to a calendar and contacts the account's sign-in does not reach
// (docs/accounts.md rule 18, docs/onboarding.md "After the connect"): the step after the connect
// that gives them a sign-in of their own, and the question the refusal's "Update the existing
// account" asks first. Both are drawn in the form, because adding another account puts the form in
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

    // The step drawn, so a flow signal that leaves it on screen does not wipe what is typed.
    private SeparateServerStep? _separateDrawn;

    // Whether the separate-server step was on screen, which its end leaves for the address.
    private bool _separateShown;

    // The update question waiting, answered as Cancel when the form moves on without it.
    private UnreachedPrompt? _updateAsking;

    // The step's sign-in at its server's own provider: what was offered, and the attempt out.
    private readonly SeparateServerSignIn _separateSignIn = new();

    private void DrawSeparateServer(SeparateServerStep step)
    {
        _separateShown = true;
        if (ReferenceEquals(_separateDrawn, step))
        {
            return;
        }
        _separateDrawn = step;
        RetireSeparateSignIn();
        SeparatePanel.Children.Clear();
        var form = UnreachedUsesViews.Form(step, L10n.SeparateServerSkip());
        // A step of the form, titled as the link step is.
        form.Title.Style = (Style)Application.Current.Resources["TitleTextBlockStyle"];
        AutomationProperties.SetHeadingLevel(form.Title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
        form.Connect.IsEnabled = false;
        form.Password.PasswordChanged += (_, _) =>
            form.Connect.IsEnabled = form.Password.Password.Length > 0 && _separateSignIn.FormEnabled;
        form.Connect.Click += async (_, _) =>
        {
            if (Model is not { } model)
            {
                return;
            }
            RetireSeparateSignIn();
            form.Say(null);
            form.Busy(true);
            var error = await model.SetupSeparateServerConnectAsync(form.Username.Text.Trim(), form.Password.Password);
            if (error is not null)
            {
                form.Busy(false);
                form.Connect.IsEnabled = form.Password.Password.Length > 0;
                form.Draw(_separateSignIn, cancelOwn: true);
                form.Say(error);
            }
        };
        form.Secondary.Click += async (_, _) =>
        {
            if (Model is { } model)
            {
                RetireSeparateSignIn();
                form.Busy(true);
                await model.SetupSeparateServerSkipAsync();
            }
        };
        form.SignIn.Button.Click += async (_, _) =>
        {
            if (Model is not { } model)
            {
                return;
            }
            form.Say(null);
            var next = await model.SetupSeparateServerSignInAsync(
                _separateSignIn, _ => form.Draw(_separateSignIn, cancelOwn: true));
            if (next is { GoesOn: false, Error: { } error } && ReferenceEquals(_separateDrawn, step))
            {
                form.Say(error);
            }
        };
        form.SignIn.Cancel.Click += (_, _) =>
        {
            RetireSeparateSignIn();
            form.Draw(_separateSignIn, cancelOwn: true);
        };
        SeparatePanel.Children.Add(form.Panel);
        form.Password.Loaded += (_, _) => form.Password.Focus(FocusState.Programmatic);
        _ = OfferSeparateSignInAsync(step, form);
    }

    // Asks once, off the UI thread, whether the step's server signs in at its own provider. The
    // row is added above the login without taking focus, so the answer moves nothing being typed.
    private async Task OfferSeparateSignInAsync(SeparateServerStep step, SeparateServerForm form)
    {
        if (Model is not { } model || !_separateSignIn.Asks(step))
        {
            form.Draw(_separateSignIn, cancelOwn: true);
            return;
        }
        _separateSignIn.Answered(step, await model.SeparateServerOffersSignInAsync(step));
        if (ReferenceEquals(_separateDrawn, step))
        {
            form.Draw(_separateSignIn, cancelOwn: true);
        }
    }

    // The way back from the step's sign-in: the attempt out opens no browser and adds no account.
    private void RetireSeparateSignIn()
    {
        if (_separateSignIn.Retire())
        {
            Model?.CancelSeparateServerSignIn();
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
