// A panel that lays its children left to right and wraps onto a new line when the next one does not
// fit, what the composer's recipient pills are arranged in.
//
// WinUI ships no flow container, and the alternatives are wrong for pills in the same way SwiftUI's
// were for the Apple client (which grew its own `RecipientFlowLayout` for this): a horizontal
// StackPanel runs a long recipient list off the side of the pane, and a UniformGridLayout gives
// every recipient a column of the same width whether it is `jo@x.eu` or a 40-character address.

using System;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Foundation;

namespace Allodia.Mailcal.Controls;

/// <summary>Arranges its children in rows, wrapping when the next child would overflow the width.</summary>
public sealed class FlowPanel : Panel
{
    /// <summary>The gap between children, horizontally and between wrapped rows.</summary>
    public double Spacing { get; set; } = 6;

    /// <inheritdoc/>
    protected override Size MeasureOverride(Size availableSize)
    {
        // An unconstrained width (inside a horizontally scrolling parent) would wrap nowhere; treat
        // it as one long row rather than dividing by infinity.
        var limit = double.IsInfinity(availableSize.Width) ? double.MaxValue : availableSize.Width;
        double x = 0, y = 0, lineHeight = 0, widest = 0;
        foreach (var child in Children)
        {
            child.Measure(new Size(limit, double.PositiveInfinity));
            var size = child.DesiredSize;
            if (x > 0 && x + size.Width > limit)
            {
                x = 0;
                y += lineHeight + Spacing;
                lineHeight = 0;
            }
            x += size.Width + Spacing;
            widest = Math.Max(widest, x - Spacing);
            lineHeight = Math.Max(lineHeight, size.Height);
        }
        return new Size(widest, y + lineHeight);
    }

    /// <inheritdoc/>
    protected override Size ArrangeOverride(Size finalSize)
    {
        double x = 0, y = 0, lineHeight = 0;
        foreach (var child in Children)
        {
            var size = child.DesiredSize;
            if (x > 0 && x + size.Width > finalSize.Width)
            {
                x = 0;
                y += lineHeight + Spacing;
                lineHeight = 0;
            }
            child.Arrange(new Rect(x, y, size.Width, size.Height));
            x += size.Width + Spacing;
            lineHeight = Math.Max(lineHeight, size.Height);
        }
        return finalSize;
    }
}
