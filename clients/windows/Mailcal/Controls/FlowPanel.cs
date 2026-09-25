// A panel that lays its children left to right and wraps onto a new line when the next one does not
// fit: the composer's recipient pills with the address being typed after them, and the reveal's
// chips. Where each child goes is FlowLayout, which Mailcal.Tests pins.
//
// WinUI ships no flow container, and the alternatives are wrong for pills in the same way SwiftUI's
// were for the Apple client (which grew its own `RecipientFlowLayout` for this): a horizontal
// StackPanel runs a long recipient list off the side of the pane, and a UniformGridLayout gives
// every recipient a column of the same width whether it is `jo@x.eu` or a 40-character address.

using System.Collections.Generic;
using System.Linq;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Foundation;

namespace Allodia.Mailcal.Controls;

/// <summary>Arranges its children in rows, wrapping when the next child would overflow the width.</summary>
public sealed class FlowPanel : Panel
{
    /// <summary>The gap between children, horizontally and between wrapped rows.</summary>
    public double Spacing { get; set; } = 6;

    /// <summary>
    /// Above zero, the last child fills the rest of its line, and starts a new line when less than
    /// this is left: the recipient field's input, which must stay wide enough to type into.
    /// </summary>
    public double FillLast { get; set; }

    /// <inheritdoc/>
    protected override Size MeasureOverride(Size availableSize)
    {
        foreach (var child in Children)
        {
            child.Measure(new Size(availableSize.Width, double.PositiveInfinity));
        }
        var flow = Flow(availableSize.Width);
        // A filling child is re-measured at the width it will get, so a TextBox reports the height
        // it has at that width rather than at the panel's.
        if (FillLast > 0 && Children.Count > 0)
        {
            var last = flow.Slots[^1];
            Children[^1].Measure(new Size(last.Width, double.PositiveInfinity));
        }
        var width = FillLast > 0 && !double.IsInfinity(availableSize.Width) ? availableSize.Width : flow.Width;
        return new Size(width, flow.Height);
    }

    /// <inheritdoc/>
    protected override Size ArrangeOverride(Size finalSize)
    {
        var flow = Flow(finalSize.Width);
        for (var index = 0; index < Children.Count; index++)
        {
            var slot = flow.Slots[index];
            Children[index].Arrange(new Rect(slot.X, slot.Y, slot.Width, slot.Height));
        }
        return finalSize;
    }

    private FlowResult Flow(double width)
    {
        List<(double, double)> sizes = Children.Select(child => (child.DesiredSize.Width, child.DesiredSize.Height)).ToList();
        return FlowLayout.Arrange(sizes, width, Spacing, FillLast);
    }
}
