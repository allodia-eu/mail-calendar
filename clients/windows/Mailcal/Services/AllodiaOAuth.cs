// Config for signing in to an **Allodia account** on Windows. The Rust core owns the whole OAuth
// state machine, it discovers the account service from the standards (RFC 9728 → 8414), runs the
// PKCE exchange, asks whose account it is and writes the grant to the Credential Manager, so this
// host owns only opening the authorization URL in the default browser and catching the redirect,
// which returns through the same custom-scheme protocol activation the Microsoft and JMAP sign-ins
// use (ProtocolAuthCallback), under its own host so the three can't cross.
//
// An Allodia account is not a mail account: it carries no mailbox, appears in no switcher, and a
// token issued for it cannot touch anyone's mail. Its screen is Settings → Accounts.

namespace Allodia.Mailcal.Services;

/// <summary>
/// The redirect this client registers with the account service. The client id is injected into the
/// core at build time and never appears here; a build given none has no Allodia sign-in at all,
/// and <c>AllodiaSignInAvailable()</c> is what the settings screen asks before drawing anything.
/// </summary>
internal static class AllodiaOAuthConfig
{
    /// <summary>
    /// The redirect host that identifies an Allodia callback (Microsoft's is <c>auth</c>, JMAP's
    /// <c>jmap-oauth</c>). Distinct because <see cref="ProtocolAuthCallback"/> matches an arriving
    /// activation to the flow that armed it by exactly this label: two flows sharing one is a
    /// redirect delivered to the wrong flow, which fails by never coming back rather than by
    /// erroring.
    /// </summary>
    public const string CallbackHost = "account-oauth";

    /// <summary>
    /// The full redirect URI handed to <c>begin_allodia_sign_in</c>. It rides the app's single
    /// registered scheme (<see cref="MicrosoftOAuthConfig.Scheme"/>), which
    /// <c>Package.appxmanifest</c> declares for packaged builds and <c>Program</c> registers at
    /// runtime for the unpackaged dev loop.
    /// <para>
    /// That scheme differs between the two shapes, and unlike JMAP this registration is
    /// <em>static</em>, nothing is minted per install, so the account service has to list BOTH
    /// forms, exactly as the Azure app registration already does. A dev build whose <c>.dev</c>
    /// form is missing there is refused at the authorization endpoint with
    /// <c>redirect_uri_mismatch</c>, before the browser ever comes back.
    /// </para>
    /// </summary>
    public static string RedirectUri => MicrosoftOAuthConfig.Scheme + "://" + CallbackHost;
}
