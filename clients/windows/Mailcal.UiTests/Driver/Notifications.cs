// The app's notifications as the shell holds them: the registration the app writes at launch, the
// toasts it has posted, and the call the shell makes when one is clicked. The toast is drawn by the
// shell, outside every accessibility tree, so no suite can press it; what the shell does on a press
// is a COM call to the app's activator, INotificationActivationCallback::Activate, with the toast's
// launch arguments, and that call can be made from here.

using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text.Json.Nodes;
using Microsoft.Win32;

namespace Allodia.Mailcal.UiTests.Driver;

internal static class Notifications
{
    /// <summary>
    /// The registration AppNotificationManager.Register() writes for an unpackaged build: the
    /// AUMID key carries CustomActivator, and that CLSID's LocalServer32 names the exe. Matched on
    /// the exe, because every checkout and architecture that ever registered leaves a key of its own.
    /// </summary>
    internal static (string Aumid, Guid Clsid) Registration(string exe)
    {
        using var classes = Registry.CurrentUser.OpenSubKey(@"Software\Classes")
            ?? throw new InvalidOperationException("HKCU has no Software\\Classes");
        using var aumids = classes.OpenSubKey("AppUserModelId");
        var found = new List<(string, Guid)>();
        foreach (var aumid in aumids?.GetSubKeyNames() ?? [])
        {
            using var key = aumids!.OpenSubKey(aumid);
            if (key?.GetValue("CustomActivator") is not string clsid)
            {
                continue;
            }
            using var server = classes.OpenSubKey($@"CLSID\{clsid}\LocalServer32");
            if (server?.GetValue(null) is string command && command.StartsWith($"\"{exe}\"", StringComparison.OrdinalIgnoreCase))
            {
                found.Add((aumid, Guid.Parse(clsid)));
            }
        }
        return found.Count == 1
            ? found[0]
            : throw new InvalidOperationException(
                $"expected one notification registration for {exe}, found {found.Count}; Register() writes it at every launch, so none means the app did not register (app.log: 'notifications: could not register')");
    }

    /// <summary>
    /// The launch arguments of the toast the app posted whose text holds <paramref name="subject"/>,
    /// waited for: read back from the shell's history, so what is clicked is what the app posted.
    /// Through Windows PowerShell, whose WinRT projection reaches ToastNotificationHistory.
    /// </summary>
    internal static string PostedToast(string aumid, string subject, TimeSpan timeout)
    {
        const string read = """
            [void][Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime]
            $toasts = @(foreach ($toast in [Windows.UI.Notifications.ToastNotificationManager]::History.GetHistory($args[0])) {
              $xml = [xml]$toast.Content.GetXml()
              @{ launch = [string]$xml.toast.launch; texts = @($xml.toast.visual.binding.text | ForEach-Object { [string]$_ }) }
            })
            ConvertTo-Json -InputObject $toasts -Depth 4 -Compress
            """;
        string? launch = null;
        App.Eventually(() =>
        {
            var history = WindowsPowerShell(read, aumid);
            if (history.Length == 0 || JsonNode.Parse(history) is not JsonArray toasts)
            {
                return false;
            }
            launch = toasts
                .Where(t => t!["texts"]!.AsArray().Any(text => (string?)text == subject))
                .Select(t => (string?)t!["launch"])
                .FirstOrDefault();
            if (launch is null)
            {
                Thread.Sleep(1000);
            }
            return launch is not null;
        }, timeout);
        return launch ?? throw new InvalidOperationException(
            $"no toast for '{subject}' was posted within {timeout.TotalSeconds} s. Check app.log for 'notifications: posted', the Notifications switch in Settings, and that Windows allows this app's notifications");
    }

    /// <summary>Clicks a toast the way the shell does: creates the app's activator and calls it with
    /// the toast's arguments. A running app answers from the class object it registered; with none
    /// running, COM starts the exe from the activator's LocalServer32 command line.</summary>
    internal static void Activate((string Aumid, Guid Clsid) registration, string arguments)
    {
        var server = (IActivationCallback)Activator.CreateInstance(Type.GetTypeFromCLSID(registration.Clsid, throwOnError: true)!)!;
        try
        {
            server.Activate(registration.Aumid, arguments, 0, 0);
        }
        finally
        {
            Marshal.ReleaseComObject(server);
        }
    }

    /// <summary>The command line <paramref name="process"/> was started with: how it was started.</summary>
    internal static string CommandLine(int process) =>
        WindowsPowerShell("(Get-CimInstance Win32_Process -Filter \"ProcessId=$($args[0])\").CommandLine", process.ToString(System.Globalization.CultureInfo.InvariantCulture));

    private static string WindowsPowerShell(string script, string argument)
    {
        var shell = new ProcessStartInfo("powershell.exe") { UseShellExecute = false, RedirectStandardOutput = true, RedirectStandardError = true };
        foreach (var part in new[] { "-NoProfile", "-NonInteractive", "-Command", $"& {{ {script} }} '{argument.Replace("'", "''")}'" })
        {
            shell.ArgumentList.Add(part);
        }
        using var process = Process.Start(shell) ?? throw new InvalidOperationException("powershell.exe did not start");
        var said = process.StandardOutput.ReadToEndAsync();
        _ = process.StandardError.ReadToEndAsync();
        process.WaitForExit(30_000);
        return said.Result.Trim();
    }

    [ComImport]
    [Guid("53E31837-6600-4A81-9395-75CFFE746F94")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IActivationCallback
    {
        void Activate(
            [MarshalAs(UnmanagedType.LPWStr)] string appUserModelId,
            [MarshalAs(UnmanagedType.LPWStr)] string invokedArgs,
            nint data,
            uint count);
    }
}
