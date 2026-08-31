using System;
using System.Globalization;
using System.Reflection;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>
/// The device facts every <c>MailcalApp</c> constructor reports to the core (<c>docs/analytics.md</c>).
///
/// <para>Two things to keep in mind when touching this:</para>
/// <list type="number">
/// <item><b>Report raw; the core coarsens.</b> We hand over the OS version and the host's own locale
/// tag (<c>nl-NL</c>); the core reduces them to a major and a language it ships before anything
/// crosses the wire. One tested reduction rule in Rust, not one per platform, and no client can
/// widen the payload by reporting something more precise than was asked for.</item>
/// <item><b>Nothing here is sent unless the user opted in.</b> These facts are handed to the core at
/// construction regardless, but the core mints no identifier and sends no event until consent is
/// given. Building this value is not "collecting" anything.</item>
/// </list>
///
/// <para>Windows exposes no reliable, permission-free way to tell a laptop from a desktop, and the
/// split is not worth a capability prompt, so every Windows install reports the same
/// <c>Pc</c> class. We deliberately do not read a device model at all; Partner Center already
/// reports models to us for free.</para>
/// </summary>
internal static class DeviceFacts
{
    /// <summary>Windows 11 is Windows 10.0 with a build number of 22000 or higher, the only
    /// honest way to report the *marketing* major the rest of the world means by "Windows 11".</summary>
    private const int Windows11FirstBuild = 22000;

    public static DeviceInfo Current()
    {
        return new DeviceInfo(
            Platform: Platform.Windows,
            OsVersion: OsVersion(),
            DeviceClass: DeviceClass.Pc,
            AppVersion: AppVersion(),
            Locale: CultureInfo.CurrentUICulture.Name);
    }

    private static string OsVersion()
    {
        var version = Environment.OSVersion.Version;
        return version.Build >= Windows11FirstBuild ? "11" : version.Major.ToString(CultureInfo.InvariantCulture);
    }

    private static string AppVersion()
    {
        var version = Assembly.GetExecutingAssembly().GetName().Version;
        return version?.ToString(3) ?? "0.0.0";
    }
}
