// Settings → Advanced ▸ AI assistant access (docs/mcp.md, docs/settings.md slot 11), drawn from the
// core's McpPanel and keeping no rule of its own. Desktop-only by construction: the core has no
// panel on a host that set no endpoint, and this draws nothing then.
//
// The order of the controls is the order of the decisions: turn it on, then choose which mailboxes
// it reaches (none to begin with), then, separately, let it send. The three groups below the switch
// are drawn only while it is on.
//
// A signal moves the switches, the boxes and the status line in place, so the control the person
// just used keeps its focus; the groups below the switch are drawn again only when what they list
// moved (AdvancedPageRules.GrantsFingerprint).

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
    // The panel's host, and the controls a signal moves in place. Null while nothing is drawn.
    private StackPanel? _assistants;
    private ToggleSwitch? _mcpToggle;
    private TextBlock? _mcpStatus;
    private StackPanel? _mcpGrants;
    private string? _mcpGrantsDrawn;
    private readonly Dictionary<string, CheckBox> _mcpAccounts = new();
    private TextBlock? _mcpNothingExposed;
    private ToggleSwitch? _mcpDirectSend;
    private CheckBox? _mcpKnownRecipient;

    // A new, empty host for the panel, forgetting the controls of the one before.
    private StackPanel AssistantsHost()
    {
        _assistants = new StackPanel { Spacing = 20 };
        ForgetMcpControls();
        return _assistants;
    }

    private void ForgetMcpControls()
    {
        _mcpToggle = null;
        _mcpStatus = null;
        _mcpGrants = null;
        ForgetMcpGrants();
    }

    private void ForgetMcpGrants()
    {
        _mcpGrantsDrawn = null;
        _mcpAccounts.Clear();
        _mcpNothingExposed = null;
        _mcpDirectSend = null;
        _mcpKnownRecipient = null;
    }

    private void RefreshAssistants(McpPanel? panel)
    {
        if (_assistants is not { } host)
        {
            return;
        }
        if (panel is null)
        {
            host.Children.Clear();
            host.Visibility = Visibility.Collapsed;
            ForgetMcpControls();
            return;
        }
        host.Visibility = Visibility.Visible;
        if (_mcpToggle is null)
        {
            host.Children.Add(Group(
                L10n.SettingsMcpHeading(), L10n.SettingsMcpDescription(), McpMasterControls()));
            _mcpGrants = new StackPanel { Spacing = 20 };
            host.Children.Add(_mcpGrants);
        }
        var grants = AdvancedPageRules.GrantsFingerprint(panel);
        if (grants != _mcpGrantsDrawn && _mcpGrants is { } below)
        {
            below.Children.Clear();
            ForgetMcpGrants();
            _mcpGrantsDrawn = grants;
            if (panel.Enabled)
            {
                below.Children.Add(Group(
                    L10n.SettingsMcpAccountsHeading(),
                    L10n.SettingsMcpAccountsDescription(),
                    McpAccountControls(panel)));
                below.Children.Add(Group(
                    L10n.SettingsMcpSendHeading(), L10n.SettingsMcpSendNote(), McpSendControls()));
                below.Children.Add(Group(
                    L10n.SettingsMcpConfigHeading(),
                    L10n.SettingsMcpConfigDescription(),
                    McpConfigControls(panel.Snippet)));
            }
        }
        Quietly(() => FollowMcp(panel));
    }

    // Every value a signal can move without the groups being drawn again.
    private void FollowMcp(McpPanel panel)
    {
        if (_mcpToggle is { } toggle && toggle.IsOn != panel.Enabled)
        {
            toggle.IsOn = panel.Enabled;
        }
        if (_mcpStatus is { } status)
        {
            status.Text = AdvancedPageRules.StatusText(panel.Status, StatusWords());
            status.Opacity = AdvancedPageRules.StatusNeedsAttention(panel.Status) ? 1.0 : 0.7;
        }
        foreach (var account in panel.Accounts)
        {
            if (_mcpAccounts.TryGetValue(account.AccountId, out var check) && check.IsChecked != account.Exposed)
            {
                check.IsChecked = account.Exposed;
            }
        }
        if (_mcpNothingExposed is { } nothing)
        {
            nothing.Visibility = panel.NothingExposed ? Visibility.Visible : Visibility.Collapsed;
        }
        if (_mcpDirectSend is { } direct && direct.IsOn != panel.AllowDirectSend)
        {
            direct.IsOn = panel.AllowDirectSend;
        }
        if (_mcpKnownRecipient is { } known)
        {
            if (known.IsChecked != panel.RequireKnownRecipient)
            {
                known.IsChecked = panel.RequireKnownRecipient;
            }
            known.IsEnabled = panel.KnownRecipientEditable;
        }
    }

    // A change drawn from the core's record, which the controls' handlers must not send back.
    private void Quietly(Action draw)
    {
        var was = _rebuilding;
        _rebuilding = true;
        try
        {
            draw();
        }
        finally
        {
            _rebuilding = was;
        }
    }

    private static McpStatusWords StatusWords() => new(
        L10n.SettingsMcpStatusOff(),
        L10n.SettingsMcpStatusStarting(),
        L10n.SettingsMcpStatusRunning(),
        L10n.SettingsMcpStatusAnotherInstance(),
        L10n.SettingsMcpStatusUnavailable());

    // On/off, and what the server is doing.
    private UIElement McpMasterControls()
    {
        var stack = new StackPanel { Spacing = 8 };
        var toggle = new ToggleSwitch();
        AutomationProperties.SetAutomationId(toggle, "McpEnabledToggle");
        AutomationProperties.SetName(toggle, L10n.SettingsMcpToggle());
        toggle.Toggled += (_, _) =>
        {
            if (!_rebuilding)
            {
                _model.SetAssistantAccess(new McpIntent.SetEnabled(toggle.IsOn));
            }
        };
        stack.Children.Add(new TextBlock { Text = L10n.SettingsMcpToggle(), TextWrapping = TextWrapping.Wrap });
        stack.Children.Add(toggle);
        var status = new TextBlock { TextWrapping = TextWrapping.Wrap };
        AutomationProperties.SetAutomationId(status, "McpStatus");
        stack.Children.Add(status);
        _mcpToggle = toggle;
        _mcpStatus = status;
        return stack;
    }

    // One box per account used for mail, none ticked to begin with.
    private UIElement McpAccountControls(McpPanel panel)
    {
        var stack = new StackPanel { Spacing = 4 };
        foreach (var account in panel.Accounts)
        {
            var check = new CheckBox { Content = account.Email };
            AutomationProperties.SetName(check, account.Email);
            var id = account.AccountId;
            void Changed()
            {
                if (!_rebuilding)
                {
                    _model.SetAssistantAccess(new McpIntent.SetAccountExposed(id, check.IsChecked == true));
                }
            }
            check.Checked += (_, _) => Changed();
            check.Unchecked += (_, _) => Changed();
            _mcpAccounts[id] = check;
            stack.Children.Add(check);
        }
        if (panel.Accounts.Length == 0)
        {
            stack.Children.Add(Description(L10n.SettingsAccountsEmpty()));
        }
        var nothing = Description(L10n.SettingsMcpAccountsEmpty());
        AutomationProperties.SetAutomationId(nothing, "McpAccountsEmpty");
        _mcpNothingExposed = nothing;
        stack.Children.Add(nothing);
        return stack;
    }

    // Direct send, and the guard that makes it defensible.
    private UIElement McpSendControls()
    {
        var stack = new StackPanel { Spacing = 8 };
        var direct = new ToggleSwitch();
        AutomationProperties.SetAutomationId(direct, "McpDirectSendToggle");
        AutomationProperties.SetName(direct, L10n.SettingsMcpSendToggle());
        direct.Toggled += (_, _) =>
        {
            if (!_rebuilding)
            {
                _model.SetAssistantAccess(new McpIntent.SetDirectSend(direct.IsOn));
            }
        };
        var known = new CheckBox { Content = L10n.SettingsMcpKnownRecipientToggle() };
        AutomationProperties.SetAutomationId(known, "McpKnownRecipientToggle");
        void KnownChanged()
        {
            if (!_rebuilding)
            {
                _model.SetAssistantAccess(new McpIntent.SetKnownRecipient(known.IsChecked == true));
            }
        }
        known.Checked += (_, _) => KnownChanged();
        known.Unchecked += (_, _) => KnownChanged();
        _mcpDirectSend = direct;
        _mcpKnownRecipient = known;

        stack.Children.Add(new TextBlock { Text = L10n.SettingsMcpSendToggle(), TextWrapping = TextWrapping.Wrap });
        stack.Children.Add(direct);
        stack.Children.Add(known);
        stack.Children.Add(Description(L10n.SettingsMcpKnownRecipientNote()));
        return stack;
    }

    // The core's snippet to paste into the assistant, and a Copy button with transient feedback.
    private UIElement McpConfigControls(string snippet)
    {
        var stack = new StackPanel { Spacing = 8 };
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
