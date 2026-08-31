// Config + the OS-secure-store sink for the JMAP "sign in with your provider" flow on Windows.
// The Rust core owns everything provider-specific here, it DISCOVERS the authorization server
// from the standards (RFC 9728 → 8414), registers this install as a client (RFC 7591), and runs
// the PKCE exchange, so unlike Microsoft/Google there is no client id to embed and no per-server
// code. This host owns only opening the authorization URL in the user's default browser and
// catching the redirect, which returns through the same custom-scheme protocol activation the
// Microsoft sign-in uses (ProtocolAuthCallback), under its own host so the two can't cross.

namespace Allodia.Mailcal.Services;

/// <summary>
/// The redirect this client registers with a JMAP provider at each sign-in. There is no client id
/// or secret: dynamic client registration mints one per install, in the core.
/// </summary>
internal static class JmapOAuthConfig
{
    /// <summary>The redirect host that identifies a JMAP callback (Microsoft's is <c>auth</c>).</summary>
    public const string CallbackHost = "jmap-oauth";

    /// <summary>
    /// The full redirect URI handed to <c>begin_jmap_login</c>, and sent verbatim to the provider
    /// on registration and on every later refresh, a server that sees a different one rejects the
    /// grant. It rides the app's single registered scheme (<see cref="MicrosoftOAuthConfig.Scheme"/>),
    /// which <c>Package.appxmanifest</c> declares for packaged builds and <c>Program</c> registers
    /// at runtime for the unpackaged dev loop, so no separate registration is needed. That scheme
    /// differs between the two shapes, which costs this flow nothing: dynamic client registration
    /// (RFC 7591) sends whatever redirect URI we hand it, so a dev build registers itself with the
    /// provider under the dev scheme without any portal entry, unlike the Microsoft flow, whose
    /// Azure registration has to list both.
    /// </summary>
    public static string RedirectUri => MicrosoftOAuthConfig.Scheme + "://" + CallbackHost;
}
