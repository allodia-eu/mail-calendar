// The column, the message's box in it, the wheel over the message, the caret and the rule over the
// actions (docs/composer-layout.md, rules 2, 5, 6 and 9). The core decides which scroll a step
// moves and where the column scrolls; this measures and applies.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    /// <summary>The shortest the message's box is drawn, before the column has measured its
    /// visible part.</summary>
    private const double MinimumEditorHeight = 240;

    /// <summary>How far the message is scrolled inside its box, from the page's <c>scroll</c>
    /// notice, in CSS pixels, which are this toolkit's device-independent pixels.</summary>
    private uint _messageScroll;

    /// <summary>The bottom of the message's last line in the page, as it is scrolled now; nothing
    /// until the page first reports.</summary>
    private uint? _messageEnd;

    /// <summary>Whether the page was last told to hold a wheel step down, and whether a call
    /// telling it is under way.</summary>
    private bool? _wheelDownHeld;
    private bool _wheelDownTelling;

    /// <summary>
    /// Hands each wheel step over the message to the column or the message, by the core's answer
    /// (docs/composer-layout.md, rule 2).
    /// </summary>
    /// <remarks>
    /// The WebView2 passes the wheel to its page before any handler here runs, and marks the event
    /// handled, so the ScrollViewer around it never sees it; the event still bubbles to a handler
    /// that takes handled events. A step the core gives the message is the page's already. A step
    /// down the core gives the column the page has held unscrolled (<see cref="HoldWheelDown"/>),
    /// so the column takes it here. Vertical only: a wide table keeps its own horizontal scroll.
    /// </remarks>
    private void ListenForWheel()
    {
        Editor.AddHandler(PointerWheelChangedEvent, new PointerEventHandler(OnEditorWheel), handledEventsToo: true);
        // Focus brings an element wholly into view, and the message's box is as tall as the
        // column's visible part, so a click in the message would carry the whole header away. The
        // caret notice scrolls the column as far as the line being typed needs instead.
        Editor.BringIntoViewRequested += (_, e) => e.Handled = true;
    }

    private void OnEditorWheel(object sender, PointerRoutedEventArgs e)
    {
        var wheel = e.GetCurrentPoint(ComposerScroll).Properties;
        if (wheel.IsHorizontalMouseWheel || wheel.MouseWheelDelta == 0)
        {
            return;
        }
        // A positive delta is a step back up the message.
        var towardsEnd = wheel.MouseWheelDelta < 0;
        if (MailcalBindingsMethods.ComposerScroller(
                ComposerScroll.VerticalOffset, ComposerScroll.ScrollableHeight, _messageScroll, towardsEnd) == ComposerScroller.Column)
        {
            ComposerScroll.ChangeView(null, ComposerScroll.VerticalOffset - wheel.MouseWheelDelta, null);
        }
    }

    private void OnColumnSizeChanged(object sender, SizeChangedEventArgs e)
    {
        SizeEditor();
        ShowEdges();
    }

    private void OnAboveEditorSizeChanged(object sender, SizeChangedEventArgs e) => ShowEdges();

    private void OnColumnContentSizeChanged(object sender, SizeChangedEventArgs e)
    {
        ShowEdges();
        HoldWheelDown();
    }

    private void OnColumnViewChanged(object? sender, ScrollViewerViewChangedEventArgs e)
    {
        ShowEdges();
        HoldWheelDown();
    }

    /// <summary>The page's <c>caret</c> notice: the column scrolls by what the core answers, as far
    /// as its end. Past it the message's box fills the visible part, and the page keeps its caret in
    /// view inside it.</summary>
    private void OnEditorCaret(uint top, uint bottom)
    {
        var page = EditorTop;
        if (MailcalBindingsMethods.ComposerCaretScroll(
                ComposerScroll.VerticalOffset, ComposerScroll.ViewportHeight, page + top, page + bottom) is { } target)
        {
            ComposerScroll.ChangeView(null, Math.Min(target, ComposerScroll.ScrollableHeight), null, disableAnimation: true);
        }
    }

    /// <summary>The page's <c>scroll</c> notice.</summary>
    private void OnEditorScroll(uint px, uint end)
    {
        _messageScroll = px;
        _messageEnd = end;
        ShowEdges();
        HoldWheelDown();
        // The page's own answer, so a suite sees which of the two scrolls moved: the agent cannot
        // reach inside the page. On the column, because the editor carries its own "seeded".
        TestAgent.Signal(ComposerScroll, px > 0 ? "message scrolled" : "message at its top");
    }

    /// <summary>Tells the page whether a wheel step down is the column's, as the core answers it.
    /// One call at a time, carrying the latest answer when it goes.</summary>
    private async void HoldWheelDown()
    {
        if (!_editorReady || _wheelDownTelling)
        {
            return;
        }
        var hold = MailcalBindingsMethods.ComposerScroller(
            ComposerScroll.VerticalOffset, ComposerScroll.ScrollableHeight, _messageScroll, towardsEnd: true) == ComposerScroller.Column;
        if (hold == _wheelDownHeld)
        {
            return;
        }
        _wheelDownTelling = true;
        _wheelDownHeld = hold;
        try
        {
            await _editor.RunAsync(hold ? "window.holdComposerWheelDown(true)" : "window.holdComposerWheelDown(false)");
        }
        catch (Exception ex)
        {
            // An async void handler: a throw here would take the app down with it.
            Log.Warn($"composer: couldn't hand the wheel to the column ({ex.GetType().Name})");
        }
        finally
        {
            _wheelDownTelling = false;
        }
        // The column may have moved on while the call was out.
        HoldWheelDown();
    }

    /// <summary>The rule over Send and Discard, drawn while there is more of the message below
    /// what the column shows, so a line cut by the edge reads as more to come rather than as the
    /// message's end (rule 9).</summary>
    private void ShowEdges() =>
        FooterRule.Visibility = _messageEnd is { } end
            && EditorTop + end > ComposerScroll.VerticalOffset + ComposerScroll.ViewportHeight + 0.5
                ? Visibility.Visible
                : Visibility.Collapsed;

    /// <summary>Where the editor's page starts in the column: under everything above the message
    /// and the gap between them.</summary>
    private double EditorTop => AboveEditor.ActualHeight + Column.Spacing;

    /// <summary>Makes the message's box exactly as tall as the column's visible part, so the
    /// column's end is where the box fills it (rule 2).</summary>
    private void SizeEditor() =>
        Editor.Height = Math.Max(Math.Floor(ComposerScroll.ViewportHeight), MinimumEditorHeight);
}
