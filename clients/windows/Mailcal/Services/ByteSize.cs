// A file's size in words, as File Explorer gives it: binary units, one decimal, in the app's own
// language. The reading pane's attachments and the composer's files both say it this way, so a
// forwarded file reads the same size in both.
//
// Its own file, WinUI-free, so `Mailcal.Tests` can link it.
using System.Globalization;

namespace Allodia.Mailcal.Services;

internal static class ByteSize
{
    private static readonly string[] Units = ["B", "KB", "MB", "GB", "TB"];

    /// <summary><paramref name="bytes"/> as "532 B", "1.2 KB" or "3.8 MB", with the decimal
    /// separator of the app's culture.</summary>
    internal static string Format(ulong bytes)
    {
        var value = (double)bytes;
        var unit = 0;
        while (value >= 1024 && unit < Units.Length - 1)
        {
            value /= 1024;
            unit++;
        }
        var number = unit == 0
            ? bytes.ToString(CultureInfo.CurrentCulture)
            : value.ToString("0.#", CultureInfo.CurrentCulture);
        return $"{number} {Units[unit]}";
    }
}
