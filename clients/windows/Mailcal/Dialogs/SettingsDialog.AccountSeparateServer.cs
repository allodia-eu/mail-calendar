// An account's page, a calendar or address book whose server needs a sign-in of its own or refused
// the account's (docs/accounts.md rules 16 and 18): an action under the use the core names opens
// the form that moves them to an account of their own, linked to this one, with a password or,
// where the server's own provider signs it in (docs/mail-oauth.md rule 13), through the browser.
// Inline, because Settings is itself a ContentDialog. The form is the core's section, the browser
// sign-in included; what is typed is kept on the dialog, so a redraw while it connects draws it
// again as it was.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The form open on an account's page: whose, and the login typed.</summary>
    private sealed record SeparateServerOpen(string AccountId, string Username);

    private SeparateServerOpen? _separateOpen;

    // The password typed, which a redraw puts back; never in the record, whose text a log could
    // print.
    private string _separatePassword = string.Empty;

    private bool _separateClosedWired;

    /// <summary>The action and its form under <paramref name="use"/>'s row, when the core offers
    /// it there; null otherwise. A form open for an account the core no longer draws one for is
    /// over.</summary>
    private StackPanel? SeparateServerAction(AccountEntry entry, AccountUseSwitch use, AccountPage page, ControlState control)
    {
        var step = UnreachedUsesRules.Step(entry.Id, page.SeparateServer);
        if (step is null && _separateOpen?.AccountId == entry.Id)
        {
            _separateOpen = null;
            _separatePassword = string.Empty;
            _model.SeparateServerOver();
        }
        if (step is null || !use.SignsInSeparately)
        {
            return null;
        }
        var panel = new StackPanel { Spacing = 8 };
        if (_separateOpen is { } open && open.AccountId == entry.Id)
        {
            panel.Children.Add(SeparateServerFormFor(step, open));
            return panel;
        }
        panel.Children.Add(ControlComponents.Button(
            L10n.SettingsAccountUseSeparateSignIn(), control, $"AccountUseSeparateSignIn_{use.Capability}", () =>
            {
                _separateOpen = new SeparateServerOpen(entry.Id, step.Username);
                _separatePassword = string.Empty;
                NotifyAccounts(null);
            }));
        return panel;
    }

    // Drawn as the core drew the section. Connect sends what was typed; the form's Cancel closes
    // it, and a sign-in the browser holds goes with it, as it does when Settings closes.
    private StackPanel SeparateServerFormFor(SeparateServerStep step, SeparateServerOpen open)
    {
        var form = UnreachedUsesViews.Form(step, open.Username);
        form.Password.Password = _separatePassword;
        form.Username.TextChanged += (_, _) =>
        {
            if (_separateOpen is { } kept)
            {
                _separateOpen = kept with { Username = form.Username.Text };
            }
        };
        form.Password.PasswordChanged += (_, _) => _separatePassword = form.Password.Password;
        form.Show(step, SeparateServerSignIn.PageCancel(step.Section), L10n.ActionCancel(), () =>
        {
            if (form.Password.Password.Length > 0)
            {
                _model.ConnectSeparateServer(step.AccountId, form.Username.Text.Trim(), form.Password.Password);
            }
        }, () =>
        {
            CancelSeparateSignIn();
            _separateOpen = null;
            _separatePassword = string.Empty;
            NotifyAccounts(null);
        }, () => _model.SignInSeparately(step.AccountId), _model.CancelSignIn);
        form.Say(step.Section.Failed is { } failed ? AccountUseText.Refused(failed) : null);
        if (!_separateClosedWired)
        {
            _separateClosedWired = true;
            Closed += (_, _) => CancelSeparateSignIn();
        }
        return form.Panel;
    }

    // Cancels the sign-in the browser holds for the open form's server, if one does.
    private void CancelSeparateSignIn()
    {
        if (_separateOpen is { } open
            && SeparateServerSignIn.ToCancel(_model.SeparateServerStepOf(open.AccountId)?.Section) is { } browser)
        {
            _model.CancelSignIn(browser);
        }
    }
}
