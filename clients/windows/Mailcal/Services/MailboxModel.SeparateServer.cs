// A calendar and contacts the account's sign-in does not reach, given a sign-in of their own
// (docs/accounts.md rule 18): the setup step after the mail sign-in, and the form on the account's
// page. Both draw the core's section; Connect and Skip are changes on the account's page, which the
// core runs and draws there. A form the core draws no section for is over.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>The form for <paramref name="accountId"/>'s separate server, from the core's page;
    /// null when nothing needs one.</summary>
    internal SeparateServerStep? SeparateServerStepOf(string accountId) =>
        UnreachedUsesRules.Step(accountId, _app?.AccountPage(accountId)?.SeparateServer);

    /// <summary>Connect: <paramref name="password"/>, tried by the core as a change on the
    /// account's page.</summary>
    internal void ConnectSeparateServer(string accountId, string username, string password) =>
        ChangeAccount(new AccountPageIntent.ConnectSeparateServer(accountId, username, new TypedPassword(password)));

    /// <summary>Skip on the setup's step: the new account stops being used for what its sign-in
    /// does not reach, so it does not open on a calendar that cannot connect.</summary>
    internal void SkipSeparateServer(string accountId) =>
        ChangeAccount(new AccountPageIntent.StopUsingSeparateServer(accountId));

    /// <summary>A form is over: the uses have an account of their own, or were switched off, so the
    /// person's other devices hear of it.</summary>
    internal void SeparateServerOver() => SyncAfterAccountChange();

    /// <summary>Draws the setup step again from the core, after a signal that may have moved it,
    /// and goes on to the link step once the core draws none.</summary>
    private void RefreshSetupSeparateServer()
    {
        if (_setupFlow.SeparateServer is not { } shown)
        {
            return;
        }
        if (SeparateServerStepOf(shown.AccountId) is not { } fresh)
        {
            SeparateServerOver();
            SeparateServerDone();
            return;
        }
        if (UnreachedUsesRules.Fingerprint(fresh.Section) != UnreachedUsesRules.Fingerprint(shown.Section))
        {
            _setupFlow.Redraw(fresh);
            RaiseSetupFlow();
        }
    }
}
