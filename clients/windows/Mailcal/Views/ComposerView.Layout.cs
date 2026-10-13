// How the composer shares its height between the header fields and the message.

using Microsoft.UI.Xaml;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    /// <summary>The share of the composer's height the header fields may take before they
    /// scroll.</summary>
    private const double HeaderShare = 0.4;

    /// <summary>Caps the header fields at <see cref="HeaderShare"/> of the composer, so the editor,
    /// whose own toolbar takes part of it, keeps room to write however many recipient rows and
    /// a quote-style picker the message carries.</summary>
    private void OnComposerSizeChanged(object sender, SizeChangedEventArgs e) =>
        HeaderScroll.MaxHeight = e.NewSize.Height * HeaderShare;
}
