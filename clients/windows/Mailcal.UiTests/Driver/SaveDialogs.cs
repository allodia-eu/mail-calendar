// The classic Save As, which the system draws for a FileSavePicker (the .eml export) and for the PDF
// printer: a #32770 dialog of the app's own process, outside the XAML tree, so it is read through UI
// Automation. It is found through the window list rather than UI Automation's tree, which files an
// owned window under its owner, so a search of the desktop's children never meets it.

using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Automation;

namespace Allodia.Mailcal.UiTests.Driver;

internal static class SaveDialogs
{
    /// <summary>Every classic dialog <paramref name="process"/> has open.</summary>
    internal static IReadOnlyList<AutomationElement> Open(int process)
    {
        var found = new List<AutomationElement>();
        var name = new StringBuilder(16);
        EnumWindows((hwnd, _) =>
        {
            GetWindowThreadProcessId(hwnd, out var owner);
            name.Clear();
            if (owner == process && IsWindowVisible(hwnd) && GetClassNameW(hwnd, name, name.Capacity) > 0 && name.ToString() == "#32770")
            {
                found.Add(AutomationElement.FromHandle(hwnd));
            }
            return true;
        }, 0);
        return found;
    }

    /// <summary>
    /// Answers the Save As <paramref name="process"/> raises with <paramref name="destination"/>,
    /// and waits until the file's length has settled: it is created empty and written afterwards.
    /// Returns the name the dialog came up with, which is the only evidence of what the app asked
    /// for the file to be called.
    /// </summary>
    /// <remarks>
    /// The name box is #1001 under #FileNameControlHost; the address band carries a #1001 of its own,
    /// which a search of the whole dialog meets first. The path goes in through the box's value,
    /// because keys typed as the dialog opens lose their first characters.
    /// </remarks>
    internal static string Save(int process, string destination)
    {
        AutomationElement? dialog = null;
        if (!App.Eventually(() => (dialog = Open(process).FirstOrDefault()) is not null, TimeSpan.FromSeconds(30)))
        {
            throw new InvalidOperationException("no Save As came up within 30 s");
        }
        var host = Desktop.ById("FileNameControlHost", dialog!) ?? throw new InvalidOperationException("the Save As has no file name field");
        var box = Desktop.ById("1001", host, ControlType.Edit) ?? throw new InvalidOperationException("the Save As has no file name box");
        var value = (ValuePattern)box.GetCurrentPattern(ValuePattern.Pattern);
        var suggested = value.Current.Value;
        value.SetValue(destination);
        Desktop.Press(Desktop.ById("1", dialog!, ControlType.Button) ?? throw new InvalidOperationException("the Save As has no Save button"));
        long previous = -1;
        var written = App.Eventually(() =>
        {
            Thread.Sleep(250);
            var length = File.Exists(destination) ? new FileInfo(destination).Length : -1;
            var settled = length > 0 && length == previous;
            previous = length;
            return settled;
        }, TimeSpan.FromSeconds(30));
        if (!written)
        {
            throw new InvalidOperationException($"nothing was written at {destination}");
        }
        return suggested;
    }

    /// <summary>Closes every classic dialog <paramref name="process"/> has open, a message box over a
    /// Save As included. Closing one is its Cancel; the topmost goes first, which is why this repeats.</summary>
    internal static void CloseAll(int process)
    {
        var closed = App.Eventually(() =>
        {
            foreach (var classic in Open(process))
            {
                PostMessageW(classic.Current.NativeWindowHandle, WmClose, 0, 0);
            }
            Thread.Sleep(200);
            return Open(process).Count == 0;
        }, TimeSpan.FromSeconds(10));
        if (!closed)
        {
            throw new InvalidOperationException("a Save As would not close");
        }
    }

    private const uint WmClose = 0x0010;

    private delegate bool EnumWindowsProc(nint hwnd, nint parameter);

    [DllImport("user32.dll")] private static extern bool PostMessageW(nint hwnd, uint message, nint wParam, nint lParam);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(nint hwnd);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindowsProc callback, nint parameter);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(nint hwnd, out int process);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassNameW(nint hwnd, StringBuilder name, int count);
}
