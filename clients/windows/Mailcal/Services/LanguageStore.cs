// The user's language choice, "system" (follow the OS), or any locale the shared catalog
// ships ("en", "nl", "de", …), persisted to a one-line file so it survives a restart. The
// set of accepted values is L10n.Locales, straight from the catalog, so adding a language to
// messages/ needs no edit here. The override lives in an in-memory MRT-Core
// ResourceContext (see L10n.SetLanguage), which resets each launch, so the app stores the
// choice here and re-applies it at startup (App's constructor). Lives in the preferences dir
// (AppPaths.PrefsDir: next to the engine store, logs, and credentials, dev-isolated under a
// harness run). Best-effort: any read/write failure falls back to the system language.

using System.IO;
using System.Runtime.InteropServices;

namespace Allodia.Mailcal.Services;

/// <summary>Persists and applies the user's language-override choice across launches.</summary>
internal static class LanguageStore
{
    private static string FilePath => Path.Combine(AppPaths.PrefsDir, "language.txt");

    /// <summary>
    /// The stored choice ("system", or a catalog locale such as "en" / "de"); "system" when
    /// unset, unreadable, or naming a language this build doesn't ship.
    /// </summary>
    public static string Read()
    {
        try
        {
            var choice = File.ReadAllText(FilePath).Trim();
            return L10n.Locales.Contains(choice) ? choice : "system";
        }
        catch
        {
            return "system";
        }
    }

    /// <summary>Stores the choice (creating the directory if needed). Best-effort.</summary>
    public static void Write(string choice)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(FilePath)!);
            File.WriteAllText(FilePath, choice);
        }
        catch
        {
            // If persistence fails the choice still applies for this session.
        }
    }

    /// <summary>
    /// Applies <paramref name="choice"/> to the resource language override, the empty string
    /// (follow the OS) for "system", else the BCP-47 tag. Call at startup before any window or
    /// resource loads, and again whenever the user changes the picker.
    /// </summary>
    /// <remarks>
    /// Three things follow it. Our own strings, through <see cref="L10n.SetLanguage"/> (an MRT-Core
    /// ResourceContext qualifier). WinUI's own strings (the navigation pane's Settings item, an
    /// InfoBar icon's spoken name), which come from its resources rather than ours, through
    /// <see cref="ApplyToWinUi"/>; without it they follow the OS display language whatever was
    /// chosen. And the *dates*, through <see cref="AppCulture"/>: the resource qualifier alone
    /// leaves <c>CultureInfo</c> on the host's regional format, which is how a fully Dutch UI
    /// ended up listing "Mon"/"Sun" (see the note in that file).
    /// </remarks>
    public static void Apply(string choice)
    {
        var language = L10n.Locales.Contains(choice) ? choice : "";
        L10n.SetLanguage(language);
        ApplyToWinUi(language);
        AppCulture.Apply(choice);
    }

    /// <summary>
    /// Sets the override WinUI's own resources are read through: <paramref name="language"/>, or
    /// none for the empty string.
    /// </summary>
    /// <remarks>
    /// ⚠️ The Windows App SDK's setter refuses the empty string in both build shapes: it accepts only
    /// a well-formed language tag, and the refusal thrown from <c>App</c>'s constructor ends the
    /// launch inside <c>Application.Start</c>, before the window exists. "Follow the system" is the
    /// default, so it is never handed to it.
    /// <list type="bullet">
    /// <item>Packaged, that setter forwards to <c>Windows.Globalization</c>'s override, which belongs
    /// to the package and persists across launches, so a language chosen in an earlier session is
    /// still set and has to be cleared. That override takes the empty string as "none".</item>
    /// <item>Unpackaged, the override lives in this process and starts empty, so a launch follows
    /// the OS already. A switch back to "system" mid-session takes effect at the restart the picker
    /// asks for.</item>
    /// </list>
    /// Best-effort: a refused override costs WinUI's own strings their language, never the launch.
    /// </remarks>
    private static void ApplyToWinUi(string language)
    {
        try
        {
            if (language.Length > 0)
            {
                Microsoft.Windows.Globalization.ApplicationLanguages.PrimaryLanguageOverride = language;
            }
            else if (AppIdentity.IsPackaged)
            {
                Windows.Globalization.ApplicationLanguages.PrimaryLanguageOverride = "";
            }
        }
        catch (Exception ex) when (ex is COMException or ArgumentException)
        {
            Log.Warn($"language: the built-in controls keep the display language, the override was refused ({ex.GetType().Name} 0x{ex.HResult:X8})");
        }
    }
}
