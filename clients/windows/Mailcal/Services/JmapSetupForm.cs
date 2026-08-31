// The pure logic behind the setup form's JMAP tab, factored out of the WinUI view so it can be
// unit-tested without a renderer, the Windows twin of Android's plain canConnectJmap / JmapSetup
// assembly (AccountSetupScreen.kt) and macOS's canConnectJmap / submitJmapSetup. Two things here are
// load-bearing and invisible once wrong: which fields make Connect tappable (JMAP needs an email and
// ONE secret, a password and an API token are interchangeable now that the engine negotiates the
// auth scheme from the server's own WWW-Authenticate challenge, and never needs a server), and what
// the form hands the core, a blank server must arrive as null, since an empty-string URL would be
// taken as a real server instead of "discover it from my email domain" (docs/jmap.md rule 4).

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Pure field logic for the account-setup form's JMAP tab (no WinUI types, so it's testable).</summary>
internal static class JmapSetupForm
{
    /// <summary>
    /// Whether the typed fields can connect: a non-blank email plus the secret. The server is never
    /// required, it's discovered from the email domain, so it doesn't gate Connect.
    /// </summary>
    internal static bool CanConnect(string email, string secret) =>
        !string.IsNullOrWhiteSpace(email) && !string.IsNullOrEmpty(secret);

    /// <summary>
    /// Builds the FFI setup record from the raw form strings. A blank server means "discover it from
    /// my email domain", not an empty URL, so it is nulled. The secret goes across as
    /// <c>password</c> whatever kind it is: paired with the email as username it can be presented as
    /// Basic <em>or</em> Bearer, while a bare token could only ever be Bearer.
    /// </summary>
    internal static JmapSetup Build(string email, string serverUrl, string secret) =>
        new(
            Email: email,
            ServerUrl: string.IsNullOrWhiteSpace(serverUrl) ? null : serverUrl,
            Password: secret);
}
