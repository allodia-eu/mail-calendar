// Config for signing in to an **Allodia account** on Windows. The core runs the whole sign-in as its
// one browser sign-in, so this host owns only the redirect it is started with: the browser returns
// through the same custom-scheme protocol activation the Microsoft and JMAP sign-ins use, under its
// own host, and Program hands it to the browser sign-in (MailboxModel.BrowserSignIn.cs).
//
// An Allodia account is not a mail account: it carries no mailbox, appears in no switcher, and a
// token issued for it cannot touch anyone's mail. Its screen is Settings → Allodia account.

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
    /// <c>jmap-oauth</c>). Distinct because an arriving activation is matched to the sign-in
    /// waiting on it by scheme and host (<c>BrowserSignInRules.Returns</c>): two flows sharing one
    /// is a redirect delivered to the wrong flow, which fails by never coming back rather than by
    /// erroring.
    /// </summary>
    public const string CallbackHost = "account-oauth";

    /// <summary>
    /// The full redirect URI the sign-in is started with (<c>AllodiaIntent.SignIn</c>). It rides
    /// the app's single registered scheme (<see cref="MicrosoftOAuthConfig.Scheme"/>), which
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
