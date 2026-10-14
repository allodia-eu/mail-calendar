// The page a message is drawn on, and the sun/moon button that changes it
// (docs/reading-dark-mode.md). Its own partial to keep ReadingView.xaml.cs under the 500-line limit.
//
// The scheme follows two inputs that change while a message is open: the view's own ActualTheme,
// which tracks the app's appearance (and, for System, the desktop's), and the core's preference,
// which the button and Settings → Reading both set. Either one re-applies the canvas and the
// button, and re-renders the body, which keeps the reader's place (ReadingView.WebView.cs).

using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ReadingView
{
    /// <summary>The page this message is drawn on: dark exactly when the app is and the reader
    /// left dark messages on. Known before the body arrives, so the canvas is right from the
    /// first frame.</summary>
    private MessageScheme Scheme =>
        MessageSchemes.For(this.IsDark(), _model?.DarkMessageBackground ?? true);

    /// <summary>The scheme the canvas and the button were last set for.</summary>
    private MessageScheme? _appliedScheme;

    /// <summary>Puts the canvas, its theme and the button in step with <see cref="Scheme"/>, and
    /// re-renders the body when the scheme moved.</summary>
    private void ApplyScheme()
    {
        var scheme = Scheme;
        var dark = scheme == MessageScheme.Dark;
        // The button exists only in a dark app: in a light one there is nothing to switch.
        SchemeToggle.Visibility = this.IsDark() ? Visibility.Visible : Visibility.Collapsed;
        // What pressing it does: a sun sends messages to their light page, a moon to a dark one.
        SchemeToggleGlyph.Glyph = dark ? "" : "";
        var name = dark ? L10n.ReadingShowLightBackground() : L10n.ReadingShowDarkBackground();
        AutomationProperties.SetName(SchemeToggle, name);
        ToolTipService.SetToolTip(SchemeToggle, name);
        if (_appliedScheme == scheme)
        {
            return;
        }
        _appliedScheme = scheme;
        BodyArea.Background = new SolidColorBrush(
            CalendarColors.Parse(MailcalBindingsMethods.MessageCanvas(scheme).Background));
        BodyArea.RequestedTheme = dark ? ElementTheme.Dark : ElementTheme.Light;
        if (_model is not null)
        {
            Render();
        }
    }

    private void OnToggleScheme(object sender, RoutedEventArgs e) =>
        _model?.SetDarkMessageBackground(Scheme != MessageScheme.Dark);
}
