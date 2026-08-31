// The user's language choice, "system" (follow the OS), or any locale the shared catalog
// ships ("en", "nl", "de", …), persisted to a one-line file so it survives a restart. The
// set of accepted values is L10n.Locales, straight from the catalog, so adding a language to
// messages/ needs no edit here. The override lives in an in-memory MRT-Core
// ResourceContext (see L10n.SetLanguage), which resets each launch, so the app stores the
// choice here and re-applies it at startup (App's constructor). Lives in the preferences dir
// (AppPaths.PrefsDir: next to the engine store, logs, and credentials, dev-isolated under a
// harness run). Best-effort: any read/write failure falls back to the system language.

using System.IO;

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
    /// (follow the OS) for "system", else the BCP-47 tag. Routes through
    /// <see cref="L10n.SetLanguage"/> (an MRT-Core ResourceContext qualifier), which, unlike
    /// ApplicationLanguages.PrimaryLanguageOverride, needs no package identity, so it works in
    /// the unpackaged dev build. Call at startup before any window or resource loads, and again
    /// whenever the user changes the picker.
    /// </summary>
    /// <remarks>
    /// The choice drives the *dates* as well as the words, via <see cref="AppCulture"/>, the
    /// resource qualifier alone leaves <c>CultureInfo</c> on the host's regional format, which is
    /// how a fully Dutch UI ended up listing "Mon"/"Sun" (see the note in that file).
    /// </remarks>
    public static void Apply(string choice)
    {
        L10n.SetLanguage(L10n.Locales.Contains(choice) ? choice : "");
        AppCulture.Apply(choice);
    }
}
