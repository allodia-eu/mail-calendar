// Reading a Windows share: copying what was shared into app-private storage, then asking the
// shared core what it means (docs/os-integration.md).
//
// The staging is the whole reason this exists. A `ShareOperation` hands over `StorageItem`s whose
// access is scoped to the operation and revoked when it reports completion, so a path taken
// straight from one would be unreadable by the time the user pressed Send. The bytes are therefore
// copied first, and only then does the operation get reported complete.
using System;
using System.Collections.Generic;
using System.IO;
using System.Threading.Tasks;
using uniffi.mailcal_bindings;
using Windows.ApplicationModel.DataTransfer;
using Windows.ApplicationModel.DataTransfer.ShareTarget;
using Windows.Storage;

namespace Allodia.Mailcal.Services;

/// <summary>Turning a Windows share into composer prefill.</summary>
internal static class ShareIntake
{
    /// <summary>Where shared bytes are staged, under this app's own local data.</summary>
    private static string StagingDirectory => Path.Combine(AppPaths.Root, "shared-attachments");

    /// <summary>
    /// Reads a share operation and returns what the composer should open with, or <c>null</c> when
    /// it carried nothing usable.
    /// </summary>
    /// <remarks>
    /// <paramref name="operation"/> is reported complete before returning, whatever the outcome:
    /// Windows keeps the sharing app's UI blocked until it is, so a path that forgets leaves the
    /// *other* app looking hung.
    /// </remarks>
    internal static async Task<SharePrefill?> ReadAsync(ShareOperation operation)
    {
        try
        {
            var data = operation.Data;
            var files = data.Contains(StandardDataFormats.StorageItems)
                ? await StageAsync(await data.GetStorageItemsAsync())
                : new List<SharedFile>();
            var text = data.Contains(StandardDataFormats.Text)
                ? await data.GetTextAsync()
                : string.Empty;
            // A shared web link is text as far as a message is concerned; the core turns either
            // into the body, or honours it as a mail link when that is what it is.
            if (text.Length == 0 && data.Contains(StandardDataFormats.WebLink))
            {
                text = (await data.GetWebLinkAsync()).ToString();
            }
            // Everything from here is the core's: the names, the media types, the cap, and which
            // items it will not take. Nothing above this line inspected a file.
            return PrefillFromShare(new ShareRequest(files, text, operation.Data.Properties.Title ?? string.Empty));
        }
        catch (Exception error)
        {
            Log.Warn($"a share could not be read: {error.GetType().Name}");
            return null;
        }
        finally
        {
            operation.ReportCompleted();
        }
    }

    /// <summary>
    /// Copies each shared file into app-private storage and describes it as the core expects.
    /// </summary>
    /// <remarks>
    /// The name and content type are passed on <em>as the sharing app gave them</em>, unsanitised:
    /// sanitising is the core's job, and doing it twice is how two answers appear for one file.
    /// The only cleaning here is of the staged filename, which is this app's own filesystem and
    /// not what a recipient sees.
    /// <para>
    /// One unreadable item never costs the user the rest of the share; a folder is skipped
    /// entirely, since a message attaches files.
    /// </para>
    /// </remarks>
    private static async Task<List<SharedFile>> StageAsync(IReadOnlyList<IStorageItem> items)
    {
        var staged = new List<SharedFile>();
        Directory.CreateDirectory(StagingDirectory);
        PruneStaging();
        foreach (var item in items)
        {
            if (item is not StorageFile file)
            {
                continue;
            }
            try
            {
                var target = Path.Combine(
                    StagingDirectory, $"{Guid.NewGuid():N}-{ShareStaging.StagedName(file.Name)}");
                using (var source = await file.OpenStreamForReadAsync())
                using (var sink = File.Create(target))
                {
                    await source.CopyToAsync(sink);
                }
                staged.Add(new SharedFile(target, file.Name, file.ContentType ?? string.Empty));
            }
            catch (Exception error)
            {
                Log.Warn($"a shared file could not be staged: {error.GetType().Name}");
            }
        }
        return staged;
    }

    /// <summary>How long a staged copy is kept before the next share clears it away.</summary>
    /// <remarks>
    /// Generous, because the only thing it must outlive is a composer someone left open: the
    /// staged path is what Send reads, and pruning underneath one would lose the attachment at the
    /// moment the user finally acted. A week is far longer than any real draft and still bounds
    /// the directory.
    /// </remarks>
    private static readonly TimeSpan StagingRetention = TimeSpan.FromDays(7);

    /// <summary>Deletes staged copies older than <see cref="StagingRetention"/>.</summary>
    /// <remarks>
    /// Every share copies its bytes in here, including the ones the user then cancelled and the
    /// ones the core refused, and this is app data rather than a cache the OS may reclaim: on
    /// Android the equivalent is `cacheDir` and needs no rule. Without this the directory only
    /// ever grows, quietly, in the user's own profile.
    /// <para>
    /// Run at staging time rather than at launch: a share is the only thing that puts files here,
    /// so it is the only moment the directory can have grown, and a launch-time sweep would race
    /// the cold-start share that stages during it.
    /// </para>
    /// </remarks>
    private static void PruneStaging()
    {
        var cutoff = DateTime.UtcNow - StagingRetention;
        try
        {
            foreach (var path in Directory.EnumerateFiles(StagingDirectory))
            {
                try
                {
                    if (File.GetLastWriteTimeUtc(path) < cutoff)
                    {
                        File.Delete(path);
                    }
                }
                catch (Exception)
                {
                    // A file still open, or gone since it was listed. Neither is worth a word:
                    // the next share tries again.
                }
            }
        }
        catch (Exception error)
        {
            Log.Warn($"the share staging directory could not be pruned: {error.GetType().Name}");
        }
    }
}
