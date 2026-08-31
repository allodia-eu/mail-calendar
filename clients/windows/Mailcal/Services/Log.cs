// A tiny rotating file logger for field debugging, lifecycle, snapshot counts, time-zone
// reports, and errors land in %LOCALAPPDATA%\Allodia\MailCalendar\logs\app.log so a real
// user's issue (e.g. an empty calendar) can be diagnosed from the file. It is deliberately
// privacy-safe: log counts, zone ids, and high-level events, never mail/event content,
// addresses, or the config (which holds the password). Best-effort: logging never throws.
//
// Rotation is size-based, at 1 MB, app.log -> app.log.1 -> ... -> app.log.3 (oldest
// dropped), so the logs cap at ~4 MB total and never grow unbounded.

using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;

namespace Allodia.Mailcal.Services;

/// <summary>A best-effort, size-rotating file log under the app data directory.</summary>
internal static class Log
{
    private const long MaxBytes = 1024 * 1024; // 1 MB per file
    private const int Backups = 3;             // app.log + app.log.1..3 => ~4 MB cap

    private static readonly object Gate = new();
    private static string? _path;

    /// <summary>
    /// Points the log at <paramref name="dataDir"/>/logs and stamps a session start naming the
    /// build. <paramref name="packageVersion"/> is <see cref="AppIdentity.PackageVersion"/>, the
    /// MSIX package version when packaged, <c>null</c> for the unpackaged dev loop.
    /// </summary>
    public static void Init(string dataDir, string? packageVersion)
    {
        try
        {
            var dir = Path.Combine(dataDir, "logs");
            Directory.CreateDirectory(dir);
            _path = Path.Combine(dir, "app.log");
            Info(SessionMarker(
                AssemblyVersion(),
                packageVersion,
                $"{RuntimeInformation.OSArchitecture}, {RuntimeInformation.OSDescription}"));
        }
        catch
        {
            // A log that can't open simply stays silent; it must not break startup.
        }
    }

    /// <summary>
    /// The session-start line: the app version, the MSIX package version when there is one, and
    /// the device string.
    /// </summary>
    /// <remarks>
    /// Both versions arrive as parameters so the rule "the log names the build it came from" is a
    /// test that can fail (<c>SessionMarkerTests</c>), the packaged branch is otherwise reachable
    /// only from inside an actual MSIX, and this file reaches no other gate. Why both: <c>/VERSION</c>
    /// holds the last <em>released</em> version (docs/versioning.md), so a dev build and the shipped
    /// one report the same marketing version, and only the package version tells them apart.
    /// </remarks>
    internal static string SessionMarker(string appVersion, string? packageVersion, string device) =>
        packageVersion is null
            ? $"--- session start ({appVersion}, {device}) ---"
            : $"--- session start ({appVersion} package {packageVersion}, {device}) ---";

    /// <summary>The marketing version this build carries, the csproj derives it from <c>/VERSION</c>.</summary>
    private static string AssemblyVersion() =>
        Assembly.GetExecutingAssembly().GetName().Version?.ToString(3) ?? "0.0.0";

    /// <summary>Logs an informational line.</summary>
    public static void Info(string message) => Write("info", message);

    /// <summary>Logs a warning line.</summary>
    public static void Warn(string message) => Write("warn", message);

    /// <summary>Logs an error line.</summary>
    public static void Error(string message) => Write("error", message);

    /// <summary>Logs a debug line (verbose; emitted only when the core's level includes Debug).</summary>
    public static void Debug(string message) => Write("debug", message);

    /// <summary>Logs a trace line (the most verbose level).</summary>
    public static void Trace(string message) => Write("trace", message);

    // --- The Settings → Diagnostics read surface (viewer / status rows / export) -------------
    //
    // Reads are taken under the same Gate as writes so they never race a rotation, and are
    // best-effort like everything here. The read logic itself lives in DiagnosticsLog (WinUI-
    // free and unit-tested); this just binds it to the live path and the rotation constants.

    /// <summary>The absolute path of the current log file, or <c>null</c> before <see cref="Init"/>.</summary>
    public static string? FilePath => _path;

    /// <summary>
    /// Total bytes across the current file + backups, and how many backups exist, the
    /// Diagnostics status rows. <c>(0, 0)</c> before <see cref="Init"/> or on failure.
    /// </summary>
    public static (long TotalBytes, int BackupCount) Snapshot()
    {
        if (_path is not { } path)
        {
            return (0, 0);
        }
        lock (Gate)
        {
            return DiagnosticsLog.Snapshot(path, Backups);
        }
    }

    /// <summary>
    /// The current file's content for the inline viewer (newest last), capped at twice the
    /// rotation size in case a rotation ever failed. Empty before <see cref="Init"/> or when
    /// unreadable.
    /// </summary>
    public static string ReadCurrent()
    {
        if (_path is not { } path)
        {
            return string.Empty;
        }
        lock (Gate)
        {
            return DiagnosticsLog.ReadTail(path, 2 * MaxBytes);
        }
    }

    /// <summary>
    /// The current file's raw bytes for the export flow (the live file only, like the
    /// Android/Apple share, see docs/logging.md), buffered under the gate (rotation caps it
    /// at ~1 MB) so the copy to the user's chosen file never blocks or races the logger. Empty
    /// before <see cref="Init"/>.
    /// </summary>
    public static byte[] ExportSnapshot()
    {
        if (_path is not { } path)
        {
            return [];
        }
        lock (Gate)
        {
            return DiagnosticsLog.ReadAllShared(path);
        }
    }

    private static void Write(string level, string message)
    {
        var path = _path;
        if (path is null)
        {
            return;
        }
        lock (Gate)
        {
            try
            {
                Rotate(path);
                var line = $"{DateTimeOffset.Now:yyyy-MM-dd HH:mm:ss.fff zzz} [{level}] {message}\n";
                File.AppendAllText(path, line);
            }
            catch
            {
                // Logging is best-effort; a transient IO failure is swallowed.
            }
        }
    }

    // app.log.(Backups-1) -> app.log.Backups, ..., app.log -> app.log.1, dropping the oldest.
    private static void Rotate(string path)
    {
        var info = new FileInfo(path);
        if (!info.Exists || info.Length < MaxBytes)
        {
            return;
        }
        var oldest = $"{path}.{Backups}";
        if (File.Exists(oldest))
        {
            File.Delete(oldest);
        }
        for (var i = Backups - 1; i >= 1; i--)
        {
            var src = $"{path}.{i}";
            if (File.Exists(src))
            {
                File.Move(src, $"{path}.{i + 1}", overwrite: true);
            }
        }
        File.Move(path, $"{path}.1", overwrite: true);
    }
}
