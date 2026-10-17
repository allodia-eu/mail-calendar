// The app's own log (docs/logging.md), read for the one thing a launch cannot see on screen: that
// the calendar has been to the server. Rules as in applog.ps1, which says why each one exists.

using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;

namespace Allodia.Mailcal.UiTests.Driver;

internal static partial class AppLog
{
    private static readonly string Path = System.IO.Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Allodia", "MailCalendar", "logs", "app.log");

    [GeneratedRegex(@"(?m)^(?<at>\S+ \S+ \S+) \[info\] --- session start")]
    private static partial Regex Banner();

    /// <summary>
    /// The lines of the session that started at or after <paramref name="since"/>, or "" if none has
    /// yet. The log rotates before every write once it is past its cap, so a running session's
    /// banner can be in app.log.1 while its newest lines are in app.log.
    /// </summary>
    internal static string Session(DateTimeOffset since)
    {
        var text = Read(Path);
        if (!Banner().IsMatch(text))
        {
            text = Read(Path + ".1") + text;
        }
        var banners = Banner().Matches(text);
        if (banners.Count == 0)
        {
            return "";
        }
        var last = banners[^1];
        if (!DateTimeOffset.TryParse(last.Groups["at"].Value, CultureInfo.InvariantCulture, DateTimeStyles.None, out var at)
            || at < since.AddSeconds(-1))
        {
            return "";
        }
        return text[last.Index..];
    }

    /// <summary>One file's text, shared for writing because the app holds it open; "" while it is
    /// missing or mid-rotation.</summary>
    private static string Read(string path)
    {
        try
        {
            using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            using var reader = new StreamReader(stream, Encoding.UTF8);
            return reader.ReadToEnd();
        }
        catch (IOException)
        {
            return "";
        }
    }
}
