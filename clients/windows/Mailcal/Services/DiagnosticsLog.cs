// The WinUI-free logic behind Settings → Diagnostics, factored out of the dialog so it can be
// unit-tested without a renderer (the same split as JmapSetupForm): the size/backup snapshot the
// status rows show, the tail read the inline viewer renders, the export payload read, the
// human-readable byte formatter, and the boot log-level resolution (env override > persisted
// preference > Info). All file IO here is best-effort and opens shared with the writer
// (FileShare.ReadWrite) so a concurrent append can never fail a read, it must never throw
// outward, matching Log's own discipline.

using System.Globalization;
using System.IO;
using System.Text;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Pure logic for the Diagnostics settings panel (no WinUI types, so it's testable).</summary>
internal static class DiagnosticsLog
{
    /// <summary>
    /// Total bytes across the current log file + its backups (<c>.1</c>..<c>.[backups]</c>),
    /// and how many backup files exist. Missing files count as absent; an IO failure returns
    /// whatever was summed so far.
    /// </summary>
    internal static (long TotalBytes, int BackupCount) Snapshot(string logPath, int backups)
    {
        long total = 0;
        var count = 0;
        try
        {
            var current = new FileInfo(logPath);
            if (current.Exists)
            {
                total += current.Length;
            }
            for (var i = 1; i <= backups; i++)
            {
                var backup = new FileInfo($"{logPath}.{i}");
                if (backup.Exists)
                {
                    total += backup.Length;
                    count++;
                }
            }
        }
        catch
        {
            // Best-effort, like every log read: an unreadable directory shows as what was summed.
        }
        return (total, count);
    }

    /// <summary>
    /// The last <paramref name="maxBytes"/> of the file (the whole file when smaller), newest
    /// last. A cut tail starts at the next line boundary so the viewer never opens mid-line.
    /// Opens shared with the writer, so a concurrent append can't fail the read; any failure
    /// (including a missing file) returns the empty string.
    /// </summary>
    internal static string ReadTail(string path, long maxBytes)
    {
        try
        {
            using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            var cut = stream.Length > maxBytes;
            if (cut)
            {
                stream.Seek(-maxBytes, SeekOrigin.End);
            }
            using var reader = new StreamReader(stream, Encoding.UTF8);
            var text = reader.ReadToEnd();
            if (cut)
            {
                // Drop the partial first line the byte cut landed in.
                var newline = text.IndexOf('\n');
                if (newline >= 0)
                {
                    text = text[(newline + 1)..];
                }
            }
            return text;
        }
        catch
        {
            return string.Empty;
        }
    }

    /// <summary>
    /// The current file's raw bytes for the export flow (backups are deliberately excluded: the
    /// export hands over the live file, exactly like the Android/Apple share, see
    /// docs/logging.md). Opens shared with the writer; a missing or unreadable file is empty.
    /// </summary>
    internal static byte[] ReadAllShared(string path)
    {
        try
        {
            using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            using var buffer = new MemoryStream();
            stream.CopyTo(buffer);
            return buffer.ToArray();
        }
        catch
        {
            return [];
        }
    }

    /// <summary>
    /// A human-readable size ("532 B", "1.2 KB", "3.8 MB") for the status row, in the current
    /// culture's decimal separator. SI units (powers of 1000), matching the platform-native
    /// formatters the Android/Apple status rows use (Formatter.formatShortFileSize /
    /// ByteCountFormatter); the log caps at ~4 MB, so MB is the largest unit needed.
    /// </summary>
    internal static string FormatBytes(long bytes)
    {
        if (bytes < 1000)
        {
            return $"{bytes} B";
        }
        if (bytes < 1_000_000)
        {
            return $"{(bytes / 1000.0).ToString("0.#", CultureInfo.CurrentCulture)} KB";
        }
        return $"{(bytes / 1_000_000.0).ToString("0.#", CultureInfo.CurrentCulture)} MB";
    }

    /// <summary>
    /// The log level to boot with: the <c>ALLODIA_LOG_LEVEL</c> env var wins when it names a
    /// level (the pre-existing dev/support escape hatch, and the only way to reach warn/error/
    /// trace); otherwise the persisted Settings → Diagnostics choice ("debug" from the toggle);
    /// otherwise Info, which keeps the rotating log useful over a long window.
    /// </summary>
    internal static LogLevel ResolveLevel(string? envValue, string? storedPreference) =>
        envValue?.Trim().ToLowerInvariant() switch
        {
            "error" => LogLevel.Error,
            "warn" => LogLevel.Warn,
            "info" => LogLevel.Info,
            "debug" => LogLevel.Debug,
            "trace" => LogLevel.Trace,
            _ => storedPreference?.Trim().ToLowerInvariant() == "debug" ? LogLevel.Debug : LogLevel.Info,
        };
}
