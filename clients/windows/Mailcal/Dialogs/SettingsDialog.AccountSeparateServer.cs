// An account's page, a calendar or address book whose server needs a sign-in of its own or refused
// the account's (docs/accounts.md rules 16 and 18): an action under the use's note opens the form
// that moves them to an account of their own, linked to this one, with a password or, where the
// server's own provider signs it in (docs/mail-oauth.md rule 13), through the browser. Inline,
// because Settings is itself a ContentDialog. The open form is kept on the dialog, so a redraw
// while it connects (the core adds the new account before it knows the mailbox will take the
// change) draws it again with what was typed and what went wrong.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>The form open on an account's page: whose, under which use's row, and what it
    /// holds besides the password, which a redraw does not keep.</summary>
    private sealed record SeparateServerOpen(string AccountId, AccountCapability Row, string Username, string? Error, bool Busy);

    private SeparateServerOpen? _separateOpen;

    // The form as drawn last, which a connect that ends after a redraw updates.
    private SeparateServerForm? _separateDrawn;

    // The form's sign-in at its server's own provider: what each server offered on this visit,
    // and the attempt out, which a redraw keeps.
    private readonly SeparateServerSignIn _separateSignIn = new();

    private bool _separateClosedWired;

    /// <summary>The action and its form under <paramref name="capability"/>'s row, when the core
    /// names a server that use needs a sign-in of its own at; null otherwise.</summary>
    private StackPanel? SeparateServerAction(AccountEntry entry, AccountCapability capability)
    {
        var step = UnreachedUsesRules.Step(entry);
        if (step is null || !UnreachedUsesRules.Offers(step, capability))
        {
            return null;
        }
        var panel = new StackPanel { Spacing = 8 };
        var action = new Button { Content = L10n.SettingsAccountUseSeparateSignIn() };
        AutomationProperties.SetAutomationId(action, $"AccountUseSeparateSignIn_{capability}");
        panel.Children.Add(action);
        var open = _separateOpen is { } kept && kept.AccountId == entry.Id && kept.Row == capability ? kept : null;
        action.Visibility = open is null ? Visibility.Visible : Visibility.Collapsed;
        action.Click += (_, _) =>
        {
            RetireSeparateSignIn();
            _separateOpen = new SeparateServerOpen(entry.Id, capability, step.Username, null, false);
            NotifyAccounts(null);
        };
        if (open is not null)
        {
            panel.Children.Add(SeparateServerFormFor(step, open));
        }
        return panel;
    }

    private StackPanel SeparateServerFormFor(SeparateServerStep step, SeparateServerOpen open)
    {
        var form = UnreachedUsesViews.Form(step, L10n.ActionCancel(), open.Username);
        _separateDrawn = form;
        form.Say(open.Error);
        // Read from the dialog, not this drawing: a redraw while a connect runs draws it busy, and
        // the connect's end is what frees it.
        void Enable() => form.Connect.IsEnabled =
            _separateOpen?.Busy != true && _separateSignIn.FormEnabled && form.Password.Password.Length > 0;
        form.Busy(open.Busy);
        Enable();
        form.Draw(_separateSignIn, cancelOwn: false);
        form.Password.PasswordChanged += (_, _) => Enable();
        // The form's Cancel is the sign-in's way back too.
        form.Secondary.Click += (_, _) =>
        {
            RetireSeparateSignIn();
            _separateOpen = null;
            NotifyAccounts(null);
        };
        form.SignIn.Button.Click += (_, _) => _ = SeparateSignInAsync(step, form);
        if (!_separateClosedWired)
        {
            _separateClosedWired = true;
            Closed += (_, _) => RetireSeparateSignIn();
        }
        _ = OfferSeparateSignInAsync(step);
        form.Connect.Click += async (_, _) =>
        {
            RetireSeparateSignIn();
            var username = form.Username.Text.Trim();
            _separateOpen = open with { Username = username, Error = null, Busy = true };
            form.Say(null);
            form.Busy(true);
            var error = await _model.AddLinkedCalendarAccountAsync(
                step.AccountId, step.Uses, username, form.Password.Password);
            if (error is null)
            {
                // The page redraws from the snapshot, which no longer has the uses on this account
                // and shows the link to the new one.
                _separateOpen = null;
                _separateDrawn = null;
                NotifyAccounts(null);
                return;
            }
            _separateOpen = _separateOpen is { } still ? still with { Error = error, Busy = false } : null;
            if (_separateDrawn is { } drawn)
            {
                drawn.Say(error);
                drawn.Busy(false);
                drawn.Connect.IsEnabled = drawn.Password.Password.Length > 0;
            }
        };
        return form.Panel;
    }

    // Asks once per visit, off the UI thread, whether the form's server signs in at its own
    // provider, and draws the answer on the form drawn last: the row goes above the login without
    // taking focus.
    private async Task OfferSeparateSignInAsync(SeparateServerStep step)
    {
        if (_separateSignIn.Asks(step))
        {
            _separateSignIn.Answered(step, await _model.SeparateServerOffersSignInAsync(step));
        }
        _separateDrawn?.Draw(_separateSignIn, cancelOwn: false);
    }

    // The browser sign-in at the server's own provider. Its progress and its end land on the form
    // drawn last, because the core redraws the page while it runs.
    private async Task SeparateSignInAsync(SeparateServerStep step, SeparateServerForm form)
    {
        form.Say(null);
        if (_separateOpen is { } before)
        {
            _separateOpen = before with { Username = form.Username.Text.Trim(), Error = null };
        }
        var next = await _model.SeparateServerSignInAsync(
            step, _separateSignIn, _ => _separateDrawn?.Draw(_separateSignIn, cancelOwn: false));
        if (next is null || _separateOpen is not { } open || open.AccountId != step.AccountId)
        {
            return;
        }
        if (next.GoesOn)
        {
            // As Connect's success: the page redraws from the snapshot, which no longer has the
            // uses on this account and shows the link to the new one.
            _separateOpen = null;
            _separateDrawn = null;
            NotifyAccounts(null);
            return;
        }
        _separateOpen = open with { Error = next.Error };
        _separateDrawn?.Say(next.Error);
    }

    // The way back from the form's sign-in: the attempt out opens no browser and adds no account.
    private void RetireSeparateSignIn()
    {
        if (_separateSignIn.Retire())
        {
            _model.CancelSeparateServerSignIn();
        }
    }
}
