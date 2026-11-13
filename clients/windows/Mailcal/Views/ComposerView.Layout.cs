// How tall the message is, where the column scrolls to keep the caret in view, and the wheel over
// the message (docs/composer-layout.md, rules 2, 5 and 6). The core decides the first two; this
// measures and applies.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    /// <summary>The height the editor's page last said its content needs, in CSS pixels, which
    /// are this toolkit's device-independent pixels. Nothing until the page first reports.</summary>
    private double _contentHeight;

    /// <summary>
    /// Hands the wheel turned over the message to the column (docs/composer-layout.md, rule 2).
    /// </summary>
    /// <remarks>
    /// The WebView2 passes the wheel on to its page and marks the event handled, so the
    /// ScrollViewer around it never scrolls; in growing mode the page has nothing to scroll
    /// either, and the wheel over most of the composer would move nothing. The event still bubbles,
    /// so a handler that takes handled events sees it. Vertical only: a wide table in the message
    /// keeps its own horizontal scroll.
    /// </remarks>
    private void ListenForWheel() =>
        EditorFrame.AddHandler(PointerWheelChangedEvent, new PointerEventHandler(OnEditorWheel), handledEventsToo: true);

    private void OnEditorWheel(object sender, PointerRoutedEventArgs e)
    {
        var wheel = e.GetCurrentPoint(ComposerScroll).Properties;
        if (!wheel.IsHorizontalMouseWheel)
        {
            ComposerScroll.ChangeView(null, ComposerScroll.VerticalOffset - wheel.MouseWheelDelta, null);
        }
    }

    private void OnColumnSizeChanged(object sender, SizeChangedEventArgs e) => SizeEditor();

    private void OnAboveEditorSizeChanged(object sender, SizeChangedEventArgs e) => SizeEditor();

    /// <summary>The page's <c>height</c> notice.</summary>
    private void OnEditorContentHeight(uint px)
    {
        _contentHeight = px;
        SizeEditor();
    }

    /// <summary>The page's <c>caret</c> notice: scrolls the column by what the core answers, and
    /// leaves it where it is when the caret is already in view.</summary>
    private void OnEditorCaret(uint top, uint bottom)
    {
        var page = EditorTop;
        if (MailcalBindingsMethods.ComposerCaretScroll(
                ComposerScroll.VerticalOffset, ComposerScroll.ViewportHeight, page + top, page + bottom) is { } offset)
        {
            ComposerScroll.ChangeView(null, offset, null, disableAnimation: true);
        }
    }

    /// <summary>Where the editor's page starts in the column: under everything above the message,
    /// the gap between them, and the frame's border.</summary>
    private double EditorTop => AboveEditor.ActualHeight + Column.Spacing + EditorFrame.BorderThickness.Top;

    /// <summary>Gives the editor what its content needs or what the column's visible part leaves
    /// under the header and the files, whichever is more. Rounded down, so a message that fits
    /// leaves the column nothing to scroll.</summary>
    private void SizeEditor()
    {
        var room = Math.Floor(ComposerScroll.ViewportHeight - EditorTop - EditorFrame.BorderThickness.Bottom);
        Editor.Height = MailcalBindingsMethods.ComposerEditorHeight(_contentHeight, room);
    }
}
