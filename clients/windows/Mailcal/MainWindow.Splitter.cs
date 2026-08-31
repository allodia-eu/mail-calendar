// The pane splitter: drag the list|reading boundary, the Windows twin of macOS's resizable
// HSplitView. The list column is resized in pixels and the reading column (the lone star) absorbs
// the remainder, so a later window resize grows the reading pane and leaves the user's chosen list
// width intact. The chosen width is persisted (PaneLayoutStore) and restored on the next launch.
// Split out of MainWindow.xaml.cs to keep that file under the 500-line limit.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    private bool _draggingSplitter;
    private double _dragStartX;
    private double _dragStartListWidth;

    private bool _draggingSidebar;
    private double _sidebarDragStartX;
    private double _sidebarDragStartWidth;

    // Reopen the divider where the user last left it: a saved pixel width pins the list column (the
    // reading column stays the star that absorbs the rest). OnMailGridSizeChanged then clamps it to
    // the live window once the grid is laid out, so a saved width that no longer fits, a smaller
    // monitor, a DPI change, can't push the reading pane below its floor.
    private void RestorePaneWidth()
    {
        // A screenshot run keeps the default split: the captured set must not depend on where this
        // particular developer happens to have dragged their divider.
        if (ShowcaseMode.IsOn)
        {
            return;
        }
        if (PaneLayoutStore.Read() is { } listWidth)
        {
            ListColumn.Width = new GridLength(listWidth, GridUnitType.Pixel);
        }
    }

    // Keep the reading pane at or above its MinWidth as the window resizes. Once the list column is
    // a fixed pixel width (the user dragged, or we restored a saved one), shrinking the window would
    // otherwise overflow the fixed list off the right edge. Only touches a pixel-sized list, and
    // only when it's actually out of range, so it never fights a normal star layout or the user.
    private void OnMailGridSizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (ListColumn.Width.GridUnitType != GridUnitType.Pixel)
        {
            return;
        }
        var min = ListColumn.MinWidth;
        var max = MailGrid.ActualWidth - PaneSplitter.ActualWidth - ReadingColumn.MinWidth;
        if (max < min)
        {
            return;
        }
        var clamped = Math.Clamp(ListColumn.Width.Value, min, max);
        if (clamped != ListColumn.Width.Value)
        {
            ListColumn.Width = new GridLength(clamped, GridUnitType.Pixel);
        }
    }

    private void OnSplitterPressed(object sender, PointerRoutedEventArgs e)
    {
        _draggingSplitter = PaneSplitter.CapturePointer(e.Pointer);
        _dragStartX = e.GetCurrentPoint(MailGrid).Position.X;
        // The list column may still be a star length on the first drag; ActualWidth is its
        // rendered size, which we pin as the pixel baseline so the drag tracks the cursor.
        _dragStartListWidth = ListColumn.ActualWidth;
        e.Handled = true;
    }

    private void OnSplitterMoved(object sender, PointerRoutedEventArgs e)
    {
        if (!_draggingSplitter)
        {
            return;
        }
        var x = e.GetCurrentPoint(MailGrid).Position.X;
        var target = _dragStartListWidth + (x - _dragStartX);

        // Keep both panes at or above their MinWidth: the upper bound leaves the reading pane (plus
        // the splitter's own width) its floor; the lower bound is the list's. If the window is too
        // narrow to honour both at once, the floors would cross, stop rather than fight the layout.
        var min = ListColumn.MinWidth;
        var max = MailGrid.ActualWidth - PaneSplitter.ActualWidth - ReadingColumn.MinWidth;
        if (max < min)
        {
            return;
        }
        ListColumn.Width = new GridLength(Math.Clamp(target, min, max), GridUnitType.Pixel);
        e.Handled = true;
    }

    private void OnSplitterReleased(object sender, PointerRoutedEventArgs e)
    {
        if (!_draggingSplitter)
        {
            return;
        }
        PaneSplitter.ReleasePointerCapture(e.Pointer);
        _draggingSplitter = false;
        // Persist the width the user settled on, so the next launch reopens at this split.
        PaneLayoutStore.Write(ListColumn.ActualWidth);
        e.Handled = true;
    }

    private void OnSplitterCaptureLost(object sender, PointerRoutedEventArgs e) =>
        _draggingSplitter = false;

    // Reopen the folder pane at the width the user dragged it to. Same shape as RestorePaneWidth,
    // including the showcase exemption: a captured screenshot must not depend on how wide this
    // particular developer likes their sidebar.
    private void RestoreSidebarWidth()
    {
        if (ShowcaseMode.IsOn)
        {
            return;
        }
        if (PaneLayoutStore.ReadSidebar() is { } width)
        {
            Nav.OpenPaneLength = ClampSidebar(width);
        }
    }

    // The width the pane may take against the live window. The bounds themselves are
    // SidebarWidth's, which is WinUI-free so they can be tested without one.
    private double ClampSidebar(double width) => SidebarWidth.Clamp(width, Nav.ActualWidth);

    // Narrow the pane as the window shrinks, so a width chosen on a large monitor cannot leave the
    // mail list a sliver on a small one. Assigns only when the clamp actually moves it, so it never
    // re-enters through its own SizeChanged, and never fights a width that already fits.
    private void OnNavSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var clamped = ClampSidebar(Nav.OpenPaneLength);
        if (clamped != Nav.OpenPaneLength)
        {
            Nav.OpenPaneLength = clamped;
        }
    }

    private void OnSidebarSplitterPressed(object sender, PointerRoutedEventArgs e)
    {
        _draggingSidebar = SidebarSplitter.CapturePointer(e.Pointer);
        // Measured against the NavigationView, because OpenPaneLength is measured from ITS left
        // edge, tracking the cursor in any other element's space drifts by that element's offset.
        _sidebarDragStartX = e.GetCurrentPoint(Nav).Position.X;
        _sidebarDragStartWidth = Nav.OpenPaneLength;
        e.Handled = true;
    }

    private void OnSidebarSplitterMoved(object sender, PointerRoutedEventArgs e)
    {
        if (!_draggingSidebar)
        {
            return;
        }
        var x = e.GetCurrentPoint(Nav).Position.X;
        Nav.OpenPaneLength = ClampSidebar(_sidebarDragStartWidth + (x - _sidebarDragStartX));
        e.Handled = true;
    }

    private void OnSidebarSplitterReleased(object sender, PointerRoutedEventArgs e)
    {
        if (!_draggingSidebar)
        {
            return;
        }
        SidebarSplitter.ReleasePointerCapture(e.Pointer);
        _draggingSidebar = false;
        PaneLayoutStore.WriteSidebar(Nav.OpenPaneLength);
        e.Handled = true;
    }

    private void OnSidebarSplitterCaptureLost(object sender, PointerRoutedEventArgs e) =>
        _draggingSidebar = false;
}
