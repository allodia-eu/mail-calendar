// Naming a staged copy of a shared file.
//
// Its own file, WinUI- and WinRT-free, so `Mailcal.Tests` can link it: the rest of ShareIntake
// needs a `ShareOperation` and a `StorageFile` and cannot be reached from a plain net10.0
// assembly. The same split DefaultMailApp and MailLink each keep.
using System;
using System.IO;

namespace Allodia.Mailcal.Services;

internal static class ShareStaging
{
    /// <summary>The longest staged name, in characters.</summary>
    /// <remarks>
    /// Short, because the staged path also carries the staging directory and a GUID prefix, and it
    /// is their sum that meets `MAX_PATH`. The attachment's own name is capped separately, and far
    /// more generously, by the shared core, which is the one a recipient reads.
    /// </remarks>
    private const int MaxLength = 80;

    /// <summary>The characters Windows refuses in a file name, written out rather than asked for.</summary>
    /// <remarks>
    /// ⚠️ Not <c>Path.GetInvalidFileNameChars()</c>, which is **host-dependent**: on Linux it
    /// answers `/` and NUL alone, so `:`, `*`, `?` and `\` would all survive. This code only ever
    /// runs on Windows, but `Mailcal.Tests` is a plain net10.0 assembly that the gate runs on
    /// whatever host is to hand, and a rule whose answer changes with the host is one its test
    /// cannot state. Naming the set fixes both: the function is deterministic everywhere, and the
    /// test means the same thing wherever it runs.
    /// </remarks>
    private static readonly char[] Reserved = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

    /// <summary>
    /// A name safe to create on this device, from whatever the sharing app called the file.
    /// </summary>
    /// <remarks>
    /// Not the attachment's name: that comes back from the core, already normalised
    /// (<c>docs/os-integration.md</c>). This one exists only so <c>File.Create</c> succeeds, and
    /// it is deliberately blunt, every reserved character, and every control character, becomes an
    /// underscore.
    /// </remarks>
    internal static string StagedName(string value)
    {
        var cleaned = new string(Array.ConvertAll(
            value.ToCharArray(),
            ch => Array.IndexOf(Reserved, ch) >= 0 || char.IsControl(ch) ? '_' : ch));
        cleaned = cleaned.Trim('.', ' ', '_');
        if (cleaned.Length > MaxLength)
        {
            cleaned = cleaned[..MaxLength];
        }
        return cleaned.Length == 0 ? "shared" : cleaned;
    }
}
