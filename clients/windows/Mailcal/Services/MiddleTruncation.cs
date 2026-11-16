// A file name cut in the middle, as Finder and GTK cut it: the start and the end stay, so the
// extension and the part of the name that tells one file from the next (a screenshot's time, a
// scan's number) are both still read. WinUI trims only at the end of a line, so the composer's
// tiles cut the name themselves.
//
// Its own file, WinUI-free, so `Mailcal.Tests` can link it.
using System.Globalization;

namespace Allodia.Mailcal.Services;

internal static class MiddleTruncation
{
    internal const string Ellipsis = "…";

    /// <summary>The number of characters, as a reader counts them, in <paramref name="name"/>.</summary>
    internal static int Length(string name) => new StringInfo(name).LengthInTextElements;

    /// <summary><paramref name="name"/> kept to <paramref name="keep"/> characters around an
    /// ellipsis: the end gets the larger half, and never less than the extension while there is
    /// room for one character before it. The whole name when it is no longer than that.</summary>
    internal static string Cut(string name, int keep)
    {
        var info = new StringInfo(name);
        var length = info.LengthInTextElements;
        if (keep >= length)
        {
            return name;
        }
        if (keep <= 0)
        {
            return Ellipsis;
        }
        var dot = name.LastIndexOf('.');
        var extension = dot > 0 ? new StringInfo(name[dot..]).LengthInTextElements : 0;
        var tail = Math.Max((keep + 1) / 2, Math.Min(extension, keep - 1));
        var head = keep - tail;
        return info.SubstringByTextElements(0, head) + Ellipsis + info.SubstringByTextElements(length - tail, tail);
    }
}
