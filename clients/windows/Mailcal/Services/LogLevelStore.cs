// The Settings → Diagnostics debug-toggle choice, "info" (default) or "debug", persisted to a
// one-line file so a support session's extra verbosity survives a restart until the user turns
// it back off. Lives in the preferences dir (AppPaths.PrefsDir: next to the engine store and
// language choice, dev-isolated under a harness run, mirroring LanguageStore). The boot path
// reads it through DiagnosticsLog.ResolveLevel, where the ALLODIA_LOG_LEVEL env var still wins
// as the dev escape hatch. Best-effort: any read/write failure falls back to "info".

using System.IO;

namespace Allodia.Mailcal.Services;

/// <summary>Persists the Diagnostics debug-logging choice across launches.</summary>
internal static class LogLevelStore
{
    private static string FilePath => Path.Combine(AppPaths.PrefsDir, "loglevel.txt");

    /// <summary>The stored choice ("info" | "debug"); "info" when unset or unreadable.</summary>
    public static string Read()
    {
        try
        {
            return File.ReadAllText(FilePath).Trim() == "debug" ? "debug" : "info";
        }
        catch
        {
            return "info";
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
            // If persistence fails the level still applies for this session.
        }
    }
}
