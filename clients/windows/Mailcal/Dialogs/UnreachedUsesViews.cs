// What a calendar and contacts a sign-in does not reach put in front of a person
// (docs/accounts.md rule 18): the question a switch to the provider asks before the browser opens,
// and the form that gives them a sign-in of their own. The question is asked over the whole of the
// surface that asks it, the form is drawn inline. The decisions are UnreachedUsesRules', which the
// unit suite drives.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;
using Windows.Foundation;
using Windows.UI;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The separate-server form's controls. Connect, the other way out and the browser
/// sign-in are drawn from the core's section each time it changes (<see cref="Show"/>); the fields
/// are the person's.</summary>
internal sealed record SeparateServerForm(
    StackPanel Panel, TextBlock Title, TextBox Username, PasswordBox Password, InfoBar Error, StackPanel Buttons,
    SeparateServerSignInRow SignIn)
{
    // What the buttons and the sign-in were last drawn from, so a draw that changes nothing keeps
    // them.
    private string? _buttonsDrawn;
    private string? _signInDrawn;

    // Connect as drawn, which a password typed or cleared readies.
    private ControlComponents.FormSend? _connect;

    /// <summary>
    /// Draws the form for <paramref name="step"/> as the core drew it: Connect from the core's
    /// control, the fields held with it, <paramref name="secondary"/> (Skip in setup, Cancel on the
    /// account's page) from <paramref name="otherState"/>, and the sign-in once offered, running
    /// as the core says, with a line and its Cancel while the browser holds it.
    /// <paramref name="signIn"/> starts the sign-in and <paramref name="cancelSignIn"/> cancels
    /// the one the browser holds.
    /// </summary>
    internal void Show(
        SeparateServerStep step, ControlState otherState, string secondary, Action connect, Action other,
        Action signIn, Action<BrowserSignIn> cancelSignIn)
    {
        var section = step.Section;
        var row = SeparateServerSignIn.Row(step);
        var offered = row.Offer != SeparateSignInOffer.None;
        ControlComponents.HoldFields(section.Connect, Username, Password);
        var key = $"{section.Connect}/{otherState}/{offered}";
        if (key != _buttonsDrawn)
        {
            _buttonsDrawn = key;
            Buttons.Children.Clear();
            // One primary action: the sign-in where it is offered, Connect otherwise.
            var style = offered ? null : AccentStyle();
            _connect = ControlComponents.FormButton(L10n.ActionConnect(), section.Connect, "SeparateServerConnect", style, connect);
            Buttons.Children.Add(_connect.Element);
            Buttons.Children.Add(ControlComponents.Button(secondary, otherState, "SeparateServerSecondary", other));
        }
        Readied();
        SignIn.Panel.Visibility = offered ? Visibility.Visible : Visibility.Collapsed;
        // Nextcloud's sign-in is named as itself, and says where the person is while the browser
        // holds it (docs/nextcloud-sign-in.md rule 5).
        var nextcloud = row.Offer == SeparateSignInOffer.Nextcloud;
        SignIn.Note.Text = row.Line switch
        {
            SignInWaitLine.Nextcloud => L10n.NextcloudSignInWaiting(),
            SignInWaitLine.Redirect => L10n.SettingsAccountSigninWaiting(section.Username),
            _ when nextcloud => L10n.NextcloudSignInNote(),
            _ => L10n.SetupImapSigninNote(),
        };
        if (row.Key == _signInDrawn)
        {
            return;
        }
        _signInDrawn = row.Key;
        SignIn.Button.Children.Clear();
        SignIn.Button.Children.Add(ControlComponents.Button(
            nextcloud ? L10n.NextcloudSignInButton() : L10n.SetupImapSigninButton(),
            row.Button, "SeparateServerSignIn", signIn, AccentStyle()));
        SignIn.Cancel.Children.Clear();
        if (row.Waiting is { } waiting)
        {
            SignIn.Cancel.Children.Add(ControlComponents.Button(
                L10n.ActionCancel(), waiting.Cancel, "SeparateServerSignInCancel", () => cancelSignIn(waiting)));
        }
    }

    private static Style AccentStyle() => (Style)Application.Current.Resources["AccentButtonStyle"];

    /// <summary>Holds Connect while there is no password to send.</summary>
    internal void Readied() => _connect?.Ready(Password.Password.Length > 0 ? null : string.Empty);

    /// <summary>Says <paramref name="error"/> under the fields, or takes the last one away.</summary>
    internal void Say(string? error)
    {
        Error.Message = error ?? string.Empty;
        Error.IsOpen = error is not null;
    }
}

/// <summary>A question being asked: its answer, and the way to take it away unanswered, which
/// answers it as Cancel.</summary>
internal sealed record UnreachedPrompt(Task<UnreachedAnswer> Answer, Action Dismiss);

/// <summary>The sign-in at the server's own provider, or Nextcloud's, above the login: where its
/// button is drawn, and the line beneath with where its Cancel is drawn while the browser holds
/// it. Collapsed until the core offers it.</summary>
internal sealed record SeparateServerSignInRow(StackPanel Panel, StackPanel Button, TextBlock Note, StackPanel Cancel);

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
    /// Asks the question centred over <paramref name="over"/>, with the rest dimmed, so it cannot
    /// be missed below a fold. It is a flyout rather than a ContentDialog because Settings and the
    /// add-account form are ContentDialogs already and WinUI shows one at a time, dropping the
    /// second, which would read as Cancel. Escape or a click outside answers Cancel.
    /// </summary>
    internal static UnreachedPrompt Ask(
        FrameworkElement over, IReadOnlyList<AccountCapability> unreached, Func<Func<bool, Color>, Brush> paint)
    {
        var answer = new TaskCompletionSource<UnreachedAnswer>(TaskCreationOptions.RunContinuationsAsynchronously);
        var flyout = new Flyout { LightDismissOverlayMode = LightDismissOverlayMode.On };
        var question = Question(unreached, paint, chosen =>
        {
            answer.TrySetResult(chosen);
            flyout.Hide();
        });
        question.Width = 360;
        flyout.Content = question;
        flyout.Closed += (_, _) => answer.TrySetResult(UnreachedAnswer.Cancel);
        // Centred by hand: a flyout placed Full fills the window's height whatever its content.
        // Below a point, it is centred on it across and starts at it down, so the point is set
        // half the question's height above the middle.
        question.Measure(new Size(question.Width, double.PositiveInfinity));
        flyout.ShowAt(over, new FlyoutShowOptions
        {
            Placement = FlyoutPlacementMode.Bottom,
            Position = new Point(over.ActualWidth / 2, Math.Max(0, (over.ActualHeight - question.DesiredSize.Height) / 2)),
        });
        return new UnreachedPrompt(answer.Task, () =>
        {
            answer.TrySetResult(UnreachedAnswer.Cancel);
            flyout.Hide();
        });
    }

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
        // Stacked, each the question's width: the three labels do not fit on one line in every
        // locale.
        var buttons = new StackPanel { Spacing = 8, Margin = new Thickness(0, 8, 0, 0) };
        var keep = new Button
        {
            Content = L10n.SigninUnreachedKeep(),
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        AutomationProperties.SetAutomationId(keep, "UnreachedKeep");
        var stop = new Button { Content = L10n.SigninUnreachedStop(), HorizontalAlignment = HorizontalAlignment.Stretch };
        AutomationProperties.SetAutomationId(stop, "UnreachedStop");
        // Lightweight styling, so the colour holds through hover and press, which a plain
        // Foreground does not.
        var critical = paint(ThemePalette.Critical);
        foreach (var key in new[] { "ButtonForeground", "ButtonForegroundPointerOver", "ButtonForegroundPressed" })
        {
            stop.Resources[key] = critical;
        }
        var cancel = new Button { Content = L10n.ActionCancel(), HorizontalAlignment = HorizontalAlignment.Stretch };
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
    /// server it names, the login and password, and the row its buttons are drawn into
    /// (<see cref="SeparateServerForm.Show"/>).
    /// </summary>
    internal static SeparateServerForm Form(SeparateServerStep step, string? username = null)
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
        var signIn = SignInRow();
        panel.Children.Add(signIn.Panel);
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
        panel.Children.Add(buttons);
        var form = new SeparateServerForm(panel, title, login, password, error, buttons, signIn);
        password.PasswordChanged += (_, _) => form.Readied();
        return form;
    }

    private static SeparateServerSignInRow SignInRow()
    {
        var panel = new StackPanel { Spacing = 8, Visibility = Visibility.Collapsed };
        var button = new StackPanel();
        panel.Children.Add(button);
        var note = new TextBlock
        {
            Text = L10n.SetupImapSigninNote(),
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(note, "SeparateServerSignInNote");
        AutomationProperties.SetLiveSetting(note, AutomationLiveSetting.Polite);
        panel.Children.Add(note);
        var cancel = new StackPanel();
        panel.Children.Add(cancel);
        return new SeparateServerSignInRow(panel, button, note, cancel);
    }
}
