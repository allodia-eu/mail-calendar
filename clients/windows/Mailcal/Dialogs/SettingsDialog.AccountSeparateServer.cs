// An account's page, a calendar or address book whose server needs a sign-in of its own or refused
// the account's (docs/accounts.md rules 16 and 18): an action under the use the core names opens
// the form that moves them to an account of their own, linked to this one, with a password or,
// where the server's own provider signs it in (docs/mail-oauth.md rule 13), through the browser.
// Inline, because Settings is itself a ContentDialog. The form is the core's section; what is
// typed is kept on the dialog, so a redraw while it connects draws it again as it was.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The form open on an account's page: whose, the login typed, and why its last
    /// sign-in in the browser failed.</summary>
    private sealed record SeparateServerOpen(string AccountId, string Username, string? Error);

    private SeparateServerOpen? _separateOpen;

    // The password typed, which a redraw puts back; never in the record, whose text a log could
    // print.
    private string _separatePassword = string.Empty;

    // The form as drawn last, which a sign-in that ends after a redraw updates.
    private SeparateServerForm? _separateDrawn;

    // The form's sign-in at its server's own provider: the attempt out, which a redraw keeps.
    private readonly SeparateServerSignIn _separateSignIn = new();

    private bool _separateClosedWired;

    /// <summary>The action and its form under <paramref name="use"/>'s row, when the core offers
    /// it there; null otherwise. A form open for an account the core no longer draws one for is
    /// over.</summary>
    private StackPanel? SeparateServerAction(AccountEntry entry, AccountUseSwitch use, AccountPage page, ControlState control)
    {
        var step = UnreachedUsesRules.Step(entry.Id, page.SeparateServer);
        if (step is null && _separateOpen?.AccountId == entry.Id)
        {
            RetireSeparateSignIn();
            _separateOpen = null;
            _separateDrawn = null;
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
                RetireSeparateSignIn();
                _separateOpen = new SeparateServerOpen(entry.Id, step.Username, null);
                _separatePassword = string.Empty;
                NotifyAccounts(null);
            }));
        return panel;
    }

    private StackPanel SeparateServerFormFor(SeparateServerStep step, SeparateServerOpen open)
    {
        var form = UnreachedUsesViews.Form(step, open.Username);
        form.Password.Password = _separatePassword;
        _separateDrawn = form;
        form.Username.TextChanged += (_, _) =>
        {
            if (_separateOpen is { } kept)
            {
                _separateOpen = kept with { Username = form.Username.Text };
            }
        };
        form.Password.PasswordChanged += (_, _) => _separatePassword = form.Password.Password;
        ShowSeparate(form, step);
        form.SignIn.Button.Click += (_, _) => _ = SeparateSignInAsync(step, form);
        if (!_separateClosedWired)
        {
            _separateClosedWired = true;
            Closed += (_, _) => RetireSeparateSignIn();
        }
        return form.Panel;
    }

    // Draws the form as the core drew it and the sign-in stands. Connect sends what was typed; the
    // form's Cancel is the sign-in's way back too.
    private void ShowSeparate(SeparateServerForm form, SeparateServerStep step)
    {
        form.Show(step, _separateSignIn, cancelOwn: false, L10n.ActionCancel(), () =>
        {
            if (form.Password.Password.Length == 0)
            {
                return;
            }
            RetireSeparateSignIn();
            _separateOpen = _separateOpen is { } kept ? kept with { Error = null } : null;
            _model.ConnectSeparateServer(step.AccountId, form.Username.Text.Trim(), form.Password.Password);
        }, () =>
        {
            RetireSeparateSignIn();
            _separateOpen = null;
            _separatePassword = string.Empty;
            NotifyAccounts(null);
        });
        form.Say(_separateOpen?.Error ?? (step.Section.Failed is { } failed ? AccountUseText.Refused(failed) : null));
    }

    // The browser sign-in at the server's own provider. Its progress and its end land on the form
    // drawn last, because the core redraws the page while it runs.
    private async Task SeparateSignInAsync(SeparateServerStep step, SeparateServerForm form)
    {
        if (_separateOpen is { } before)
        {
            _separateOpen = before with { Username = form.Username.Text.Trim(), Error = null };
        }
        ShowSeparate(form, step);
        var next = await _model.SeparateServerSignInAsync(
            step, _separateSignIn, _ => Redrawn(step));
        if (next is null || _separateOpen is not { } open || open.AccountId != step.AccountId)
        {
            return;
        }
        if (next.GoesOn)
        {
            // The page redraws without the form, the core drawing none, and shows the link.
            _separateOpen = null;
            _separateDrawn = null;
            _separatePassword = string.Empty;
            NotifyAccounts(null);
            return;
        }
        _separateOpen = open with { Error = next.Error };
        Redrawn(step);
    }

    // The form drawn last, drawn again with the section it was drawn from.
    private void Redrawn(SeparateServerStep step)
    {
        if (_separateDrawn is { } drawn)
        {
            ShowSeparate(drawn, _model.SeparateServerStepOf(step.AccountId) ?? step);
        }
    }

    // The way back from the form's sign-in: the attempt out opens no browser and adds no account,
    // and a Nextcloud grant the form held for another try is withdrawn.
    private void RetireSeparateSignIn()
    {
        if (_separateSignIn.Retire())
        {
            _model.CancelSeparateServerSignIn();
        }
        _model.ReleaseSeparateNextcloudGrant(_separateSignIn);
    }
}
