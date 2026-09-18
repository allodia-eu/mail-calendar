// The root of a reading or composer window: the app's own caption, and the view under it.
//
// Its markup says why it is markup. What is here is the two things a window hands it, the name in
// the caption and the view below, and the one thing it hands back: the element the window's drag
// region is computed from.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Views;

/// <summary>A window's caption and the view it was opened for.</summary>
public sealed partial class WindowShell : UserControl
{
    /// <summary>Builds an empty shell; <see cref="Init"/> fills it.</summary>
    public WindowShell() => this.InitializeComponent();

    /// <summary>Names the window <paramref name="title"/> and puts <paramref name="content"/> under
    /// its caption.</summary>
    internal void Init(string title, UIElement content)
    {
        Caption.Title = title;
        Body.Children.Add(content);
    }

    /// <summary>The element this window's drag region is computed from.</summary>
    /// <remarks>
    /// The caption control itself, unlike the shell's, whose caption row carries a search field
    /// beside it and so hands over the row (MainWindow.TitleBar.cs).
    /// </remarks>
    internal UIElement DragRegion => Caption;
}
