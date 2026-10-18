// Windows' print dialog and the PDF printer's Save As, which the system draws outside the app's XAML
// tree: the print dialog is a top-level window of the shell, owned by ours, and Save As a classic
// dialog of the app's own process. So both are read through UI Automation, not the agent.

using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows.Automation;

namespace Allodia.Mailcal.UiTests.Driver;

internal static class PrintDialogs
{
    internal const string PdfPrinter = "Microsoft Print to PDF";

    /// <summary>Every open print dialog, recognised by its printer picker's id rather than by its
    /// title, which is localised.</summary>
    internal static IReadOnlyList<AutomationElement> Open() =>
        AutomationElement.RootElement
            .FindAll(TreeScope.Children, new PropertyCondition(AutomationElement.ClassNameProperty, "ApplicationFrameWindow"))
            .Cast<AutomationElement>()
            .Where(frame => ById("printerSelector", frame) is not null)
            .ToList();

    /// <summary>The print dialog, waited for. A minute, because it first waits on the default
    /// printer, and a network printer that is off holds it on "Waiting for printer connection".</summary>
    internal static AutomationElement Wait()
    {
        AutomationElement? dialog = null;
        return App.Eventually(() => (dialog = Open().FirstOrDefault()) is not null, TimeSpan.FromSeconds(60))
            ? dialog!
            : throw new InvalidOperationException("no system print dialog within 60 s of pressing Print");
    }

    /// <summary>The element under <paramref name="root"/> with AutomationId <paramref name="id"/>, of
    /// <paramref name="type"/> when one is given. The id comes first, quoted, so check-automation-ids
    /// reads it.</summary>
    internal static AutomationElement? ById(string id, AutomationElement root, ControlType? type = null) =>
        root.FindFirst(TreeScope.Descendants, type is null
            ? new PropertyCondition(AutomationElement.AutomationIdProperty, id)
            : new AndCondition(
                new PropertyCondition(AutomationElement.AutomationIdProperty, id),
                new PropertyCondition(AutomationElement.ControlTypeProperty, type)));

    internal static void Press(AutomationElement button) =>
        ((InvokePattern)button.GetCurrentPattern(InvokePattern.Pattern)).Invoke();

    /// <summary>Chooses the PDF printer in <paramref name="dialog"/> and returns its Print button
    /// once it will print.</summary>
    internal static AutomationElement ChoosePdfPrinter(AutomationElement dialog)
    {
        var picker = ById("printerSelector", dialog) ?? throw new InvalidOperationException("the print dialog has no printer picker");
        ((ExpandCollapsePattern)picker.GetCurrentPattern(ExpandCollapsePattern.Pattern)).Expand();
        var named = new AndCondition(
            new PropertyCondition(AutomationElement.ControlTypeProperty, ControlType.ListItem),
            new PropertyCondition(AutomationElement.NameProperty, PdfPrinter));
        AutomationElement? choice = null;
        if (!App.Eventually(() => (choice = dialog.FindFirst(TreeScope.Descendants, named)) is not null, TimeSpan.FromSeconds(10)))
        {
            throw new InvalidOperationException($"the print dialog does not offer '{PdfPrinter}', which this suite prints through");
        }
        ((SelectionItemPattern)choice!.GetCurrentPattern(SelectionItemPattern.Pattern)).Select();
        AutomationElement? print = null;
        return App.Eventually(() => (print = ById("PrintButton", dialog)) is { Current.IsEnabled: true }, TimeSpan.FromSeconds(15))
            ? print!
            : throw new InvalidOperationException($"the dialog's Print button never enabled with '{PdfPrinter}' chosen");
    }

    /// <summary>
    /// The PDF printer's Save As windows: classic dialogs of the app's process. Found through the
    /// window list rather than UI Automation's tree, which files an owned window under its owner, so
    /// a search of the desktop's children never meets it.
    /// </summary>
    internal static IReadOnlyList<AutomationElement> SaveAs(int process)
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
    /// Puts <paramref name="destination"/> in the PDF printer's Save As, saves, and waits until the
    /// file's length has settled: it is created empty and written afterwards. The name goes in
    /// through the field's value, because keys typed as the dialog opens lose their first characters.
    /// </summary>
    internal static void SavePdf(int process, string destination)
    {
        AutomationElement? save = null;
        if (!App.Eventually(() => (save = SaveAs(process).FirstOrDefault()) is not null, TimeSpan.FromSeconds(30)))
        {
            throw new InvalidOperationException("the PDF printer never asked where to save, so nothing was printed");
        }
        var name = ById("1001", save!, ControlType.Edit)
            ?? throw new InvalidOperationException("the PDF printer's Save As has no file name field");
        ((ValuePattern)name.GetCurrentPattern(ValuePattern.Pattern)).SetValue(destination);
        Press(ById("1", save!, ControlType.Button) ?? throw new InvalidOperationException("the PDF printer's Save As has no Save button"));
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
    }

    /// <summary>Puts away every dialog a print case may leave: a message box over Save As, Save As,
    /// and the print dialog under them.</summary>
    internal static void CloseAll(int process)
    {
        // Closing a classic dialog is its Cancel; the topmost goes first, which is why this repeats.
        // The print dialog's buttons refuse a press while one is open over it.
        App.Eventually(() =>
        {
            foreach (var classic in SaveAs(process))
            {
                PostMessageW(classic.Current.NativeWindowHandle, WmClose, 0, 0);
            }
            Thread.Sleep(200);
            return SaveAs(process).Count == 0;
        }, TimeSpan.FromSeconds(10));
        foreach (var dialog in Open())
        {
            if (ById("CloseButton", dialog) is { } close)
            {
                Press(close);
            }
        }
        if (!App.Eventually(() => Open().Count == 0 && SaveAs(process).Count == 0, TimeSpan.FromSeconds(10)))
        {
            throw new InvalidOperationException("a print dialog would not close");
        }
    }

    /// <summary>The /Title the PDF printer wrote: the job's name, and the one thing in the file that
    /// says which message was printed, since the pages are images. A literal string, UTF-16BE behind a
    /// byte-order mark.</summary>
    internal static string? PdfTitle(string path)
    {
        var raw = Encoding.Latin1.GetString(File.ReadAllBytes(path));
        var match = Regex.Match(raw, @"/Title\s*\(((?:\\.|[^\\)])*)\)", RegexOptions.Singleline);
        if (!match.Success)
        {
            return null;
        }
        var bytes = Encoding.Latin1.GetBytes(Regex.Replace(match.Groups[1].Value, @"\\(.)", "$1"));
        return bytes is [0xFE, 0xFF, ..]
            ? Encoding.BigEndianUnicode.GetString(bytes, 2, bytes.Length - 2)
            : Encoding.Latin1.GetString(bytes);
    }

    /// <summary>Whether <paramref name="hwnd"/> takes input: a modal dialog disables its owner.</summary>
    internal static bool Enabled(long hwnd) => IsWindowEnabled((nint)hwnd);

    private const uint WmClose = 0x0010;

    private delegate bool EnumWindowsProc(nint hwnd, nint parameter);

    [DllImport("user32.dll")] private static extern bool PostMessageW(nint hwnd, uint message, nint wParam, nint lParam);

    [DllImport("user32.dll")] private static extern bool IsWindowEnabled(nint hwnd);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(nint hwnd);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindowsProc callback, nint parameter);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(nint hwnd, out int process);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassNameW(nint hwnd, StringBuilder name, int count);
}
