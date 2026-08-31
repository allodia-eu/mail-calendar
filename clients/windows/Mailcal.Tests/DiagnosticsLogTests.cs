// The Diagnostics settings panel's WinUI-free logic (Services/DiagnosticsLog.cs), pinned where
// it is load-bearing and invisible once wrong: the boot log-level precedence (the env var must
// keep winning over the new persisted toggle, or a dev override silently stops working), the
// tail read that must start on a line boundary and tolerate a concurrently open writer, the
// export payload that must be the current file only (like the Android/Apple share), and the
// size/backup snapshot behind the status rows. File IO runs against throwaway temp directories
// The real log path is never touched.

using System.Globalization;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class DiagnosticsLogTests
{
    // --- ResolveLevel: env override > persisted preference > Info --------------------------

    [Fact]
    public void Env_var_wins_over_the_persisted_preference()
    {
        // The dev escape hatch keeps working even when the settings toggle says otherwise.
        Assert.Equal(LogLevel.Warn, DiagnosticsLog.ResolveLevel("warn", "debug"));
        Assert.Equal(LogLevel.Trace, DiagnosticsLog.ResolveLevel("trace", "info"));
        // Explicit env "info" pins Info even with the toggle on.
        Assert.Equal(LogLevel.Info, DiagnosticsLog.ResolveLevel("info", "debug"));
        // Case and whitespace are forgiven, like the pre-existing env parsing.
        Assert.Equal(LogLevel.Debug, DiagnosticsLog.ResolveLevel(" DEBUG ", "info"));
    }

    [Fact]
    public void Preference_applies_when_the_env_var_is_unset_or_unrecognized()
    {
        Assert.Equal(LogLevel.Debug, DiagnosticsLog.ResolveLevel(null, "debug"));
        Assert.Equal(LogLevel.Debug, DiagnosticsLog.ResolveLevel("", "debug"));
        // A garbage env value falls through to the preference, not to Info.
        Assert.Equal(LogLevel.Debug, DiagnosticsLog.ResolveLevel("verbose", "debug"));
        Assert.Equal(LogLevel.Info, DiagnosticsLog.ResolveLevel(null, "info"));
        Assert.Equal(LogLevel.Info, DiagnosticsLog.ResolveLevel(null, null));
    }

    // --- FormatBytes ------------------------------------------------------------------------

    [Fact]
    public void FormatBytes_picks_the_SI_unit_and_rounds_to_one_decimal()
    {
        // SI units (powers of 1000), matching the platform-native formatters the Android/Apple
        // status rows use, a 1.2 MB store must not read differently across the clients.
        RunWithCulture(CultureInfo.InvariantCulture, () =>
        {
            Assert.Equal("0 B", DiagnosticsLog.FormatBytes(0));
            Assert.Equal("532 B", DiagnosticsLog.FormatBytes(532));
            Assert.Equal("1 KB", DiagnosticsLog.FormatBytes(1000));
            Assert.Equal("1.2 KB", DiagnosticsLog.FormatBytes(1229));
            Assert.Equal("1 MB", DiagnosticsLog.FormatBytes(1_000_000));
            Assert.Equal("3.8 MB", DiagnosticsLog.FormatBytes(3_800_000));
        });
    }

    [Fact]
    public void FormatBytes_uses_the_current_cultures_decimal_separator()
    {
        // A Dutch user reads "1,2 MB", not "1.2 MB".
        RunWithCulture(new CultureInfo("nl-NL"), () =>
            Assert.Equal("1,2 KB", DiagnosticsLog.FormatBytes(1229)));
    }

    // --- Snapshot ---------------------------------------------------------------------------

    [Fact]
    public void Snapshot_sums_the_current_file_and_backups_and_counts_backups()
    {
        RunInTempDir(dir =>
        {
            var log = Path.Combine(dir, "app.log");
            File.WriteAllBytes(log, new byte[100]);
            File.WriteAllBytes($"{log}.1", new byte[50]);
            // .2 deliberately absent, a gap must not stop the count at the hole.
            File.WriteAllBytes($"{log}.3", new byte[25]);

            var (total, backups) = DiagnosticsLog.Snapshot(log, backups: 3);
            Assert.Equal(175, total);
            Assert.Equal(2, backups);
        });
    }

    [Fact]
    public void Snapshot_of_a_missing_log_is_zero()
    {
        RunInTempDir(dir =>
        {
            var (total, backups) = DiagnosticsLog.Snapshot(Path.Combine(dir, "app.log"), backups: 3);
            Assert.Equal(0, total);
            Assert.Equal(0, backups);
        });
    }

    // --- ReadTail ---------------------------------------------------------------------------

    [Fact]
    public void ReadTail_returns_the_whole_file_when_it_fits()
    {
        RunInTempDir(dir =>
        {
            var log = Path.Combine(dir, "app.log");
            File.WriteAllText(log, "first line\nsecond line\n");
            Assert.Equal("first line\nsecond line\n", DiagnosticsLog.ReadTail(log, maxBytes: 1024));
        });
    }

    [Fact]
    public void ReadTail_cut_starts_at_a_line_boundary_and_keeps_the_newest_lines()
    {
        RunInTempDir(dir =>
        {
            var log = Path.Combine(dir, "app.log");
            // 100 fixed-width lines ("line-000\n" … "line-099\n", 9 bytes each = 900 bytes).
            File.WriteAllText(log, string.Concat(
                Enumerable.Range(0, 100).Select(i => $"line-{i:000}\n")));

            // A 103-byte tail starts at byte 797, inside "line-088" ("088\n" would remain),
            // and the cut must drop that partial so the viewer never opens mid-line.
            var tail = DiagnosticsLog.ReadTail(log, maxBytes: 103);
            Assert.StartsWith("line-089\n", tail);
            Assert.EndsWith("line-099\n", tail);
        });
    }

    [Fact]
    public void ReadTail_of_a_missing_file_is_empty()
    {
        RunInTempDir(dir =>
            Assert.Equal(string.Empty, DiagnosticsLog.ReadTail(Path.Combine(dir, "app.log"), 1024)));
    }

    [Fact]
    public void ReadTail_reads_while_a_writer_holds_the_file_open()
    {
        RunInTempDir(dir =>
        {
            var log = Path.Combine(dir, "app.log");
            File.WriteAllText(log, "settled line\n");
            // The logger appends with the file open for writing; the viewer's read must not
            // fail on the shared handle (FileShare.ReadWrite on both sides).
            using var writer = new FileStream(
                log, FileMode.Append, FileAccess.Write, FileShare.ReadWrite);
            Assert.Equal("settled line\n", DiagnosticsLog.ReadTail(log, 1024));
        });
    }

    // --- ReadAllShared (the export payload) ---------------------------------------------------

    [Fact]
    public void ReadAllShared_round_trips_the_current_file_only()
    {
        RunInTempDir(dir =>
        {
            var log = Path.Combine(dir, "app.log");
            File.WriteAllText(log, "current\n");
            // A backup must NOT leak into the export, the live file only, like the
            // Android/Apple share.
            File.WriteAllText($"{log}.1", "backup\n");

            var text = System.Text.Encoding.UTF8.GetString(DiagnosticsLog.ReadAllShared(log));
            Assert.Equal("current\n", text);
        });
    }

    [Fact]
    public void ReadAllShared_of_a_missing_log_is_empty()
    {
        RunInTempDir(dir =>
            Assert.Empty(DiagnosticsLog.ReadAllShared(Path.Combine(dir, "app.log"))));
    }

    // --- Helpers ------------------------------------------------------------------------------

    private static void RunInTempDir(Action<string> test)
    {
        var dir = Path.Combine(Path.GetTempPath(), "mailcal-diag-tests-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        try
        {
            test(dir);
        }
        finally
        {
            try
            {
                Directory.Delete(dir, recursive: true);
            }
            catch
            {
                // best-effort temp cleanup
            }
        }
    }

    private static void RunWithCulture(CultureInfo culture, Action test)
    {
        var previous = CultureInfo.CurrentCulture;
        CultureInfo.CurrentCulture = culture;
        try
        {
            test();
        }
        finally
        {
            CultureInfo.CurrentCulture = previous;
        }
    }
}
