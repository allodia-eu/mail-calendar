// The custom URI scheme this build claims for OAuth redirects, the pure half, split from
// MicrosoftOAuthConfig so it can be unit-tested. (MicrosoftOAuthConfig reaches AppIdentity, which
// touches WinRT and so cannot compile into the plain net10.0 test assembly; the *choice* can.)
//
// Two schemes, and getting either wrong is silent and expensive:
//
//   - Packaged MUST equal the scheme in Package.appxmanifest and the Azure redirect registration,
//     character for character. Get it wrong and the SHIPPED app's sign-in breaks, the OS would
//     deliver its redirect to nothing. It is the application id (docs/branding.md), taken from the
//     generated Brand rather than written again, and OAuthSchemeTests reads the manifest and
//     asserts the match, so the "must match" is a check rather than a comment.
//   - Unpackaged exists because Windows registers a protocol PER USER, not per build. A developer's
//     machine has the Store app installed and runs dev builds; when both claimed one scheme the OS
//     could only put up a "select an app" picker for a redirect carrying a live auth code, and an
//     "Always" would have permanently routed the shipped app's sign-ins to a debug build.

namespace Allodia.Mailcal.Services;

/// <summary>Which custom URI scheme a build claims, by packaging shape.</summary>
internal static class OAuthScheme
{
    /// <summary>The scheme a packaged (Store/MSIX) build claims. Mirrored in the app manifest.</summary>
    public const string Packaged = Brand.AppId;

    /// <summary>The scheme an unpackaged dev build claims instead, so the two can coexist.</summary>
    public const string Unpackaged = Packaged + ".dev";

    /// <summary>The scheme for a build with (or without) MSIX package identity.</summary>
    public static string For(bool packaged) => packaged ? Packaged : Unpackaged;
}
