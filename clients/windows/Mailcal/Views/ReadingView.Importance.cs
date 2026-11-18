// The importance banner under an open message's header (docs/importance.md). Whether there is one,
// and which, is the core's ReadingSnapshot.ImportanceBanner; this only words and shows it. Its own
// partial to keep ReadingView.xaml.cs under the 500-line limit.

using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ReadingView
{
    /// <summary>
    /// Opens the banner the body asks for, or closes it for none: null while loading, or when
    /// nothing is selected, beside the recipient rows.
    /// </summary>
    private void SetImportance(ReadingBody? body)
    {
        var importance = body?.ImportanceBanner;
        ImportanceBanner.Message = importance switch
        {
            MessageImportance.High => L10n.ReadingImportanceHigh(),
            MessageImportance.Low => L10n.ReadingImportanceLow(),
            _ => string.Empty,
        };
        var open = importance is MessageImportance.High or MessageImportance.Low;
        if (open)
        {
            // The glyph a list row draws for the same importance: the exclamation mark, or the
            // arrow.
            ImportanceBanner.IconSource = new FontIconSource
            {
                Glyph = importance == MessageImportance.Low ? "\uE74B" : "\uE8C9",
            };
        }
        ImportanceBanner.IsOpen = open;
    }
}
