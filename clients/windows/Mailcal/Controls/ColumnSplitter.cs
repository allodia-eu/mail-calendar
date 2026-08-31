// A thin vertical grip between the message list and the reading pane that drags to resize them,
// the Windows twin of the macOS HSplitView divider. The control itself only supplies the
// west-east resize cursor: WinUI 3 exposes a UIElement's cursor solely through the protected
// ProtectedCursor, so it must be set from a subclass. MainWindow wires the pointer drag that
// actually moves the column boundary (it owns the Grid columns and their MinWidth floors).
//
// We derive from Grid rather than ContentControl so the visible 1px line is just a child element
// and there's no control template to supply; a Transparent Background makes the full width
// hit-testable so the grip is easy to grab even though the line itself is hairline-thin.

using Microsoft.UI.Input;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Controls;

/// <summary>A drag-to-resize divider placed between two Grid columns.</summary>
public sealed class ColumnSplitter : Grid
{
    public ColumnSplitter()
    {
        ProtectedCursor = InputSystemCursor.Create(InputSystemCursorShape.SizeWestEast);
    }
}
