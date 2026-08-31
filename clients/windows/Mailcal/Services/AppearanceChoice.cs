// Which light/dark appearance the app paints in, and where that answer comes from at launch.
//
// The choice itself is a CORE setting (docs/settings.md → General), persisted in preferences.toml
// beside every other display preference, so the four clients cannot each invent their own default.
// It is read here rather than pulled off MailcalApp because the window is built long before the
// core is: MailcalApp.NewAccounts opens the engine store and starts dialing, and a window painted
// in the desktop's scheme until that returns is a visible flash of exactly the theme the user said
// they did not want. `stored_appearance` reads the one small TOML file and nothing else.
//
// WinUI-free on purpose, so the resolution rule is pinned in Mailcal.Tests; MainWindow.Theme.cs
// owns the ElementTheme half.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Resolves the appearance the app comes up in.</summary>
internal static class AppearanceChoice
{
    /// <summary>
    /// The appearance a launch paints with: the DEBUG-only <c>MAILCAL_APPEARANCE</c> override when
    /// it names one, else whatever the core has persisted.
    /// </summary>
    public static Appearance AtLaunch => Resolve(OverrideRaw, Stored);

    /// <summary>
    /// The stored choice, straight out of the (dev-isolated) preferences directory the core writes
    /// to. Falls back to following the host when the file is missing or unreadable.
    /// </summary>
    private static Appearance Stored => MailcalBindingsMethods.StoredAppearance(AppPaths.PrefsDir);

    /// <summary>
    /// <c>MAILCAL_APPEARANCE</c>, or null in a release build, a shipped app must not have its
    /// theme flipped by a stray environment variable, the same property the dev-account and
    /// showcase switches hold (scripts/ci/check-dev-account.sh).
    /// </summary>
    private static string? OverrideRaw
    {
        get
        {
#if DEBUG
            return Environment.GetEnvironmentVariable("MAILCAL_APPEARANCE");
#else
            return null;
#endif
        }
    }

    /// <summary>
    /// The pure rule: <paramref name="overrideRaw"/> when it names an appearance, else
    /// <paramref name="stored"/>. A value that names nothing we know is ignored rather than
    /// treated as "system", a typo'd override must not silently look like a working one.
    /// </summary>
    public static Appearance Resolve(string? overrideRaw, Appearance stored) =>
        overrideRaw?.Trim().ToLowerInvariant() switch
        {
            "system" => Appearance.System,
            "light" => Appearance.Light,
            "dark" => Appearance.Dark,
            _ => stored,
        };
}
