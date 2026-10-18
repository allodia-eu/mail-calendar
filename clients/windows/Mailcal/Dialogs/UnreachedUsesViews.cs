// What a calendar and contacts a sign-in does not reach put in front of a person
// (docs/accounts.md rule 18): the question a switch to the provider asks before the browser opens,
// and the form that gives them a sign-in of their own. Both are drawn inline, in the panel that
// asks: Settings and the add-account form are ContentDialogs already, and WinUI allows one at a
// time per XamlRoot. The decisions are UnreachedUsesRules', which the unit suite drives.

using Allodia.Mailcal.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;
using Windows.UI;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The separate-server form's controls, for the caller that wires its buttons.</summary>
internal sealed record SeparateServerForm(
    StackPanel Panel, TextBlock Title, TextBox Username, PasswordBox Password, InfoBar Error, Button Connect, Button Secondary)
{
    /// <summary>Disables the form while a connect runs, and enables it again after.</summary>
    internal void Busy(bool busy) =>
        Username.IsEnabled = Password.IsEnabled = Connect.IsEnabled = Secondary.IsEnabled = !busy;

    /// <summary>Says <paramref name="error"/> under the fields, or takes the last one away.</summary>
    internal void Say(string? error)
    {
        Error.Message = error ?? string.Empty;
        Error.IsOpen = error is not null;
    }
}

internal static class UnreachedUsesViews
{
    /// <summary>The question's title, naming what the sign-in will not reach.</summary>
    internal static string QuestionTitle(UnreachedTitle title) => title switch
    {
        UnreachedTitle.Calendar => L10n.SigninUnreachedTitleCalendar(),
        UnreachedTitle.Contacts => L10n.SigninUnreachedTitleContacts(),
        _ => L10n.SigninUnreachedTitleBoth(),
    };

    /// <summary>The form's title, naming what is on a separate server.</summary>
    internal static string FormTitle(UnreachedTitle title) => title switch
    {
        UnreachedTitle.Calendar => L10n.SeparateServerTitleCalendar(),
        UnreachedTitle.Contacts => L10n.SeparateServerTitleContacts(),
        _ => L10n.SeparateServerTitleBoth(),
    };

    /// <summary>
    /// The question, with keep (the default), stop and cancel. <paramref name="answered"/> is
    /// called once, with every button disabled from then on. <paramref name="paint"/> resolves a
    /// colour for the panel's theme: stop is drawn as destructive, because it deletes what the
    /// device holds of those uses.
    /// </summary>
    internal static StackPanel Question(
        IReadOnlyList<AccountCapability> unreached, Func<Func<bool, Color>, Brush> paint, Action<UnreachedAnswer> answered)
    {
        var panel = new StackPanel { Spacing = 8 };
        AutomationProperties.SetAutomationId(panel, "UnreachedQuestion");
        var title = new TextBlock
        {
            Text = QuestionTitle(UnreachedUsesRules.Title(unreached) ?? UnreachedTitle.Both),
            FontWeight = FontWeights.SemiBold,
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
        panel.Children.Add(title);
        panel.Children.Add(new TextBlock { Text = L10n.SigninUnreachedBody(), TextWrapping = TextWrapping.Wrap });
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var keep = new Button
        {
            Content = L10n.SigninUnreachedKeep(),
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
        };
        AutomationProperties.SetAutomationId(keep, "UnreachedKeep");
        var stop = new Button { Content = L10n.SigninUnreachedStop() };
        AutomationProperties.SetAutomationId(stop, "UnreachedStop");
        // Lightweight styling, so the colour holds through hover and press, which a plain
        // Foreground does not.
        var critical = paint(ThemePalette.Critical);
        foreach (var key in new[] { "ButtonForeground", "ButtonForegroundPointerOver", "ButtonForegroundPressed" })
        {
            stop.Resources[key] = critical;
        }
        var cancel = new Button { Content = L10n.ActionCancel() };
        AutomationProperties.SetAutomationId(cancel, "UnreachedCancel");
        void Answer(UnreachedAnswer answer)
        {
            keep.IsEnabled = stop.IsEnabled = cancel.IsEnabled = false;
            answered(answer);
        }
        keep.Click += (_, _) => Answer(UnreachedAnswer.Keep);
        stop.Click += (_, _) => Answer(UnreachedAnswer.Stop);
        cancel.Click += (_, _) => Answer(UnreachedAnswer.Cancel);
        buttons.Children.Add(keep);
        buttons.Children.Add(stop);
        buttons.Children.Add(cancel);
        panel.Children.Add(buttons);
        // Keep is the default, so Enter in the panel answers it.
        panel.Loaded += (_, _) => keep.Focus(FocusState.Programmatic);
        return panel;
    }

    /// <summary>
    /// The form that gives <paramref name="step"/>'s uses a sign-in of their own: the title, the
    /// server it names, the login and password, Connect and <paramref name="secondary"/> (Skip
    /// in setup, Cancel on the account's page). The caller wires both buttons.
    /// </summary>
    internal static SeparateServerForm Form(SeparateServerStep step, string secondary, string? username = null)
    {
        var panel = new StackPanel { Spacing = 8 };
        AutomationProperties.SetAutomationId(panel, "SeparateServerForm");
        var title = new TextBlock
        {
            Text = FormTitle(step.Title),
            FontWeight = FontWeights.SemiBold,
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
        panel.Children.Add(title);
        panel.Children.Add(new TextBlock { Text = L10n.SeparateServerBody(step.Host), TextWrapping = TextWrapping.Wrap });
        var login = new TextBox { Header = L10n.SetupFieldUsername(), Text = username ?? step.Username };
        AutomationProperties.SetAutomationId(login, "SeparateServerUsername");
        var password = new PasswordBox { Header = L10n.SetupFieldPassword() };
        AutomationProperties.SetAutomationId(password, "SeparateServerPassword");
        var error = new InfoBar { Severity = InfoBarSeverity.Error, IsClosable = false, IsOpen = false };
        AutomationProperties.SetAutomationId(error, "SeparateServerError");
        panel.Children.Add(login);
        panel.Children.Add(password);
        panel.Children.Add(error);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var connect = new Button
        {
            Content = L10n.ActionConnect(),
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
        };
        AutomationProperties.SetAutomationId(connect, "SeparateServerConnect");
        var other = new Button { Content = secondary };
        AutomationProperties.SetAutomationId(other, "SeparateServerSecondary");
        buttons.Children.Add(connect);
        buttons.Children.Add(other);
        panel.Children.Add(buttons);
        return new SeparateServerForm(panel, title, login, password, error, connect, other);
    }
}
