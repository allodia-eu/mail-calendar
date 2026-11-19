// The account's notice mark, one line: see AccountMarkLine.xaml.

using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Views;

/// <summary>The notice mark beneath an account's address.</summary>
public sealed partial class AccountMarkLine : UserControl
{
    /// <summary>The mark to draw, or null for none.</summary>
    public static readonly DependencyProperty MarkProperty = DependencyProperty.Register(
        nameof(Mark), typeof(SidebarMark), typeof(AccountMarkLine), new PropertyMetadata(null, OnMarkChanged));

    /// <summary>Initialises the control.</summary>
    public AccountMarkLine()
    {
        this.InitializeComponent();
    }

    /// <summary>The mark to draw, or null for none.</summary>
    public SidebarMark? Mark
    {
        get => (SidebarMark?)GetValue(MarkProperty);
        set => SetValue(MarkProperty, value);
    }

    private static void OnMarkChanged(DependencyObject owner, DependencyPropertyChangedEventArgs e)
    {
        var line = (AccountMarkLine)owner;
        if (e.NewValue is not SidebarMark mark)
        {
            line.Visibility = Visibility.Collapsed;
            return;
        }
        line.MarkGlyph.Glyph = mark.Glyph;
        line.MarkGlyph.Foreground = NoticeBar.ToneBrush(mark.Tone);
        line.MarkLabel.Text = mark.Label;
        line.Visibility = Visibility.Visible;
    }
}
