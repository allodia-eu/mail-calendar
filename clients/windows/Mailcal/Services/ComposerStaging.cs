// Where the core writes the files a composer opens holding, and when this app deletes them
// (docs/sending.md, "What a forward carries").
//
// Its own file, WinUI-free, so `Mailcal.Tests` can link it.
using System;
using System.Collections.Generic;
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

    /// <summary>Deletes the directories <paramref name="staged"/> were written to: what a
    /// discarded composer opened holding.</summary>
    internal static void Release(IEnumerable<string> staged)
    {
        foreach (var directory in DirectoriesOf(Root, staged))
        {
            TryDelete(directory);
        }
    }

    /// <summary>The composer directories under <paramref name="root"/> that hold
    /// <paramref name="staged"/>; a path anywhere else, such as a share's copy, names none.</summary>
    internal static IReadOnlyList<string> DirectoriesOf(string root, IEnumerable<string> staged)
    {
        var full = Path.TrimEndingDirectorySeparator(Path.GetFullPath(root));
        return staged
            .Select(path => Path.GetDirectoryName(Path.GetFullPath(path)))
            .OfType<string>()
            .Where(directory => IsComposerDirectory(full, directory))
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToList();
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
