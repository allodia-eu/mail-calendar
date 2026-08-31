// The user's last message-list pane width, i.e. where they left the draggable list|reading
// divider, persisted so the reading pane reopens at the size they chose instead of snapping back
// to the default proportion every launch. One number: the list pane width in logical (DPI-
// independent) pixels, stored beside the window placement in the preferences dir
// (AppPaths.PrefsDir, dev-isolated under a harness run, mirroring WindowStateStore).
// Best-effort: any read/write failure just falls back to the default.

using System.Globalization;
using System.IO;

namespace Allodia.Mailcal.Services;

/// <summary>Persists and restores the message-list pane width (the list|reading divider position).</summary>
internal static class PaneLayoutStore
{
    // Below this a saved width is treated as junk and ignored, the list column's own MinWidth is
    // 320, so anything this small never came from a real drag. (The reopen path clamps to the live
    // window regardless, so this is just a cheap garbage filter.)
    private const double MinWidth = 120;

    private static string FilePath => Path.Combine(AppPaths.PrefsDir, "pane.txt");

    // Its own file rather than a second number in pane.txt: an existing install already has a
    // one-number pane.txt, and a two-number format would have to be told apart from it at every
    // read. A separate file makes "never dragged the sidebar" simply a missing file.
    private static string SidebarFilePath => Path.Combine(AppPaths.PrefsDir, "sidebar.txt");

    /// <summary>The stored list-pane width in logical pixels, or <c>null</c> when unset/unreadable.</summary>
    public static double? Read() => ReadWidth(FilePath);

    /// <summary>The stored folder-pane width in logical pixels, or <c>null</c> when unset.</summary>
    public static double? ReadSidebar() => ReadWidth(SidebarFilePath);

    /// <summary>Stores the folder-pane <paramref name="width"/> (logical pixels). Best-effort.</summary>
    public static void WriteSidebar(double width) => WriteWidth(SidebarFilePath, width);

    private static double? ReadWidth(string path)
    {
        try
        {
            if (double.TryParse(File.ReadAllText(path).Trim(), NumberStyles.Float,
                    CultureInfo.InvariantCulture, out var width) && width >= MinWidth)
            {
                return width;
            }
            return null;
        }
        catch
        {
            return null;
        }
    }

    /// <summary>Stores <paramref name="width"/> (logical pixels). Best-effort.</summary>
    public static void Write(double width) => WriteWidth(FilePath, width);

    private static void WriteWidth(string path, double width)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            File.WriteAllText(path, width.ToString("0.##", CultureInfo.InvariantCulture));
        }
        catch
        {
            // If persistence fails the divider simply opens at the default proportion next time.
        }
    }
}
