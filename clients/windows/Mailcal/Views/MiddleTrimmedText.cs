// One line of text cut in the middle when it does not fit (Services/MiddleTruncation.cs), which
// WinUI's own TextTrimming cannot do: it trims only at the end.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Windows.Foundation;

namespace Allodia.Mailcal.Views;

/// <summary>Shows as much of a name as its width holds, around an ellipsis. Assistive technology
/// reads the whole name from whatever names the control around it, never the cut one.</summary>
internal sealed partial class MiddleTrimmedText : Panel
{
    private readonly string _full;
    private readonly TextBlock _shown;

    /// <summary>Off the tree, so trying a length never lays out the line on screen.</summary>
    private readonly TextBlock _probe;

    private double _fittedTo = double.NaN;

    internal MiddleTrimmedText(string text)
    {
        _full = text;
        _shown = new TextBlock { Text = text, TextWrapping = TextWrapping.NoWrap };
        AutomationProperties.SetAccessibilityView(_shown, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        _probe = new TextBlock
        {
            TextWrapping = TextWrapping.NoWrap,
            FontSize = _shown.FontSize,
            FontFamily = _shown.FontFamily,
            FontWeight = _shown.FontWeight,
        };
        Children.Add(_shown);
    }

    protected override Size MeasureOverride(Size availableSize)
    {
        var width = availableSize.Width;
        if (width != _fittedTo)
        {
            _fittedTo = width;
            _shown.Text = double.IsInfinity(width) ? _full : Fit(width);
        }
        _shown.Measure(availableSize);
        return new Size(Math.Min(_shown.DesiredSize.Width, width), _shown.DesiredSize.Height);
    }

    protected override Size ArrangeOverride(Size finalSize)
    {
        _shown.Arrange(new Rect(0, 0, finalSize.Width, finalSize.Height));
        return finalSize;
    }

    /// <summary>The longest cut of the name that fits <paramref name="width"/>: the whole name
    /// when it fits, else a search over how many characters to keep.</summary>
    private string Fit(double width)
    {
        if (Measured(_full) <= width)
        {
            return _full;
        }
        var (fits, fails) = (0, MiddleTruncation.Length(_full));
        while (fails - fits > 1)
        {
            var keep = (fits + fails) / 2;
            if (Measured(MiddleTruncation.Cut(_full, keep)) <= width)
            {
                fits = keep;
            }
            else
            {
                fails = keep;
            }
        }
        return MiddleTruncation.Cut(_full, fits);
    }

    private double Measured(string text)
    {
        _probe.Text = text;
        _probe.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        return _probe.DesiredSize.Width;
    }
}
