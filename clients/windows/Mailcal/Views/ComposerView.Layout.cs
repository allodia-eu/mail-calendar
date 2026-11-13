// How tall the message is, and where the column scrolls to keep the caret in view
// (docs/composer-layout.md, rules 5 and 6). The core decides both; this measures and applies.

using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    /// <summary>The height the editor's page last said its content needs, in CSS pixels, which
    /// are this toolkit's device-independent pixels. Nothing until the page first reports.</summary>
    private double _contentHeight;

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
