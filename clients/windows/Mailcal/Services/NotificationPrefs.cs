// The Settings → Notifications choice, persisted to a one-line file so it survives a restart.
// The Windows twin of Linux's `notifications_enabled` preference and Android/iOS's
// NotificationPrefs; a host preference, not a core one, because what a client may put on the
// desktop is the host's question (docs/background-sync.md).
//
// Default ON, matching every other client: the app was installed to deliver mail. The toggle
// gates POSTING only; the scan behind it still runs and still advances the core's high-water
// marks, so turning notifications off and back on never floods with a backlog.
//
// Lives in the preferences dir (AppPaths.PrefsDir: beside the language and log-level choices,
// dev-isolated under a harness run). Best-effort: an unreadable file reads as the default.

using System.IO;

namespace Allodia.Mailcal.Services;

/// <summary>Persists whether new-mail notifications are posted.</summary>
internal static class NotificationPrefs
{
    private static string FilePath => Path.Combine(AppPaths.PrefsDir, "notifications.txt");

    /// <summary>Whether new-mail notifications are on; <c>true</c> when unset or unreadable.</summary>
    public static bool Enabled()
    {
        try
        {
            return File.ReadAllText(FilePath).Trim() != "off";
        }
        catch
        {
            return true;
        }
    }

    /// <summary>Stores the choice (creating the directory if needed). Best-effort.</summary>
    public static void SetEnabled(bool enabled)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(FilePath)!);
            File.WriteAllText(FilePath, enabled ? "on" : "off");
        }
        catch
        {
            // If persistence fails the choice still applies for this session.
        }
    }
}
