// Config + the OS-secure-store sink for the Microsoft 365 OAuth sign-in on Windows.
// The Rust core owns the OAuth state machine (PKCE, token exchange, refresh); this host owns only
// opening the authorization URL in the user's default browser (reusing its logged-in Microsoft
// session) and catching the redirect. The browser returns to our registered custom scheme
// (eu.allodia.mailcal://auth), which the OS delivers as a protocol activation, routed by `Program`
// into ProtocolAuthCallback (shared with the JMAP sign-in), which the in-flight sign-in awaits.
// (Earlier this used an http://localhost loopback listener; the custom scheme matches
// macOS/Android and drops the stray "you can close this tab" browser page, and with it that
// page's encoding/localisation warts.)

namespace Allodia.Mailcal.Services;

/// <summary>
/// The half of the Azure app registration that stays with the host. The client id is injected into
/// the core at build time; the redirect cannot be, because Azure registers it against this app's
/// own identity, register <see cref="RedirectUri"/> under "Mobile and desktop applications" in
/// the Azure portal, character-for-character. This is a public client: PKCE, owned by the core,
/// stands in for a secret.
/// </summary>
internal static class MicrosoftOAuthConfig
{
    public const string Tenant = "common";

    /// <summary>
    /// The custom URI scheme the Store build claims. Reverse-DNS of <c>allodia.eu</c> so it can't
    /// collide with another app's protocol registration, and declared in
    /// <c>Package.appxmanifest</c>.
    /// </summary>
    public const string PackagedScheme = OAuthScheme.Packaged;

    /// <summary>
    /// The scheme an <em>unpackaged</em> dev build claims instead, because a developer's machine
    /// has the Store build installed too, and Windows registers a scheme per user, not per build.
    /// Both claiming <see cref="PackagedScheme"/> means the OS cannot tell them apart: it puts up a
    /// "select an app" picker for a redirect carrying a live one-time auth code, sends it to
    /// whichever is chosen, and a stray "Always" wires every future sign-in, including the real
    /// app's, to the wrong one, silently and permanently. A separate dev scheme removes the
    /// ambiguity at the source. It is registered with Azure as a second redirect URI.
    /// </summary>
    public const string UnpackagedScheme = OAuthScheme.Unpackaged;

    /// <summary>
    /// The custom URI scheme this build redirects back to: <see cref="PackagedScheme"/> when
    /// running with package identity, <see cref="UnpackagedScheme"/> in the dev loop. Registered by
    /// <c>Package.appxmanifest</c> and by <c>Program.RegisterProtocolForUnpackaged</c>
    /// respectively, both driven by the same <see cref="AppIdentity.IsPackaged"/> predicate, so
    /// the scheme a build *claims* and the scheme it *uses* cannot drift apart. The app owns
    /// exactly one scheme at a time, every browser sign-in that returns through a protocol
    /// activation shares it and is told apart by the redirect's host (see
    /// <see cref="ProtocolAuthCallback"/>).
    /// </summary>
    public static string Scheme => OAuthScheme.For(AppIdentity.IsPackaged);

    /// <summary>The redirect host that identifies a Microsoft callback (JMAP's is <c>jmap-oauth</c>).</summary>
    public const string CallbackHost = "auth";

    /// <summary>
    /// The full redirect URI, must be registered in the Azure "Mobile and desktop applications"
    /// redirect list exactly. BOTH forms are registered there, since a dev build sends the
    /// <c>.dev</c> one. Deliberately not MSAL-format (no <c>msauth</c> prefix): that prefix is an
    /// MSAL-SDK requirement on Apple/Android, and this client rolls its own PKCE flow, so any
    /// registered custom scheme works.
    /// </summary>
    public static string RedirectUri => Scheme + "://" + CallbackHost;
}

/// <summary>
/// The Credential Manager, as the core sees it: the only way an account's credential is written or
/// erased on this device. The core calls it when an account is added, when a refresh token
/// rotates, when a grant is re-authorised, and when an account is removed. One sink for every
/// provider family: there were three, with identical bodies, behind three identical ports, which
/// is what made forgetting one cheap.
///
/// Both methods throw rather than swallowing, the core decides what a refused write means, and it
/// decides differently depending on what it was doing (a failed add is rolled back; a failed
/// rotation cannot be). Reporting success on a write that did not happen is what the old,
/// return-less port made unavoidable.
/// </summary>
internal sealed class CredentialStoreSink : uniffi.mailcal_bindings.AccountCredentialStore
{
    public void Persist(string accountId, string configToml)
    {
        if (!CredentialStore.Save(accountId, configToml))
        {
            throw new uniffi.mailcal_bindings.CredentialStoreException.Store(
                "the Windows Credential Manager refused to store this account");
        }
    }

    public void Delete(string accountId)
    {
        if (!CredentialStore.Remove(accountId))
        {
            throw new uniffi.mailcal_bindings.CredentialStoreException.Store(
                "the Windows Credential Manager refused to erase this account");
        }
    }
}
