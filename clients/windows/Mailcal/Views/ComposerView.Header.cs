// The composer's header rows (From, To, Cc, Bcc, Subject), one line each, with the labels in one
// column: each row is its own grid, and WinUI has no shared column size, so the column takes the
// widest label's width here. The labels are localised, and "Onderwerp" is not "To".

using System.Linq;
using Allodia.Mailcal.Controls;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    // Before the box applies its template, which reads the overrides once (HeaderField).
    private void StyleHeader() => HeaderField.Attach(SubjectBox, SubjectFocusLine);

    private void InitHeader()
    {
        FromLabel.Text = L10n.ComposeFrom();
        SubjectLabel.Text = L10n.ComposeSubject();
        // The labels are siblings of what they name, not headers of it, so each control is named
        // for a screen reader, and a test reading a TextBox's Name gets the label, not the content.
        AutomationProperties.SetName(FromBox, FromLabel.Text);
        AutomationProperties.SetName(SubjectBox, SubjectLabel.Text);
        AutomationProperties.SetLabeledBy(SubjectBox, SubjectLabel);

        var fields = new[] { ToField, CcField, BccField };
        var width = fields.Select(field => field.LabelWidth).Append(Measured(FromLabel)).Append(Measured(SubjectLabel)).Max();
        foreach (var field in fields)
        {
            field.LabelWidth = width;
        }
        FromLabelColumn.Width = new GridLength(width);
        SubjectLabelColumn.Width = new GridLength(width);
    }

    // The width a label takes, its margin included.
    private static double Measured(TextBlock label)
    {
        label.Measure(new Windows.Foundation.Size(double.PositiveInfinity, double.PositiveInfinity));
        return label.DesiredSize.Width;
    }
}
