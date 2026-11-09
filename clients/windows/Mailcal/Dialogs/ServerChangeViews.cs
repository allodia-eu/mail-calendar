// The question a calendar or address book server found on another site asks
// (docs/account-autodetect.md rule 14), drawn from the core's AccountServerChange once for the
// dialog the calendar and contacts screens open and for the account's page, which draws it inline
// because Settings is itself a ContentDialog. The sentences and which variant each change earns
// are ServerChangeCopy's.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

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
    /// The question as the core draws it (<paramref name="question"/>): the sentences, the line
    /// saying the last allow failed, "Not now" (<paramref name="notNow"/>) and Allow, which
    /// <paramref name="allow"/> sends. While Allow runs, "Not now" waits for it too: closing would
    /// not stop it. <paramref name="titled"/> draws the title as the panel's heading, for the
    /// account's page; a dialog carries it as its own title. Also returns "Not now", which a
    /// question drawn again is to give the focus, so a stray Return never allows.
    /// </summary>
    internal static (StackPanel Panel, Control NotNow) Question(
        AccountServerChange question, bool titled, Action allow, Action notNow)
    {
        var copy = Copy(question.Change, AccountNameText.Of(question.Account));
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
        if (question.Failed)
        {
            var error = new InfoBar
            {
                Severity = InfoBarSeverity.Error,
                IsClosable = false,
                IsOpen = true,
                Message = L10n.ServerChangedFailed(),
            };
            AutomationProperties.SetAutomationId(error, "ServerChangeError");
            panel.Children.Add(error);
        }
        // Neither is the accent button: allowing a server is never the easy press. "Not now" comes
        // first, so it is what the focus finds first.
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var declining = ControlComponents.Button(
            L10n.ServerChangedNotNow(), new ControlState(!question.Allow.Running, false),
            "ServerChangeNotNow", notNow);
        buttons.Children.Add(declining);
        buttons.Children.Add(ControlComponents.Button(
            L10n.ServerChangedAllow(), question.Allow, "ServerChangeAllow", allow));
        panel.Children.Add(buttons);
        return (panel, (Control)declining.Children[0]);
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
