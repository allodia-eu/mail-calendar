// The accounts a dev namespace keeps in Credential Manager, as the app's CredentialStore lays them
// out: an index credential holding the ordered ids as JSON, and one credential per account holding
// its config. Only what a suite needs to stage a stored account the core cannot read, and only in a
// `dev` namespace, so the developer's own accounts are out of its reach.

using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace Allodia.Mailcal.UiTests.Driver;

internal static class StoredAccounts
{
    private const uint CredTypeGeneric = 1;
    private const uint CredPersistLocalMachine = 2;

    /// <summary>Adds <paramref name="id"/> holding <paramref name="config"/> to the end of
    /// <paramref name="space"/>'s index.</summary>
    internal static void Plant(string space, string id, string config)
    {
        var ids = Index(space);
        Write(Target(space, $"account:{id}"), config);
        if (!ids.Contains(id))
        {
            ids.Add(id);
            Write(Target(space, "account-index"), JsonSerializer.Serialize(ids));
        }
    }

    /// <summary>Takes <paramref name="id"/> out of <paramref name="space"/> again. Safe when it is
    /// not there.</summary>
    internal static void Remove(string space, string id)
    {
        CredDeleteW(Target(space, $"account:{id}"), CredTypeGeneric, 0);
        var ids = Index(space);
        if (ids.Remove(id))
        {
            Write(Target(space, "account-index"), JsonSerializer.Serialize(ids));
        }
    }

    private static string Target(string space, string name)
    {
        if (!space.StartsWith("dev", StringComparison.Ordinal))
        {
            throw new ArgumentException($"only a dev namespace is staged, not {space}", nameof(space));
        }
        return $"{Brand.AppId}:{space}:{name}";
    }

    private static List<string> Index(string space) =>
        Read(Target(space, "account-index")) is { } json
            ? JsonSerializer.Deserialize<List<string>>(json) ?? []
            : [];

    private static string? Read(string target)
    {
        if (!CredReadW(target, CredTypeGeneric, 0, out var handle))
        {
            return null;
        }
        try
        {
            var credential = Marshal.PtrToStructure<Credential>(handle);
            var bytes = new byte[credential.CredentialBlobSize];
            Marshal.Copy(credential.CredentialBlob, bytes, 0, bytes.Length);
            return Encoding.UTF8.GetString(bytes);
        }
        finally
        {
            CredFree(handle);
        }
    }

    private static void Write(string target, string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        var blob = Marshal.AllocHGlobal(bytes.Length);
        var name = Marshal.StringToHGlobalUni(target);
        var user = Marshal.StringToHGlobalUni("ui-suite");
        try
        {
            Marshal.Copy(bytes, 0, blob, bytes.Length);
            var credential = new Credential
            {
                Type = CredTypeGeneric,
                TargetName = name,
                CredentialBlobSize = (uint)bytes.Length,
                CredentialBlob = blob,
                Persist = CredPersistLocalMachine,
                UserName = user,
            };
            if (!CredWriteW(ref credential, 0))
            {
                throw new InvalidOperationException($"CredWrite failed for {target} ({Marshal.GetLastWin32Error()})");
            }
        }
        finally
        {
            Marshal.FreeHGlobal(blob);
            Marshal.FreeHGlobal(name);
            Marshal.FreeHGlobal(user);
        }
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct Credential
    {
        public uint Flags;
        public uint Type;
        public IntPtr TargetName;
        public IntPtr Comment;
        public long LastWritten;
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
    private static extern bool CredWriteW(ref Credential credential, uint flags);

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredDeleteW(string target, uint type, uint flags);

    [DllImport("advapi32.dll")]
    private static extern void CredFree(IntPtr buffer);
}
