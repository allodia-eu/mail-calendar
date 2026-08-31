// Where this device remembers what it has synced with the Allodia account service. Its Apple,
// Android and Linux twins keep the same blob in each platform's own ordinary preferences.
//
// A preference file and not the Credential Manager: nothing in the blob is secret (a record id, a
// version, a fingerprint, a flag), and there is nothing to protect that a credential prompt would
// buy. It lives in AppPaths.PrefsDir beside the language and window-placement files, so a harness
// run's bookkeeping is isolated with the rest of that run's state.
//
// Unlike the other preference stores here, a failed write is REPORTED rather than swallowed: by the
// time the core calls this it has already written to the service, and a note that never landed is a
// record this device will offer itself back at the next pass.

using System;
using System.IO;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The sync bookkeeping, in one file, written whole.</summary>
internal sealed class FileSyncStateStore : SyncStateStore
{
    private static string FilePath => Path.Combine(AppPaths.PrefsDir, "allodia-sync.json");

    /// <summary>The blob last written, or <c>null</c> if this device has never synced.</summary>
    public string? Load()
    {
        try
        {
            return File.Exists(FilePath) ? File.ReadAllText(FilePath) : null;
        }
        catch (Exception e)
        {
            // Not "never synced": that would start a pass which re-adopts every record.
            throw new SyncStateException.Store(e.Message);
        }
    }

    /// <summary>Replaces the blob, whole.</summary>
    public void Save(string blob)
    {
        try
        {
            Directory.CreateDirectory(AppPaths.PrefsDir);
            File.WriteAllText(FilePath, blob);
        }
        catch (Exception e)
        {
            throw new SyncStateException.Store(e.Message);
        }
    }
}
