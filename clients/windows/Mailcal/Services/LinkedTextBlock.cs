// Sender text drawn natively with its web and mail addresses clickable: a plain-text body, an
// event's notes, an invitation's description.
//
// SECURITY (Gate 8, docs/rendering-security.md): the core finds the addresses
// (MailcalBindingsMethods.LinkedText) and every run goes in as `Run.Text`, which WinUI renders as
// text and nothing else. No XAML is parsed and nothing reaches XamlReader. A link carries no
// `NavigateUri`, which WinUI would launch on its own; its click asks the shared launch policy first,
// exactly as the reading view's WebView2 handoff does.

using System.Runtime.CompilerServices;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

internal static class LinkedTextBlock
{
    // The text each block was last filled with. A re-render with the same text leaves the inlines
    // alone, so a snapshot arriving behind an open message does not take the reader's selection away.
    private static readonly ConditionalWeakTable<TextBlock, string> Shown = new();

    /// <summary>
    /// Replaces <paramref name="block"/>'s content with <paramref name="text"/>, its addresses as
    /// links. The only writer of a block it fills: <c>TextBlock.Text</c> and the inlines are one
    /// value, and a second writer would leave the other's content standing.
    /// </summary>
    public static void Fill(TextBlock block, string text)
    {
        if (Shown.TryGetValue(block, out var shown) && shown == text)
        {
            return;
        }
        Shown.AddOrUpdate(block, text);
        block.Inlines.Clear();
        foreach (var run in MailcalBindingsMethods.LinkedText(text))
        {
            if (run.Link is not { } target)
            {
                block.Inlines.Add(new Run { Text = run.Text });
                continue;
            }
            var link = new Hyperlink();
            link.Inlines.Add(new Run { Text = run.Text });
            link.Click += (_, _) => Open(target);
            block.Inlines.Add(link);
        }
    }

    // Through the shared launch policy (http(s) and mailto only), then to the OS default handler.
    private static void Open(string url)
    {
        if (MailcalBindingsMethods.ShouldOpenExternalLink(url)
            && Uri.TryCreate(url, UriKind.Absolute, out var uri))
        {
            _ = Windows.System.Launcher.LaunchUriAsync(uri);
        }
    }
}
