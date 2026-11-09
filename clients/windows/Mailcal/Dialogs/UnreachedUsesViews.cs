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
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;
using Windows.Foundation;
using Windows.UI;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The separate-server form's controls. Connect and the other way out are drawn from the
/// core's section each time it changes (<see cref="Show"/>); the fields are the person's.</summary>
internal sealed record SeparateServerForm(
    StackPanel Panel, TextBlock Title, TextBox Username, PasswordBox Password, InfoBar Error, StackPanel Buttons,
    SeparateServerSignInRow SignIn)
{
    // What the buttons were last drawn from, so a draw that changes nothing keeps them.
    private string? _buttonsDrawn;

    // Connect as drawn, which a password typed or cleared readies.
    private ControlComponents.FormSend? _connect;

    /// <summary>
    /// Draws the form for <paramref name="step"/> as the core drew it and as
    /// <paramref name="signIn"/> stands: Connect and <paramref name="secondary"/> (Skip in setup,
    /// Cancel on the account's page) from the core's controls, held as well while the sign-in's
    /// own core call runs, the fields with Connect, and the sign-in once offered.
    /// <paramref name="cancelOwn"/> is whether the attempt draws a Cancel of its own, which the
    /// account's page does not, its form's Cancel being that way back. Setup's Skip is the core's
    /// <c>StopUsing</c>; the page's Cancel only closes, so it waits for a connect it would not stop.
    /// </summary>
    internal void Show(
        SeparateServerStep step, SeparateServerSignIn signIn, bool cancelOwn, string secondary,
        Action connect, Action other)
    {
        var section = step.Section;
        var offer = SeparateServerSignIn.Offer(step);
        var offered = offer != SeparateSignInOffer.None;
        // The sign-in's own blocking core calls hold what the core does not know is held.
        var browsing = !signIn.FormEnabled;
        var connectState = section.Connect with { Enabled = section.Connect.Enabled && !browsing };
        var otherState = cancelOwn
            ? section.StopUsing with { Enabled = section.StopUsing.Enabled && !browsing }
            : new ControlState(
                !section.Connect.Running && signIn.Phase != SeparateSignInPhase.Completing, false);
        ControlComponents.HoldFields(connectState, Username, Password);
        var key = $"{connectState}/{otherState}/{offered}";
        if (key != _buttonsDrawn)
        {
            _buttonsDrawn = key;
            Buttons.Children.Clear();
            // One primary action: the sign-in where it is offered, Connect otherwise.
            var style = offered ? null : (Style)Application.Current.Resources["AccentButtonStyle"];
            _connect = ControlComponents.FormButton(L10n.ActionConnect(), connectState, "SeparateServerConnect", style, connect);
            Buttons.Children.Add(_connect.Element);
            Buttons.Children.Add(ControlComponents.Button(secondary, otherState, "SeparateServerSecondary", other));
        }
        Readied();
        SignIn.Panel.Visibility = offered ? Visibility.Visible : Visibility.Collapsed;
        // Nextcloud's sign-in is named as itself, and says where the person is while the browser
        // holds it (docs/nextcloud-sign-in.md rule 5).
        var nextcloud = offer == SeparateSignInOffer.Nextcloud;
        SignIn.Button.Content = nextcloud ? L10n.NextcloudSignInButton() : L10n.SetupImapSigninButton();
        SignIn.Note.Text = !nextcloud ? L10n.SetupImapSigninNote()
            : signIn.Phase == SeparateSignInPhase.Waiting ? L10n.NextcloudSignInWaiting()
            : L10n.NextcloudSignInNote();
        SignIn.Button.IsEnabled = signIn.ButtonEnabled && section.Connect.Enabled;
        SignIn.Cancel.Visibility = cancelOwn && signIn.CancelShown ? Visibility.Visible : Visibility.Collapsed;
        SignIn.Ring.IsActive = signIn.Busy;
        SignIn.Ring.Visibility = signIn.Busy ? Visibility.Visible : Visibility.Collapsed;
    }

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

/// <summary>The sign-in at the server's own provider, or Nextcloud's, above the login: the button,
/// its Cancel and ring while an attempt is out, and the note beneath. Collapsed until the core
/// offers it.</summary>
internal sealed record SeparateServerSignInRow(StackPanel Panel, Button Button, Button Cancel, ProgressRing Ring, TextBlock Note);

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
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var button = new Button
        {
            Content = L10n.SetupImapSigninButton(),
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
        };
        AutomationProperties.SetAutomationId(button, "SeparateServerSignIn");
        var cancel = new Button { Content = L10n.ActionCancel(), Visibility = Visibility.Collapsed };
        AutomationProperties.SetAutomationId(cancel, "SeparateServerSignInCancel");
        var ring = new ProgressRing
        {
            Width = 20,
            Height = 20,
            IsActive = false,
            Visibility = Visibility.Collapsed,
            VerticalAlignment = VerticalAlignment.Center,
        };
        buttons.Children.Add(button);
        buttons.Children.Add(cancel);
        buttons.Children.Add(ring);
        panel.Children.Add(buttons);
        var note = new TextBlock
        {
            Text = L10n.SetupImapSigninNote(),
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(note, "SeparateServerSignInNote");
        panel.Children.Add(note);
        return new SeparateServerSignInRow(panel, button, cancel, ring, note);
    }
}
