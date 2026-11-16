// How tall the message is, where the column scrolls to keep the caret in view, where the editor
// keeps its toolbar, and the wheel over the message (docs/composer-layout.md, rules 2, 5, 6 and 7).
// The core decides the height and the scroll; this measures and applies.

using System.Globalization;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    /// <summary>The height the editor's page last said its content needs, in CSS pixels, which
    /// are this toolkit's device-independent pixels. Nothing until the page first reports.</summary>
    private double _contentHeight;

    /// <summary>The caret the page last reported and the column has not yet brought into view.
    /// Kept while it cannot be reached: the page reports its new height and then its caret in one
    /// frame, so a line typed past the end arrives before the column has grown to hold it, and a
    /// scroll asked for then stops at the old end.</summary>
    private (uint Top, uint Bottom)? _caret;

    /// <summary>The visible top last told to the page, and whether a call telling it is under
    /// way: the column reports every frame of a scroll, and only the latest value is worth
    /// sending.</summary>
    private double _visibleTopSent = double.NaN;
    private bool _visibleTopSending;

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

    private void OnColumnSizeChanged(object sender, SizeChangedEventArgs e)
    {
        SizeEditor();
        ShowEdges();
    }

    private void OnAboveEditorSizeChanged(object sender, SizeChangedEventArgs e) => SizeEditor();

    /// <summary>The column's content after a layout pass: the editor has taken the height it was
    /// given, so a caret that was out of reach may now be reached.</summary>
    private void OnColumnContentSizeChanged(object sender, SizeChangedEventArgs e)
    {
        FollowCaret();
        ShowEdges();
        TellVisibleTop();
    }

    private void OnColumnViewChanged(object? sender, ScrollViewerViewChangedEventArgs e)
    {
        ShowEdges();
        TellVisibleTop();
    }

    /// <summary>The page's <c>height</c> notice.</summary>
    private void OnEditorContentHeight(uint px)
    {
        _contentHeight = px;
        SizeEditor();
    }

    /// <summary>The page's <c>caret</c> notice.</summary>
    private void OnEditorCaret(uint top, uint bottom)
    {
        _caret = (top, bottom);
        FollowCaret();
    }

    /// <summary>Scrolls the column by what the core answers for the caret last reported, and
    /// leaves it where it is when the caret is already in view. A target beyond the column's end
    /// is applied as far as it goes and kept, and tried again after the next layout pass
    /// (<see cref="OnColumnContentSizeChanged"/>). It is always reached in the end: the editor is
    /// at least as tall as its content, and the room under the caret (the editor's padding, its
    /// border and the column's own bottom padding) is more than the core's margin.</summary>
    private void FollowCaret()
    {
        if (_caret is not { } caret)
        {
            return;
        }
        var page = EditorTop;
        if (MailcalBindingsMethods.ComposerCaretScroll(
                ComposerScroll.VerticalOffset, ComposerScroll.ViewportHeight, page + caret.Top, page + caret.Bottom) is { } offset)
        {
            ComposerScroll.ChangeView(null, offset, null, disableAnimation: true);
            // Half a pixel either way is the editor's height rounded to whole pixels.
            if (offset > ComposerScroll.ScrollableHeight + 0.5)
            {
                return;
            }
        }
        _caret = null;
    }

    /// <summary>Tells the page where the column's visible part starts in its own coordinates, so
    /// it keeps its toolbar there (docs/composer-layout.md, rule 7). One call at a time, carrying
    /// the latest value when it goes.</summary>
    private async void TellVisibleTop()
    {
        if (!_editorReady || _visibleTopSending)
        {
            return;
        }
        // Zero or less is the same answer to the page: its top is in view.
        var top = Math.Max(0, Math.Round(ComposerScroll.VerticalOffset - EditorTop));
        if (top == _visibleTopSent)
        {
            return;
        }
        _visibleTopSending = true;
        _visibleTopSent = top;
        try
        {
            var drawnAt = await _editor.ReadNumberAsync(string.Create(CultureInfo.InvariantCulture, $"window.setComposerVisibleTop({top})"));
            // The page's own answer, so a suite sees the toolbar drawn where it is rather than a
            // call merely having been made: the agent cannot reach inside the page.
            TestAgent.Signal(ComposerScroll, drawnAt > 0 ? "toolbar kept in view" : "toolbar in its place");
        }
        catch (Exception ex)
        {
            // An async void handler: a throw here would take the app down with it.
            Log.Warn($"composer: couldn't place the toolbar ({ex.GetType().Name})");
        }
        finally
        {
            _visibleTopSending = false;
        }
        // The column may have moved on while the call was out.
        TellVisibleTop();
    }

    /// <summary>The rule over Send and Discard, drawn while the column runs on under them, so a
    /// line cut by the edge reads as more below rather than as the message's end. Only the
    /// column's own bottom padding left below is the end: following a caret on the last line
    /// stops a little short of it, by the padding the core's margin does not use.</summary>
    private void ShowEdges() =>
        FooterRule.Visibility = ComposerScroll.ScrollableHeight - ComposerScroll.VerticalOffset > Column.Padding.Bottom
            ? Visibility.Visible
            : Visibility.Collapsed;

    /// <summary>Where the editor's page starts in the column: under everything above the message,
    /// the gap between them, and the frame's border.</summary>
    private double EditorTop => AboveEditor.ActualHeight + Column.Spacing + EditorFrame.BorderThickness.Top;

    /// <summary>Gives the editor what its content needs or what the column's visible part leaves
    /// under the header, the files and the column's bottom padding, whichever is more. Rounded
    /// down, so a message that fits leaves the column nothing to scroll.</summary>
    private void SizeEditor()
    {
        var room = Math.Floor(ComposerScroll.ViewportHeight - EditorTop - EditorFrame.BorderThickness.Bottom - Column.Padding.Bottom);
        Editor.Height = MailcalBindingsMethods.ComposerEditorHeight(_contentHeight, room);
    }
}
