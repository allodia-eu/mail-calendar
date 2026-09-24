// Where a FlowPanel puts each child: left to right, a new line when the next one does not fit, each
// child centred on its line. WinUI-free, so Mailcal.Tests can pin the rules FlowPanel only measures
// and arranges by: the recipient field's input fills what is left of its line rather than taking a
// line of its own, and moves to a fresh line only when what is left is too narrow to type in.

using System;
using System.Collections.Generic;

namespace Allodia.Mailcal.Services;

/// <summary>One child's place, in the panel's own coordinates.</summary>
internal readonly record struct FlowSlot(double X, double Y, double Width, double Height);

/// <summary>A flow of children, and the size it takes.</summary>
internal sealed record FlowResult(IReadOnlyList<FlowSlot> Slots, double Width, double Height);

/// <summary>Lays children out in wrapping lines.</summary>
internal static class FlowLayout
{
    /// <summary>
    /// Places children of the given desired sizes within <paramref name="limit"/>. With
    /// <paramref name="fillLast"/> above zero, the last child takes the rest of its line, and
    /// starts a new one when less than <paramref name="fillLast"/> is left there.
    /// </summary>
    internal static FlowResult Arrange(
        IReadOnlyList<(double Width, double Height)> sizes, double limit, double spacing, double fillLast = 0)
    {
        var slots = new FlowSlot[sizes.Count];
        var lineStart = 0;
        double x = 0, y = 0, lineHeight = 0, widest = 0;
        for (var index = 0; index < sizes.Count; index++)
        {
            var (width, height) = sizes[index];
            var filling = fillLast > 0 && index == sizes.Count - 1;
            var needed = filling ? fillLast : width;
            if (x > 0 && x + needed > limit)
            {
                Centre(slots, lineStart, index, y, lineHeight);
                y += lineHeight + spacing;
                x = 0;
                lineHeight = 0;
                lineStart = index;
            }
            if (filling && !double.IsInfinity(limit))
            {
                width = Math.Max(limit - x, 0);
            }
            slots[index] = new FlowSlot(x, y, width, height);
            x += width + spacing;
            widest = Math.Max(widest, x - spacing);
            lineHeight = Math.Max(lineHeight, height);
        }
        Centre(slots, lineStart, sizes.Count, y, lineHeight);
        return new FlowResult(slots, widest, sizes.Count == 0 ? 0 : y + lineHeight);
    }

    // Children shorter than their line sit in its middle: a pill beside the taller input is level
    // with the text typed into it.
    private static void Centre(FlowSlot[] slots, int from, int to, double top, double lineHeight)
    {
        for (var index = from; index < to; index++)
        {
            var slot = slots[index];
            slots[index] = slot with { Y = top + (lineHeight - slot.Height) / 2 };
        }
    }
}
