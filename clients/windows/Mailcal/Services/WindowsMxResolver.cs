// The Windows MX resolver behind the shared detection engine's DNS port. It calls the OS
// resolver (DnsQuery_W in dnsapi.dll) so the device's DNS configuration is honoured, the Rust
// core ships no resolver on purpose. DnsQuery doesn't surface the DNSSEC AD bit, so
// authentic_data is always false here, but that value is not used in any trust decision today
// (DNS-derived configs are trusted on CA-validated TLS); it is reserved for a future opt-in
// "require DNSSEC" setting. The core calls ResolveMx on a background thread, so the synchronous
// P/Invoke is fine.

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Resolves MX records with the Windows DNS API for the autodetect MX fallback.</summary>
internal sealed class WindowsMxResolver : MxResolver
{
    private const ushort DnsTypeMx = 0x000F;
    private const ushort DnsTypeSrv = 0x0021;
    private const int DnsQueryStandard = 0;
    private const int DnsFreeRecordList = 1;
    private const int DnsInfoNoRecords = 9501;
    private const int DnsErrorNameError = 9003;

    /// <inheritdoc />
    public MxResolution ResolveMx(string @domain)
    {
        var status = DnsQuery_W(@domain, DnsTypeMx, DnsQueryStandard, IntPtr.Zero, out var results, IntPtr.Zero);
        // A clean "no MX records" answer is not a failure, return an empty set so the core moves on.
        if (status == DnsInfoNoRecords || status == DnsErrorNameError)
        {
            return new MxResolution(Array.Empty<MxRecord>(), false);
        }
        if (status != 0)
        {
            throw new DnsException.Lookup($"DnsQuery failed for {@domain}: {status}");
        }

        try
        {
            var records = new List<MxRecord>();
            for (var current = results; current != IntPtr.Zero;)
            {
                var header = Marshal.PtrToStructure<DnsRecordHeader>(current);
                if (header.WType == DnsTypeMx)
                {
                    // The MX data (union) follows the fixed header: a name-exchange pointer then a
                    // 16-bit preference.
                    var data = current + Marshal.SizeOf<DnsRecordHeader>();
                    var exchange = Marshal.PtrToStringUni(Marshal.ReadIntPtr(data));
                    var preference = unchecked((ushort)Marshal.ReadInt16(data + IntPtr.Size));
                    if (!string.IsNullOrEmpty(exchange))
                    {
                        records.Add(new MxRecord(preference, exchange!));
                    }
                }
                current = header.PNext;
            }
            // DnsQuery doesn't report the AD bit, so this stays false (never upgrades trust).
            return new MxResolution(records.ToArray(), false);
        }
        finally
        {
            DnsRecordListFree(results, DnsFreeRecordList);
        }
    }

    /// <inheritdoc />
    public SrvResolution ResolveSrv(string @name)
    {
        var status = DnsQuery_W(@name, DnsTypeSrv, DnsQueryStandard, IntPtr.Zero, out var results, IntPtr.Zero);
        // A clean "no SRV records" answer is not a failure, return an empty set so the core moves on.
        if (status == DnsInfoNoRecords || status == DnsErrorNameError)
        {
            return new SrvResolution(Array.Empty<SrvRecord>(), false);
        }
        if (status != 0)
        {
            throw new DnsException.Lookup($"DnsQuery failed for {@name}: {status}");
        }

        try
        {
            var records = new List<SrvRecord>();
            for (var current = results; current != IntPtr.Zero;)
            {
                var header = Marshal.PtrToStructure<DnsRecordHeader>(current);
                if (header.WType == DnsTypeSrv)
                {
                    // DNS_SRV_DATAW follows the header: a name-target pointer, then 16-bit
                    // priority, weight, and port.
                    var data = current + Marshal.SizeOf<DnsRecordHeader>();
                    var target = Marshal.PtrToStringUni(Marshal.ReadIntPtr(data));
                    var priority = unchecked((ushort)Marshal.ReadInt16(data + IntPtr.Size));
                    var weight = unchecked((ushort)Marshal.ReadInt16(data + IntPtr.Size + 2));
                    var port = unchecked((ushort)Marshal.ReadInt16(data + IntPtr.Size + 4));
                    if (!string.IsNullOrEmpty(target))
                    {
                        records.Add(new SrvRecord(priority, weight, port, target!));
                    }
                }
                current = header.PNext;
            }
            // DnsQuery doesn't report the AD bit, so this stays false (never upgrades trust).
            return new SrvResolution(records.ToArray(), false);
        }
        finally
        {
            DnsRecordListFree(results, DnsFreeRecordList);
        }
    }

    // The fixed prefix of a DNS_RECORDW; the per-type data union follows it.
    [StructLayout(LayoutKind.Sequential)]
    private struct DnsRecordHeader
    {
        public IntPtr PNext;
        public IntPtr PName;
        public ushort WType;
        public ushort WDataLength;
        public uint Flags;
        public uint Ttl;
        public uint Reserved;
    }

    [DllImport("dnsapi.dll", CharSet = CharSet.Unicode, SetLastError = false)]
    private static extern int DnsQuery_W(
        string lpstrName,
        ushort wType,
        int options,
        IntPtr pExtra,
        out IntPtr ppQueryResults,
        IntPtr pReserved);

    [DllImport("dnsapi.dll")]
    private static extern void DnsRecordListFree(IntPtr pRecordList, int freeType);
}
