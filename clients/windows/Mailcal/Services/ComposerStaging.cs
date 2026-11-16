// Where the core writes the files a composer opens holding, and when this app deletes them
// (docs/sending.md, "What a forward carries").
//
// Its own file, WinUI-free, so `Mailcal.Tests` can link it.
using System;
using System.IO;
using System.Linq;

namespace Allodia.Mailcal.Services;

/// <summary>The directories a composer's files are staged in: one per composer.</summary>
/// <remarks>
/// A send or a save reads the files after the composer has handed them over, off the thread that
/// handed them, and nothing answers when it has. So only a discard deletes its own at once, and
/// every other composer's waits for the next launch, which runs before any composer can open.
/// <para>
/// Only a directory named as <see cref="NewDirectory"/> names one is ever deleted. That matters
/// for the directories earlier versions staged in, which sit under generic names in the user's
/// temporary directory, where another program may keep something of its own.
/// </para>
/// </remarks>
internal static class ComposerStaging
{
    /// <summary>Where every composer's directory is made, under this app's own local data.</summary>
    internal static string Root => Path.Combine(AppPaths.Root, "composer-files");

    /// <summary>Where earlier versions staged, under the user's temporary directory, which
    /// Windows does not empty on its own.</summary>
    private static readonly string[] EarlierRoots = ["composer-files", "forward-attachments", "resumed-drafts"];

    /// <summary>A directory of one composer's own, so two composers never share a staged file.
    /// The core makes it only when there is a file to write.</summary>
    internal static string NewDirectory() => Path.Combine(Root, Guid.NewGuid().ToString("N"));

    /// <summary>Deletes <paramref name="directory"/>, the one a discarded composer's files were
    /// staged in. Anything that is not a composer directory under <see cref="Root"/> is left
    /// alone.</summary>
    internal static void Release(string? directory)
    {
        if (ComposerDirectoryOf(Root, directory) is { } staged)
        {
            TryDelete(staged);
        }
    }

    /// <summary><paramref name="directory"/> as a composer directory under
    /// <paramref name="root"/>, or <c>null</c> when it is not one.</summary>
    internal static string? ComposerDirectoryOf(string root, string? directory)
    {
        if (string.IsNullOrEmpty(directory))
        {
            return null;
        }
        var full = Path.TrimEndingDirectorySeparator(Path.GetFullPath(root));
        var candidate = Path.TrimEndingDirectorySeparator(Path.GetFullPath(directory));
        return IsComposerDirectory(full, candidate) ? candidate : null;
    }

    /// <summary>Deletes every composer directory made before <paramref name="beforeUtc"/>, in
    /// <see cref="Root"/> and where earlier versions staged, and answers how many went.</summary>
    internal static int Sweep(DateTime beforeUtc) =>
        SweepIn(Root, beforeUtc)
        + EarlierRoots.Sum(name => SweepIn(Path.Combine(Path.GetTempPath(), name), beforeUtc, dropEmptyRoot: true));

    /// <summary>The one-root form of <see cref="Sweep"/>. With <paramref name="dropEmptyRoot"/>,
    /// the root goes too once nothing is left in it.</summary>
    internal static int SweepIn(string root, DateTime beforeUtc, bool dropEmptyRoot = false)
    {
        var deleted = 0;
        try
        {
            var full = Path.TrimEndingDirectorySeparator(Path.GetFullPath(root));
            foreach (var directory in Directory.EnumerateDirectories(full))
            {
                if (IsComposerDirectory(full, directory)
                    && Directory.GetCreationTimeUtc(directory) < beforeUtc
                    && TryDelete(directory))
                {
                    deleted++;
                }
            }
            if (dropEmptyRoot && !Directory.EnumerateFileSystemEntries(full).Any())
            {
                Directory.Delete(full);
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            // No root yet, which is every launch until a composer has staged something.
        }
        return deleted;
    }

    private static bool IsComposerDirectory(string root, string directory) =>
        string.Equals(Path.GetDirectoryName(directory), root, StringComparison.OrdinalIgnoreCase)
        && Guid.TryParseExact(Path.GetFileName(directory), "N", out _);

    private static bool TryDelete(string directory)
    {
        try
        {
            Directory.Delete(directory, recursive: true);
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            // A file another program still has open. The next launch tries again.
            return false;
        }
    }
}
