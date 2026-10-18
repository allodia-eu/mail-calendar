// Windows' print dialog, which the system draws outside the app's XAML tree: a top-level window of
// the shell, owned by ours, so it is read through UI Automation, not the agent. The PDF printer's
// Save As is SaveDialogs'.

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
            .Where(frame => Desktop.ById("printerSelector", frame) is not null)
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

    /// <summary>Chooses the PDF printer in <paramref name="dialog"/> and returns its Print button
    /// once it will print.</summary>
    internal static AutomationElement ChoosePdfPrinter(AutomationElement dialog)
    {
        var picker = Desktop.ById("printerSelector", dialog) ?? throw new InvalidOperationException("the print dialog has no printer picker");
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
        return App.Eventually(() => (print = Desktop.ById("PrintButton", dialog)) is { Current.IsEnabled: true }, TimeSpan.FromSeconds(15))
            ? print!
            : throw new InvalidOperationException($"the dialog's Print button never enabled with '{PdfPrinter}' chosen");
    }

    /// <summary>Puts away every dialog a print case may leave: a message box over Save As, Save As,
    /// and the print dialog under them.</summary>
    internal static void CloseAll(int process)
    {
        // The print dialog's buttons refuse a press while a classic dialog is open over it.
        SaveDialogs.CloseAll(process);
        foreach (var dialog in Open())
        {
            if (Desktop.ById("CloseButton", dialog) is { } close)
            {
                Desktop.Press(close);
            }
        }
        if (!App.Eventually(() => Open().Count == 0 && SaveDialogs.Open(process).Count == 0, TimeSpan.FromSeconds(10)))
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

    [DllImport("user32.dll")] private static extern bool IsWindowEnabled(nint hwnd);
}
