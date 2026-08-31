// Whether this process is running with MSIX package identity (the Store/packaged shape) or as the
// unpackaged dev-loop exe. It decides two things that must agree: which custom URI scheme this
// build claims for OAuth redirects (MicrosoftOAuthConfig.Scheme), and who registers it, the
// package manifest for a packaged build, Program.RegisterProtocolForUnpackaged for a dev one.
//
// It is deliberately NOT `#if DEBUG`: the discriminator is packaging, not configuration. A
// Release-configuration unpackaged build (build-and-run.ps1 -Configuration Release) is still a dev
// build sitting beside an installed Store app, and must not claim the Store app's scheme.

using System;

namespace Allodia.Mailcal.Services;

/// <summary>Which shape this build is running in, packaged (Store/MSIX) or unpackaged (dev loop).</summary>
internal static class AppIdentity
{
    // Package.Current THROWS when there is no package identity, so this cannot be a plain property
    // read; resolve it once and cache, since it can't change within a process.
    private static readonly Lazy<bool> Packaged = new(() =>
    {
        try
        {
            return Windows.ApplicationModel.Package.Current is not null;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
    });

    /// <summary>
    /// Whether this process has MSIX package identity. <c>true</c> for the Store/packaged build
    /// (<c>package.ps1</c>), <c>false</c> for the unpackaged dev-loop exe (<c>build-and-run.ps1</c>).
    /// </summary>
    public static bool IsPackaged => Packaged.Value;

    // The same read, for the package VERSION. It lives here rather than at the call site so the
    // WinRT dependency stays in this one file: `Log` composes the session marker from it, and a
    // marker built from a parameter is a rule the plain net10.0 suite can pin (SessionMarkerTests)
    // A `Package.Current` read could only ever be exercised inside an actual MSIX.
    //
    // It catches wider than IsPackaged does, and the asymmetry is deliberate. That answer is
    // load-bearing, a wrong `false` would claim the unpackaged OAuth scheme from inside the Store
    // app, so anything beyond the documented no-identity throw should surface rather than be
    // guessed at. This one only decorates a log line, and it is read on the startup path OUTSIDE
    // Log.Init's own try/catch, so an unexpected failure here must cost the second version number
    // and nothing else. A logger may never take startup down.
    private static readonly Lazy<string?> Version = new(() =>
    {
        try
        {
            var version = Windows.ApplicationModel.Package.Current.Id.Version;
            return $"{version.Major}.{version.Minor}.{version.Build}.{version.Revision}";
        }
        catch (Exception)
        {
            return null;
        }
    });

    /// <summary>
    /// The MSIX package version (<c>major.minor.build.revision</c>) when this process is packaged,
    /// or <c>null</c> for the unpackaged dev loop. It is the only version that moves per package:
    /// the assembly version comes from <c>/VERSION</c>, which holds the last <em>released</em>
    /// version (<c>docs/versioning.md</c>).
    /// </summary>
    public static string? PackageVersion => Version.Value;
}
