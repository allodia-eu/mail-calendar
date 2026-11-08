// The name an account's mail is sent under, as setup's name step asks it. An account's page asks the
// core through its own change (AccountPageIntent.SetSenderName).

using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Sets the name one account's outgoing mail is sent under; empty clears it and the
    /// account sends as a bare address.</summary>
    /// <remarks>
    /// Passed through unchanged: the core sanitises it, and a second opinion here about what a
    /// <c>From</c> header may contain would only drift from the first (<c>docs/sending.md</c>).
    /// </remarks>
    public void SetAccountSenderNameChoice(string account, string name) =>
        _app?.SetAccountSenderName(account, name);

    /// <summary>The name to fill a "your name" field in with: the one already set, else the one
    /// the provider holds. Empty when neither exists, which means <em>ask</em>.</summary>
    /// <remarks>
    /// Talks to the provider, so it is awaited off the UI thread exactly as the account add is.
    /// An unreachable provider answers empty rather than failing.
    /// </remarks>
    public Task<string> SuggestedSenderNameAsync(string account) =>
        _app is { } app
            ? Task.Run(() => app.SuggestedSenderName(account))
            : Task.FromResult(string.Empty);

    /// <summary>Whether the "your name" step still has to be asked for an account that has just
    /// connected.</summary>
    /// <remarks>
    /// <c>false</c> where the provider already holds a name: the core adopts it, so the step
    /// would arrive with a pre-filled answer and nothing to decide (<c>docs/sending.md</c>).
    /// Talks to the provider, so it is awaited off the UI thread exactly as the account add is.
    /// With no core to ask, the step is shown: skipping it leaves the account sending as a bare
    /// address, and dismissing it is one action.
    /// </remarks>
    public Task<bool> NeedsSenderNameAsync(string account) =>
        _app is { } app
            ? Task.Run(() => app.NeedsSenderName(account))
            : Task.FromResult(true);
}
