// The question a calendar or address book server found on another site asks
// (docs/account-autodetect.md rule 14), drawn once for the dialog the calendar and contacts screens
// open and for the account's page, which draws it inline because Settings is itself a
// ContentDialog. The sentences and which variant each change earns are ServerChangeCopy's.

using Allodia.Mailcal.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The question's controls: the sentences, the ring while the new server is dialled, and
/// the line saying it could not be.</summary>
internal sealed record ServerChangePanel(StackPanel Panel, ProgressRing Ring, InfoBar Error)
{
    /// <summary>Shows the ring while allowing the new server runs.</summary>
    internal void Busy(bool busy)
    {
        Ring.IsActive = busy;
        Ring.Visibility = busy ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>Says <paramref name="error"/> under the sentences, or takes the last one away.</summary>
    internal void Say(string? error)
    {
        Error.Message = error ?? string.Empty;
        Error.IsOpen = error is not null;
    }
}

internal static class ServerChangeViews
{
    /// <summary>The catalog's words for <see cref="ServerChangeCopy"/>.</summary>
    internal static ServerChangeWords Words() => new(
        uses => uses switch
        {
            ServerChangeUses.Calendar => L10n.ServerChangedTitleCalendar(),
            ServerChangeUses.Contacts => L10n.ServerChangedTitleContacts(),
            _ => L10n.ServerChangedTitleBoth(),
        },
        (uses, address) => uses switch
        {
            ServerChangeUses.Calendar => L10n.ServerChangedBodyCalendar(address),
            ServerChangeUses.Contacts => L10n.ServerChangedBodyContacts(address),
            _ => L10n.ServerChangedBodyBoth(address),
        },
        L10n.ServerChangedBefore,
        L10n.ServerChangedNow,
        L10n.ServerChangedAdvice(),
        uses => uses switch
        {
            ServerChangeUses.Calendar => L10n.ServerChangedUnsureCalendar(),
            ServerChangeUses.Contacts => L10n.ServerChangedUnsureContacts(),
            _ => L10n.ServerChangedUnsureBoth(),
        });

    /// <summary>The question for <paramref name="change"/> on the account at
    /// <paramref name="address"/>.</summary>
    internal static ServerChangeCopy Copy(ServerChange change, string address) =>
        ServerChangeCopy.Of(change, address, Words());

    /// <summary>
    /// The sentences, the ring and the error line. <paramref name="titled"/> draws the title as the
    /// panel's heading, for the account's page; a dialog carries it as its own title.
    /// </summary>
    internal static ServerChangePanel Panel(ServerChangeCopy copy, bool titled)
    {
        var panel = new StackPanel { Spacing = 8 };
        AutomationProperties.SetAutomationId(panel, "ServerChangeQuestion");
        if (titled)
        {
            var title = new TextBlock { Text = copy.Title, FontWeight = FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap };
            AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
            panel.Children.Add(title);
        }
        panel.Children.Add(Sentence(copy.Body));
        var sites = new StackPanel { Spacing = 2 };
        sites.Children.Add(Site(copy.Before, "ServerChangeBefore"));
        sites.Children.Add(Site(copy.Now, "ServerChangeNow"));
        panel.Children.Add(sites);
        panel.Children.Add(Sentence(copy.Advice));
        panel.Children.Add(Sentence(copy.Unsure));
        var ring = new ProgressRing
        {
            Width = 20,
            Height = 20,
            IsActive = false,
            Visibility = Visibility.Collapsed,
            HorizontalAlignment = HorizontalAlignment.Left,
        };
        AutomationProperties.SetAutomationId(ring, "ServerChangeProgress");
        panel.Children.Add(ring);
        var error = new InfoBar { Severity = InfoBarSeverity.Error, IsClosable = false, IsOpen = false };
        AutomationProperties.SetAutomationId(error, "ServerChangeError");
        panel.Children.Add(error);
        return new ServerChangePanel(panel, ring, error);
    }

    private static TextBlock Sentence(string text) => new() { Text = text, TextWrapping = TextWrapping.Wrap };

    // The site in bold, because it is what the person is asked to recognise.
    private static TextBlock Site(SiteLine line, string automationId)
    {
        var block = new TextBlock { TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
        block.Inlines.Add(new Run { Text = line.Lead });
        block.Inlines.Add(new Run { Text = line.Site, FontWeight = FontWeights.Bold });
        block.Inlines.Add(new Run { Text = line.Tail });
        AutomationProperties.SetAutomationId(block, automationId);
        return block;
    }
}
