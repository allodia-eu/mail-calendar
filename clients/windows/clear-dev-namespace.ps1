#!/usr/bin/env pwsh
# Empties one dev namespace of the Windows client: its store directory AND its credentials.
#
#   ./clear-dev-namespace.ps1 -Namespace dev-first-run
#
# Both halves, because the client keeps them apart. The engine store and the preference files live
# under %LOCALAPPDATA%\Allodia\MailCalendar\<namespace>, while the account list and every account's
# config live in Credential Manager under `<app id>:<namespace>:` (CredentialStore.UseDevNamespace).
# Deleting only the directory leaves the accounts behind, so the next launch reads them back and
# opens on a mailbox: the first run a test asked for never appears, and the failure names the
# screen it landed on rather than the credentials that put it there.
#
# Only a dev namespace, and only its own prefix. The real accounts sit under `<app id>:account:`
# and `<app id>:account-index`, so the namespace has to be a `dev…` segment and every target is
# checked against the full `<app id>:<namespace>:` prefix before it is deleted.
[CmdletBinding()]
param(
  [Parameter(Mandatory)] [ValidatePattern('^dev(-[a-z]+)*$')] [string] $Namespace
)
$ErrorActionPreference = 'Stop'

if (-not $IsWindows) { throw 'Credential Manager is a Windows store; there is nothing to clear on this host.' }

# The application id this build was branded with, from the file the build generated, rather than
# parsed out of branding/*.env a second time: scripts/dev/brand.sh owns that resolution order.
$brand = Join-Path $PSScriptRoot 'Mailcal/Brand.cs'
if (-not (Test-Path -LiteralPath $brand)) {
  throw "no generated $brand, so the credential prefix is unknown; build the client first (build-and-run.ps1 -NoRun)."
}
$appId = [regex]::Match((Get-Content -Raw -LiteralPath $brand), 'AppId\s*=\s*"([^"]+)"').Groups[1].Value
if (-not $appId) { throw "$brand names no AppId" }
$prefix = "${appId}:${Namespace}:"

if (-not ('MailcalDev.Credentials' -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace MailcalDev
{
    public static class Credentials
    {
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct CREDENTIAL
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
        private static extern bool CredEnumerateW(string filter, uint flags, out uint count, out IntPtr credentials);

        [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern bool CredDeleteW(string target, uint type, uint flags);

        [DllImport("advapi32.dll")]
        private static extern void CredFree(IntPtr buffer);

        // (target, type) of every credential whose target starts with the prefix. ERROR_NOT_FOUND
        // is an empty namespace, which is the common answer rather than a failure.
        public static List<KeyValuePair<string, uint>> Under(string prefix)
        {
            var found = new List<KeyValuePair<string, uint>>();
            if (!CredEnumerateW(prefix + "*", 0, out var count, out var list))
            {
                var error = Marshal.GetLastWin32Error();
                if (error == 1168) { return found; }
                throw new System.ComponentModel.Win32Exception(error);
            }
            try
            {
                for (var i = 0; i < count; i++)
                {
                    var entry = Marshal.ReadIntPtr(list, i * IntPtr.Size);
                    var credential = Marshal.PtrToStructure<CREDENTIAL>(entry);
                    var target = Marshal.PtrToStringUni(credential.TargetName) ?? string.Empty;
                    if (target.StartsWith(prefix, StringComparison.Ordinal))
                    {
                        found.Add(new KeyValuePair<string, uint>(target, credential.Type));
                    }
                }
            }
            finally { CredFree(list); }
            return found;
        }

        public static void Delete(string target, uint type)
        {
            if (!CredDeleteW(target, type, 0))
            {
                var error = Marshal.GetLastWin32Error();
                if (error != 1168) { throw new System.ComponentModel.Win32Exception(error); }
            }
        }
    }
}
'@
}

# The store first: a running app holds its sqlite file open, and a delete that half-succeeds is
# worth a clear refusal rather than a namespace with accounts and no store, or the reverse.
$store = Join-Path $env:LOCALAPPDATA "Allodia\MailCalendar\$Namespace"
Remove-Item -Recurse -Force -LiteralPath $store -ErrorAction SilentlyContinue
if (Test-Path -LiteralPath $store) {
  throw "the store at $store could not be cleared; close any running Mailcal.exe and retry."
}

$removed = 0
foreach ($credential in [MailcalDev.Credentials]::Under($prefix)) {
  # Checked again here, against the prefix as this script spelled it, so a filter the API
  # interpreted more loosely than expected can never reach a target outside the namespace.
  if (-not $credential.Key.StartsWith($prefix, [StringComparison]::Ordinal)) { continue }
  [MailcalDev.Credentials]::Delete($credential.Key, $credential.Value)
  $removed++
}
Write-Host "    cleared $Namespace`: its store, and $removed credential(s) under $prefix" -ForegroundColor DarkGray
