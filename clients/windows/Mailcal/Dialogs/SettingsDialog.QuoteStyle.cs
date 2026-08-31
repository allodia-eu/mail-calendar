// The Composing category's quote-style chooser: how a reply or forward quotes the original. Two
// named styles, each shown as a worked example rather than described in words, the names alone
// ("Indented", "Line + header") don't tell you what you'd get, and the preview does. Below them, an
// opt-in toggle that puts the same choice in every composer so a single message can deviate from
// the default without changing it.
//
// Split into its own partial to keep SettingsDialog.cs under the 500-line limit. The example content
// comes from ComposerQuote.Example, which builds it from the same catalog keys a real quote uses, so
// the preview can't drift from what the composer actually renders; the rendering mirrors the shared
// editor's CSS (clients/composer/dist/editor.html) and the Rust renderer (mailcal-composer).

using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The app-level default style (each option previewed) plus the per-message opt-in.
    private UIElement QuoteStyleControl()
    {
        var settings = _model.QuoteSettings;
        var panel = new StackPanel { Spacing = 12 };
        const string Group = "quote-style";

        panel.Children.Add(QuoteStyleOption(
            L10n.QuoteStyleIndented(),
            L10n.QuoteStyleIndentedDescription(),
            Group,
            QuoteStyleChoice.Indented,
            settings.Style));
        panel.Children.Add(QuoteStyleOption(
            L10n.QuoteStyleLineHeader(),
            L10n.QuoteStyleLineHeaderDescription(),
            Group,
            QuoteStyleChoice.LineAndHeader,
            settings.Style));

        // The advanced opt-in: show the style picker in every composer. Off by default, so an
        // ordinary reply just uses the default above and the composer stays uncluttered.
        var perMessage = new CheckBox
        {
            Content = L10n.SettingsQuotePerMessageHeading(),
            IsChecked = settings.PerMessage,
        };
        perMessage.Checked += (_, _) => _model.SetQuoteStylePerMessage(true);
        perMessage.Unchecked += (_, _) => _model.SetQuoteStylePerMessage(false);
        panel.Children.Add(perMessage);
        panel.Children.Add(new TextBlock
        {
            Text = L10n.SettingsQuotePerMessageDescription(),
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.7,
        });

        return panel;
    }

    // One style: the radio + its name, a plain-language description, and the live example below.
    private UIElement QuoteStyleOption(
        string label,
        string description,
        string group,
        QuoteStyleChoice style,
        QuoteStyleChoice selected)
    {
        var panel = new StackPanel { Spacing = 4 };
        panel.Children.Add(Radio(
            label, group, selected == style, () => _model.SetQuoteStyleChoice(style)));
        panel.Children.Add(new TextBlock
        {
            Text = description,
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.7,
            Margin = new Thickness(28, 0, 0, 4),
        });
        panel.Children.Add(QuoteStyleExample(style));
        return panel;
    }

    // The worked example. Deliberately not an editor, just enough of the shape (the indent and
    // left rule, or the divider and labelled header block) to recognise at a glance which one you
    // want.
    private static UIElement QuoteStyleExample(QuoteStyleChoice style)
    {
        var example = ComposerQuote.Example();
        var rule = (Brush)Application.Current.Resources["ControlStrokeColorDefaultBrush"];
        var body = new StackPanel { Spacing = 4 };

        if (style == QuoteStyleChoice.Indented)
        {
            body.Children.Add(Caption(example.Line, muted: true));
            // The left rule + inset the indented style renders the original in.
            var indented = new Border
            {
                BorderBrush = rule,
                BorderThickness = new Thickness(2, 0, 0, 0),
                Padding = new Thickness(8, 0, 0, 0),
                Child = Caption(example.Body, muted: false),
            };
            body.Children.Add(indented);
        }
        else
        {
            // The divider the original is set off by, then the header block at full width.
            body.Children.Add(new Border
            {
                BorderBrush = rule,
                BorderThickness = new Thickness(0, 1, 0, 0),
                Margin = new Thickness(0, 0, 0, 2),
            });
            foreach (var (headerLabel, value) in example.Headers)
            {
                var line = new TextBlock { FontSize = 11, Opacity = 0.7, TextWrapping = TextWrapping.Wrap };
                line.Inlines.Add(new Run { Text = $"{headerLabel}: ", FontWeight = FontWeights.SemiBold });
                line.Inlines.Add(new Run { Text = value });
                body.Children.Add(line);
            }
            body.Children.Add(Caption(example.Body, muted: false));
        }

        return new Border
        {
            Background = (Brush)Application.Current.Resources["LayerFillColorDefaultBrush"],
            CornerRadius = new CornerRadius(6),
            Padding = new Thickness(10),
            Margin = new Thickness(28, 0, 0, 0),
            Child = body,
        };
    }

    private static TextBlock Caption(string text, bool muted) => new()
    {
        Text = text,
        FontSize = 11,
        Opacity = muted ? 0.7 : 1.0,
        TextWrapping = TextWrapping.Wrap,
    };
}
