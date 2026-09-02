// Where an IMAP authorization server sends the browser back to. The sibling of JmapOAuth.cs, and
// as small for the same reason: the server was discovered at runtime rather than integrated at
// build time, so there is no client id here. The core registers this install with it (RFC 7591)
// and this host owns only the redirect and the browser hop.

namespace Allodia.Mailcal.Services;

/// <summary>The IMAP sign-in's redirect, under the app's own protocol scheme.</summary>
internal static class ImapOAuthConfig
{
    /// <summary>
    /// The redirect's host. Its own, beside the JMAP and Allodia ones: they share a scheme, and
    /// only the host tells them apart. A redirect handed to the wrong flow does not error, it is
    /// exchanged against a different client and the sign-in somebody is waiting on never comes
    /// back, which is why <c>ProtocolAuthCallback</c> is armed with this rather than the scheme.
    /// </summary>
    public const string CallbackHost = "imap-oauth";

    /// <summary>
    /// The redirect URI sent to the server at registration time, and replayed verbatim on every
    /// later refresh. Built from the app's protocol scheme so a re-branded build moves with it.
    /// </summary>
    public static string RedirectUri => MicrosoftOAuthConfig.Scheme + "://" + CallbackHost;
}
