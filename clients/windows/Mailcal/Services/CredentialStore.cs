// Secure credential storage for Windows: every account's config (TOML, the password for IMAP,
// the refresh token for Microsoft 365) is kept in the Windows Credential Manager (the OS secure
// store), the Windows counterpart of macOS's Keychain (KeychainHelper.swift) and Android's
// EncryptedSharedPreferences (SecureStore.kt).
//
// Layout: ONE credential per account (target "eu.allodia.mailcal:account:<id>"), plus a small
// index credential ("…:account-index") holding the ordered ids, so the switcher keeps add-order
// and an account can be added, replaced, or (later) removed on its own. This is the foundation
// for per-account removal in the UI.
//
// All three clients now use this per-account layout (macOS's Keychain, KeychainHelper.swift,
// and Android's EncryptedSharedPreferences, SecureStore.kt, each keep one entry per account
// under an ordered index too). Windows is the only store with a hard per-entry size cap, so it
// alone self-chunks a large entry (see below); the other two store each config in one item.
//
// Windows caps one credential's blob at CRED_MAX_CREDENTIAL_BLOB_SIZE (2560 bytes), and a single
// Microsoft refresh-token config can exceed that (tokens vary; CAE/claims/many-scopes push them
// to 2–4 KB). So each entry is itself split across chunk credentials when needed. The advapi32
// Cred* glue stays isolated here.

using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace Allodia.Mailcal.Services;

/// <summary>Reads/writes the configured accounts in the Windows Credential Manager, one credential
/// per account under an ordered index.</summary>
internal static class CredentialStore
{
    // The production namespace, under which the developer's real accounts live. A debug harness run
    // swaps the active prefix once at startup (UseDevNamespace) so its form-adds, removes, and token
    // rotations land in an isolated store instead of co-mingling with, or reordering the index of,
    // the real accounts. Every target is derived from _prefix, so the whole store moves together.
    private const string ProdPrefix = Brand.AppId + ":";
    private static string _prefix = ProdPrefix;

    // The ordered list of account ids; the per-account configs live under AccountTarget(id).
    private static string IndexTarget => _prefix + "account-index";
    // Shown as the credential's user field in the Windows Credential Manager UI, so it is the
    // name the app calls itself by rather than a second copy of it (docs/branding.md).
    private static readonly string UserLabel = L10n.AppTitle();

    private const uint CRED_TYPE_GENERIC = 1;
    private const uint CRED_PERSIST_LOCAL_MACHINE = 2;
    // Comfortably under the 2560-byte CRED_MAX_CREDENTIAL_BLOB_SIZE, so a chunk write never trips
    // the cap (the very failure that used to silently drop Microsoft accounts).
    private const int MaxChunkBytes = 2048;

    private static string AccountTarget(string id) => $"{_prefix}account:{id}";

    /// <summary>
    /// Switches the store to an isolated, throwaway <em>dev</em> namespace for the rest of the
    /// process, keyed to the harness account's store subdir, so a <c>stalwart</c> (JMAP) run and a
    /// <c>stalwart-imap</c> run keep separate credentials, just as they keep separate engine stores.
    /// Call once at startup, before any read/write. This is what keeps an account added through the
    /// setup form during a harness run from ever touching the developer's real accounts. Debug-only
    /// in practice, the only caller is under <c>#if DEBUG</c>.
    /// </summary>
    public static void UseDevNamespace(string tag) => _prefix = $"{ProdPrefix}{tag}:";

    /// <summary>
    /// Every stored account's config TOML, in the order they were added (empty on first run). The
    /// host passes these to MailcalApp.NewAccounts, which re-derives each id. An indexed id whose
    /// credential is missing is skipped rather than failing the whole launch.
    /// </summary>
    public static string[] Configs()
    {
        return ReadIndex()
            .Select(id => ReadChunked(AccountTarget(id)))
            .Where(config => config is not null)
            .Select(config => config!)
            .ToArray();
    }

    /// <summary>
    /// Stores <paramref name="config"/> for account <paramref name="id"/> in its own credential,
    /// replacing that account's entry (a reconnect / rotated token) and appending the id to the
    /// ordered index on first add, so the switcher stays stable. Returns whether the config and
    /// the index both landed.
    ///
    /// It used to return nothing, which was defensible only while the caller had nothing to do
    /// with the answer. The core now decides what a refused write means, rolling an add back, or
    /// reporting a rotation it cannot recover from, so this has to be able to say no.
    /// </summary>
    public static bool Save(string id, string config)
    {
        if (!WriteChunked(AccountTarget(id), config))
        {
            return false; // WriteChunked logged the failure; don't index an unpersisted account.
        }
        var ids = ReadIndex();
        if (ids.Contains(id))
        {
            return true;
        }
        ids.Add(id);
        return WriteChunked(IndexTarget, JsonSerializer.Serialize(ids));
    }

    /// <summary>
    /// Removes account <paramref name="id"/>: deletes its credential (every chunk) and drops its
    /// id from the ordered index, so a later launch no longer loads it. Returns whether nothing is
    /// stored for <paramref name="id"/> any more, an id that was not there is a success, since
    /// that is already the desired end state. The account's runtime removal is the core's job
    /// (<c>MailcalApp.RemoveAccount</c>), which calls this itself.
    /// </summary>
    public static bool Remove(string id)
    {
        // Delete chunk 0, then any overflow chunks, stopping at the first that doesn't exist.
        for (var chunk = 0; CredDeleteW(ChunkTarget(AccountTarget(id), chunk), CRED_TYPE_GENERIC, 0); chunk++)
        {
        }
        var ids = ReadIndex();
        return !ids.Remove(id) || WriteChunked(IndexTarget, JsonSerializer.Serialize(ids));
    }

    private static List<string> ReadIndex()
    {
        var json = ReadChunked(IndexTarget);
        if (string.IsNullOrEmpty(json))
        {
            return new List<string>();
        }
        try
        {
            return JsonSerializer.Deserialize<List<string>>(json) ?? new List<string>();
        }
        catch (JsonException)
        {
            // A corrupt index reads as no accounts (first-run setup) rather than crashing.
            return new List<string>();
        }
    }

    // --- Chunked value read/write over a base target (chunk 0 = base, then ":1", ":2", …) -------
    // A value larger than one credential's cap is split across successive credentials and
    // reassembled on read; the bytes are joined before UTF-8 decode, so a multi-byte character
    // straddling a chunk boundary still decodes.

    private static string ChunkTarget(string baseTarget, int chunk) =>
        chunk == 0 ? baseTarget : $"{baseTarget}:{chunk}";

    private static string? ReadChunked(string baseTarget)
    {
        var all = new List<byte>();
        for (var chunk = 0; ; chunk++)
        {
            var bytes = ReadChunkBytes(ChunkTarget(baseTarget, chunk));
            if (bytes is null)
            {
                break;
            }
            all.AddRange(bytes);
        }
        return all.Count == 0 ? null : Encoding.UTF8.GetString(all.ToArray());
    }

    // Writes value across as many chunks as needed, then drops any stale higher-index chunks from
    // a previously larger value. Each CredWrite is checked, a swallowed failure here is exactly
    // what used to lose Microsoft accounts, so a failure is logged loudly.
    private static bool WriteChunked(string baseTarget, string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        var chunkCount = System.Math.Max(1, (bytes.Length + MaxChunkBytes - 1) / MaxChunkBytes);
        for (var chunk = 0; chunk < chunkCount; chunk++)
        {
            var offset = chunk * MaxChunkBytes;
            var length = System.Math.Min(MaxChunkBytes, bytes.Length - offset);
            if (!WriteChunkBytes(ChunkTarget(baseTarget, chunk), bytes, offset, length))
            {
                Log.Error($"credential store: CredWrite failed for '{baseTarget}' chunk {chunk} of " +
                          $"{chunkCount} (err {Marshal.GetLastWin32Error()}); not persisted");
                return false;
            }
        }
        for (var chunk = chunkCount; CredDeleteW(ChunkTarget(baseTarget, chunk), CRED_TYPE_GENERIC, 0); chunk++)
        {
        }
        return true;
    }

    private static byte[]? ReadChunkBytes(string target)
    {
        if (!CredReadW(target, CRED_TYPE_GENERIC, 0, out var handle))
        {
            return null;
        }
        try
        {
            var cred = Marshal.PtrToStructure<CREDENTIAL>(handle);
            if (cred.CredentialBlobSize == 0 || cred.CredentialBlob == IntPtr.Zero)
            {
                return null;
            }
            var bytes = new byte[cred.CredentialBlobSize];
            Marshal.Copy(cred.CredentialBlob, bytes, 0, (int)cred.CredentialBlobSize);
            return bytes;
        }
        finally
        {
            CredFree(handle);
        }
    }

    private static bool WriteChunkBytes(string target, byte[] bytes, int offset, int length)
    {
        var blobPtr = Marshal.AllocHGlobal(length);
        var targetPtr = Marshal.StringToHGlobalUni(target);
        var userPtr = Marshal.StringToHGlobalUni(UserLabel);
        try
        {
            Marshal.Copy(bytes, offset, blobPtr, length);
            var cred = new CREDENTIAL
            {
                Type = CRED_TYPE_GENERIC,
                TargetName = targetPtr,
                CredentialBlobSize = (uint)length,
                CredentialBlob = blobPtr,
                Persist = CRED_PERSIST_LOCAL_MACHINE,
                UserName = userPtr,
            };
            return CredWriteW(ref cred, 0);
        }
        finally
        {
            Marshal.FreeHGlobal(blobPtr);
            Marshal.FreeHGlobal(targetPtr);
            Marshal.FreeHGlobal(userPtr);
        }
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct CREDENTIAL
    {
        public uint Flags;
        public uint Type;
        public IntPtr TargetName;
        public IntPtr Comment;
        public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
        public uint CredentialBlobSize;
        public IntPtr CredentialBlob;
        public uint Persist;
        public uint AttributeCount;
        public IntPtr Attributes;
        public IntPtr TargetAlias;
        public IntPtr UserName;
    }

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredReadW(string target, uint type, uint flags, out IntPtr credential);

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredWriteW(ref CREDENTIAL credential, uint flags);

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredDeleteW(string target, uint type, uint flags);

    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern void CredFree(IntPtr buffer);
}
