// The user's last main-window placement, position, size, and whether it was maximised,
// persisted to a one-line file so the app reopens exactly as they left it instead of snapping
// back to a fixed default every launch. Lives in the preferences dir (AppPaths.PrefsDir: next
// to the engine store, logs, and language, dev-isolated under a harness run, mirroring
// LanguageStore). Best-effort: any read/write failure simply falls back to the default size.

using System.Globalization;
using System.IO;

namespace Allodia.Mailcal.Services;

/// <summary>A saved top-level window placement: restored bounds plus the maximised flag.</summary>
/// <remarks>
/// The bounds are the <em>restored</em> (un-maximised) rectangle in physical pixels, even when
/// <see cref="Maximized"/> is true, so the app can reopen maximised yet still return to the
/// right size when the user un-maximises.
/// </remarks>
internal readonly record struct WindowPlacement(int X, int Y, int Width, int Height, bool Maximized);

/// <summary>Persists and restores the main window's placement across launches.</summary>
internal static class WindowStateStore
{
    // Below this a saved size is treated as junk (a collapsed or garbage record) and ignored, so
    // a bad value can never reopen the app as an unusable sliver.
    private const int MinWidth = 480;
    private const int MinHeight = 360;

    private static string FilePath => Path.Combine(AppPaths.PrefsDir, "window.txt");

    /// <summary>
    /// The stored placement, or <c>null</c> when unset, unreadable, or implausibly small. The
    /// caller falls back to its default size when this is <c>null</c>.
    /// </summary>
    public static WindowPlacement? Read()
    {
        try
        {
            var parts = File.ReadAllText(FilePath).Trim().Split(' ');
            if (parts.Length != 5
                || !int.TryParse(parts[0], NumberStyles.Integer, CultureInfo.InvariantCulture, out var x)
                || !int.TryParse(parts[1], NumberStyles.Integer, CultureInfo.InvariantCulture, out var y)
                || !int.TryParse(parts[2], NumberStyles.Integer, CultureInfo.InvariantCulture, out var w)
                || !int.TryParse(parts[3], NumberStyles.Integer, CultureInfo.InvariantCulture, out var h)
                || !int.TryParse(parts[4], NumberStyles.Integer, CultureInfo.InvariantCulture, out var max)
                || w < MinWidth || h < MinHeight)
            {
                return null;
            }
            return new WindowPlacement(x, y, w, h, max != 0);
        }
        catch
        {
            return null;
        }
    }

    /// <summary>Stores <paramref name="placement"/> (creating the directory if needed). Best-effort.</summary>
    public static void Write(WindowPlacement placement)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(FilePath)!);
            var line = string.Join(' ',
                placement.X.ToString(CultureInfo.InvariantCulture),
                placement.Y.ToString(CultureInfo.InvariantCulture),
                placement.Width.ToString(CultureInfo.InvariantCulture),
                placement.Height.ToString(CultureInfo.InvariantCulture),
                placement.Maximized ? "1" : "0");
            File.WriteAllText(FilePath, line);
        }
        catch
        {
            // If persistence fails the window simply opens at the default size next time.
        }
    }
}
