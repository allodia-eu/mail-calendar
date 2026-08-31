// Settings → Advanced ▸ AI assistant access (docs/mcp.md, docs/settings.md slot 9). The Windows leg
// of the cross-platform contract; split into its own partial to keep SettingsDialog.cs under the
// 500-line limit.
//
// Desktop-only, and by construction rather than by a platform check: the core reports no endpoint on
// a host that set none, and this renders nothing when there is none. That mirrors Notifications
// being mobile-only in the same taxonomy.
//
// The order of the controls is the order of the decisions, and it is deliberate: turn it on, then
// choose which mailboxes it reaches (none to begin with), then, separately, and only if you mean it
// Only then let it send. Each is a distinct grant; a single switch conferring all three would be the wrong
// default in the place a wrong default costs the most.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.ApplicationModel.DataTransfer;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The whole panel, or nothing at all when the core has no endpoint (not connected yet, or a
    // host that set none). Returning null rather than an empty group keeps BuildAdvanced's layout
    // from carrying a hole where a section would be.
    private UIElement? McpSection()
    {
        if (_model.Mcp is not { } settings || settings.Endpoint is null)
        {
            return null;
        }
        var panel = new StackPanel { Spacing = 20 };
        panel.Children.Add(Group(
            L10n.SettingsMcpHeading(), L10n.SettingsMcpDescription(), McpMasterControls(settings)));
        // The three grants below exist only once the feature is on. Flipping the switch re-renders
        // the category (Apply), so they appear and disappear as one.
        if (settings.Enabled)
        {
            panel.Children.Add(Group(
                L10n.SettingsMcpAccountsHeading(),
                L10n.SettingsMcpAccountsDescription(),
                McpAccountControls(settings)));
            panel.Children.Add(Group(
                L10n.SettingsMcpSendHeading(), L10n.SettingsMcpSendNote(), McpSendControls(settings)));
            panel.Children.Add(Group(
                L10n.SettingsMcpConfigHeading(),
                L10n.SettingsMcpConfigDescription(),
                McpConfigControls()));
        }
        return panel;
    }

    // On/off, plus whether a pipe is actually bound.
    private UIElement McpMasterControls(McpSettings settings)
    {
        var stack = new StackPanel { Spacing = 8 };
        var toggle = new ToggleSwitch { IsOn = settings.Enabled };
        AutomationProperties.SetAutomationId(toggle, "McpEnabledToggle");
        AutomationProperties.SetName(toggle, L10n.SettingsMcpToggle());
        toggle.Toggled += (_, _) =>
        {
            if (!_rebuilding)
            {
                // Apply, not a bare call: turning it on reveals three more groups, so the panel
                // has to re-render, and on the next dispatcher turn, so the tree is not mutated
                // from inside the Toggled event that is walking it.
                Apply(() => _model.SetMcpEnabled(toggle.IsOn));
            }
        };
        stack.Children.Add(new TextBlock { Text = L10n.SettingsMcpToggle(), TextWrapping = TextWrapping.Wrap });
        stack.Children.Add(toggle);

        // Whether a pipe is bound, not just what the switch says. The two can disagree, another
        // instance owning the name, a name that will not bind, and a panel showing only the switch
        // would tell the user it is on while nothing can reach it.
        var status = new TextBlock
        {
            Text = McpStatusText(settings),
            TextWrapping = TextWrapping.Wrap,
            Opacity = settings.Enabled && !settings.Running ? 1.0 : 0.7,
        };
        AutomationProperties.SetAutomationId(status, "McpStatus");
        stack.Children.Add(status);
        return stack;
    }

    private static string McpStatusText(McpSettings settings) => settings.Enabled
        ? settings.Running ? L10n.SettingsMcpStatusRunning() : L10n.SettingsMcpStatusUnavailable()
        : L10n.SettingsMcpStatusOff();

    // One checkbox per configured account. Nothing is ticked to begin with, and an account that is
    // not ticked is not even NAMED to a client, which mailboxes exist is itself a disclosure.
    private UIElement McpAccountControls(McpSettings settings)
    {
        var stack = new StackPanel { Spacing = 4 };
        // Said out loud rather than implied by empty checkboxes: an assistant reporting "your inbox
        // is empty" is otherwise indistinguishable from one that has been given nothing to look at.
        var empty = new TextBlock
        {
            Text = L10n.SettingsMcpAccountsEmpty(),
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.7,
            Visibility = settings.Accounts.Any(a => a.Exposed) ? Visibility.Collapsed : Visibility.Visible,
        };
        AutomationProperties.SetAutomationId(empty, "McpAccountsEmpty");

        foreach (var account in settings.Accounts)
        {
            // IsChecked is set before the handler attaches, so seeding the stored state does not
            // re-fire into the core (the Radio pattern this dialog uses throughout).
            var check = new CheckBox { Content = account.Email, IsChecked = account.Exposed };
            AutomationProperties.SetName(check, account.Email);
            var id = account.AccountId;
            void Changed()
            {
                if (_rebuilding)
                {
                    return;
                }
                _model.SetMcpAccountExposed(id, check.IsChecked == true);
                // Re-read rather than tracking the boxes: the core is the source of truth for what
                // is exposed, and it is what the note is about.
                empty.Visibility = _model.Mcp?.Accounts.Any(a => a.Exposed) == true
                    ? Visibility.Collapsed
                    : Visibility.Visible;
            }
            check.Checked += (_, _) => Changed();
            check.Unchecked += (_, _) => Changed();
            stack.Children.Add(check);
        }

        if (settings.Accounts.Length == 0)
        {
            stack.Children.Add(new TextBlock
            {
                Text = L10n.SettingsAccountsEmpty(),
                TextWrapping = TextWrapping.Wrap,
                Opacity = 0.7,
            });
        }
        stack.Children.Add(empty);
        return stack;
    }

    // Direct send, and the guard that makes it defensible.
    private UIElement McpSendControls(McpSettings settings)
    {
        var stack = new StackPanel { Spacing = 8 };

        var known = new CheckBox
        {
            Content = L10n.SettingsMcpKnownRecipientToggle(),
            IsChecked = settings.RequireKnownRecipient,
            // Disabled rather than hidden while direct send is off: the guard is what makes direct
            // send defensible, so the user should see it exists before they reach for the switch
            // above it.
            IsEnabled = settings.AllowDirectSend,
        };
        AutomationProperties.SetAutomationId(known, "McpKnownRecipientToggle");

        var direct = new ToggleSwitch { IsOn = settings.AllowDirectSend };
        AutomationProperties.SetAutomationId(direct, "McpDirectSendToggle");
        AutomationProperties.SetName(direct, L10n.SettingsMcpSendToggle());
        direct.Toggled += (_, _) =>
        {
            if (_rebuilding)
            {
                return;
            }
            _model.SetMcpAllowDirectSend(direct.IsOn);
            // Enable the guard's checkbox in place rather than re-rendering: nothing appears or
            // disappears here, and a rebuild would flash the whole category for one IsEnabled.
            known.IsEnabled = direct.IsOn;
        };

        void KnownChanged()
        {
            if (!_rebuilding)
            {
                _model.SetMcpRequireKnownRecipient(known.IsChecked == true);
            }
        }
        known.Checked += (_, _) => KnownChanged();
        known.Unchecked += (_, _) => KnownChanged();

        stack.Children.Add(new TextBlock { Text = L10n.SettingsMcpSendToggle(), TextWrapping = TextWrapping.Wrap });
        stack.Children.Add(direct);
        stack.Children.Add(known);
        stack.Children.Add(new TextBlock
        {
            Text = L10n.SettingsMcpKnownRecipientNote(),
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.7,
        });
        return stack;
    }

    // The snippet to paste into the assistant, and a Copy button with transient feedback.
    private UIElement McpConfigControls()
    {
        var stack = new StackPanel { Spacing = 8 };
        var snippet = _model.McpConfigurationSnippet() ?? string.Empty;

        var text = new TextBlock
        {
            Text = snippet,
            FontFamily = new FontFamily("Consolas"),
            FontSize = 12,
            IsTextSelectionEnabled = true,
        };
        AutomationProperties.SetAutomationId(text, "McpConfigSnippet");
        // Horizontal scroll, never wrap: a wrapped Windows path breaks across a backslash, which
        // reads as a line the user could retype wrongly. It is meant to be copied, not transcribed.
        stack.Children.Add(new ScrollViewer
        {
            Content = text,
            MaxHeight = 160,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            HorizontalScrollMode = ScrollMode.Auto,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        });

        var copy = new Button { Content = L10n.SettingsMcpCopy() };
        AutomationProperties.SetAutomationId(copy, "McpCopyConfig");
        copy.Click += (_, _) =>
        {
            var package = new DataPackage();
            package.SetText(snippet);
            Clipboard.SetContent(package);
            copy.Content = L10n.SettingsMcpCopied();
            var timer = DispatcherQueue.CreateTimer();
            timer.Interval = TimeSpan.FromSeconds(2);
            timer.IsRepeating = false;
            timer.Tick += (_, _) => copy.Content = L10n.SettingsMcpCopy();
            timer.Start();
        };
        stack.Children.Add(copy);
        return stack;
    }
}
