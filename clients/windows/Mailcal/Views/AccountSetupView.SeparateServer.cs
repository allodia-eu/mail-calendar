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
    private TaskCompletionSource<UnreachedAnswer>? _updateAsking;

    private void DrawSeparateServer(SeparateServerStep step)
    {
        _separateShown = true;
        if (ReferenceEquals(_separateDrawn, step))
        {
            return;
        }
        _separateDrawn = step;
        SeparatePanel.Children.Clear();
        var form = UnreachedUsesViews.Form(step, L10n.SeparateServerSkip());
        // A step of the form, titled as the link step is.
        form.Title.Style = (Style)Application.Current.Resources["TitleTextBlockStyle"];
        AutomationProperties.SetHeadingLevel(form.Title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
        form.Connect.IsEnabled = false;
        form.Password.PasswordChanged += (_, _) => form.Connect.IsEnabled = form.Password.Password.Length > 0;
        form.Connect.Click += async (_, _) =>
        {
            if (Model is not { } model)
            {
                return;
            }
            form.Say(null);
            form.Busy(true);
            var error = await model.SetupSeparateServerConnectAsync(form.Username.Text.Trim(), form.Password.Password);
            if (error is not null)
            {
                form.Busy(false);
                form.Connect.IsEnabled = form.Password.Password.Length > 0;
                form.Say(error);
            }
        };
        form.Secondary.Click += async (_, _) =>
        {
            if (Model is { } model)
            {
                form.Busy(true);
                await model.SetupSeparateServerSkipAsync();
            }
        };
        SeparatePanel.Children.Add(form.Panel);
        form.Password.Loaded += (_, _) => form.Password.Focus(FocusState.Programmatic);
    }

    /// <summary>Asks, under the update button, what becomes of <paramref name="unreached"/>.</summary>
    private Task<UnreachedAnswer> AskUpdateQuestionAsync(IReadOnlyList<AccountCapability> unreached)
    {
        ClearUpdateQuestion();
        var asking = new TaskCompletionSource<UnreachedAnswer>(TaskCreationOptions.RunContinuationsAsynchronously);
        _updateAsking = asking;
        _brushes ??= new ThemeBrushes(this);
        UpdateExistingButton.Visibility = Visibility.Collapsed;
        UpdateQuestionHost.Children.Add(UnreachedUsesViews.Question(unreached, _brushes.Of, answer =>
        {
            ClearUpdateQuestion();
            asking.TrySetResult(answer);
        }));
        return asking.Task;
    }

    /// <summary>Takes the update question away, answered as Cancel if it was still open.</summary>
    private void ClearUpdateQuestion()
    {
        _updateAsking?.TrySetResult(UnreachedAnswer.Cancel);
        _updateAsking = null;
        UpdateQuestionHost.Children.Clear();
        UpdateExistingButton.Visibility = Visibility.Visible;
    }
}
