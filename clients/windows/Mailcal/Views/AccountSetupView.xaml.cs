// The account-setup form, the Windows twin of the Android/Apple email-first flow. The user types
// only their email; the shared core detects the provider's settings and routes to a prefilled IMAP
// / JMAP / Microsoft tab, with "Set up manually" as the escape and a reason note when nothing
// usable is found. Untrusted settings gate Connect behind an explicit approval. The routing and
// connect-gating logic lives in AccountDetectForm (unit-tested); this file drives the WinUI panels.
// The JMAP tab's "Sign in with your provider" button is driven from the sibling partial
// AccountSetupView.JmapSignIn.cs, over the likewise unit-tested JmapSignInGate.

using Allodia.Mailcal;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>A setup form for a new account.</summary>
public sealed partial class AccountSetupView : UserControl
{
    /// <summary>The shared app model (set by the host via <see cref="Init"/>).</summary>
    public MailboxModel? Model { get; private set; }

    // Whether the currently-shown settings were obtained untrustably and so need the user's
    // explicit approval before Connect (a non-HTTPS hop, e.g. an http autoconfig).
    private bool _needsApproval;
    // The connection security detection found, remembered across the connect click (there is no
    // security field in the form, the manual form is implicit-TLS only). Defaults to implicit TLS.
    private ConnectionSecurity _imapSecurity = ConnectionSecurity.ImplicitTls;
    private ConnectionSecurity _smtpSecurity = ConnectionSecurity.ImplicitTls;

    /// <summary>Initialises the control.</summary>
    public AccountSetupView()
    {
        this.InitializeComponent();
        // A browser sign-in needs the provider's OAuth client registration, which is injected at
        // build time, so a build given none drops the choice rather than showing one that fails at
        // the provider. Detection never routes to a missing one either, so the section behind a
        // hidden choice is unreachable.
        var routes = MailcalBindingsMethods.OauthRoutes();
        MicrosoftChoice.Visibility = routes.Microsoft ? Visibility.Visible : Visibility.Collapsed;
        GoogleChoice.Visibility = routes.Google ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>Binds the form to the shared model and resets it to the email-first step when reused.</summary>
    public void Init(MailboxModel model)
    {
        Model = model;
        model.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(MailboxModel.AddingAccount) && model.AddingAccount)
            {
                ResetToDetect();
            }
            // The first-run recommendation is drawn only while NeedsSetup holds, and adding a
            // second account has to take it away again (docs/onboarding.md).
            if (e.PropertyName == nameof(MailboxModel.NeedsSetup)
                || e.PropertyName == nameof(MailboxModel.AddingAccount))
            {
                RenderOnboarding();
            }
            if (e.PropertyName == nameof(MailboxModel.IsSubmitting))
            {
                UpdateCanConnect();
                UpdateGoogleSignInEnabled();
            }
        };
        // Once now: on a first run nothing raises AddingAccount, so the panel would otherwise
        // never be built.
        RenderOnboarding();
        this.Bindings.Update();
    }

    private void OnDetectEmailChanged(object sender, TextChangedEventArgs e) =>
        ContinueButton.IsEnabled = !string.IsNullOrWhiteSpace(DetectEmail.Text);

    // Enter in the email box submits detection, same as pressing Continue, but only when Continue
    // would accept it (a non-blank address, and no lookup already in flight), so Enter can't fire an
    // empty or double detection. The single-line TextBox doesn't use Enter for newlines.
    private void OnDetectEmailKeyDown(object sender, Microsoft.UI.Xaml.Input.KeyRoutedEventArgs e)
    {
        if (e.Key == Windows.System.VirtualKey.Enter && ContinueButton.IsEnabled)
        {
            e.Handled = true;
            OnContinueDetect(ContinueButton, new RoutedEventArgs());
        }
    }

    // The email-first lookup: run the core off the UI thread, then route to the prefilled form.
    private async void OnContinueDetect(object sender, RoutedEventArgs e)
    {
        if (Model is null)
        {
            return;
        }
        SetDetecting(true);
        try
        {
            var recommendation = await Model.DetectAsync(DetectEmail.Text);
            ApplyRoute(AccountDetectForm.Route(recommendation));
        }
        finally
        {
            SetDetecting(false);
        }
    }

    // Skip detection straight to the manual tabs, carrying over any email already typed.
    private void OnSetUpManually(object sender, RoutedEventArgs e) =>
        ApplyRoute(new DetectRoute(
            IsManual: true, Tab: DetectTab.Imap, Email: DetectEmail.Text,
            ImapHost: string.Empty, SmtpHost: string.Empty, JmapServer: string.Empty,
            CaldavUrl: string.Empty, NeedsApproval: false, Reason: null));

    // Applies a routed result: prefill the fields, select the tab, show the found/reason note and
    // (when untrusted) the approval, then reveal the form.
    private void ApplyRoute(DetectRoute route)
    {
        _needsApproval = route.NeedsApproval;
        _imapSecurity = route.ImapSecurity;
        _smtpSecurity = route.SmtpSecurity;
        // Whether the JMAP fields are a detected result or the manual form decides whether an
        // offered sign-in stands beside the secret field or replaces it. Set before the tab is
        // selected below, since selecting one lays the section out immediately.
        _jmapSignIn.CardChanged(detected: !route.IsManual);
        ApprovalPanel.Visibility = route.NeedsApproval ? Visibility.Visible : Visibility.Collapsed;
        ApprovalCheck.IsChecked = false;
        Username.Text = route.Email;
        JmapEmail.Text = route.Email;
        // The account-type picker (IMAP/JMAP/Microsoft) is a manual-setup control, not something to
        // put in front of someone the moment detection succeeded, a detected result routes to one
        // provider, so the picker only reads as a confusing choice (mirrors the Android flow, where
        // the picker lives on the manual screen alone). On a detected result we hide it and offer a
        // "Set up manually" link that reveals it; in manual mode the picker is the point, so it shows.
        AccountTypeRow.Visibility = route.IsManual ? Visibility.Visible : Visibility.Collapsed;
        SetupManualButton.Visibility = route.IsManual ? Visibility.Collapsed : Visibility.Visible;

        if (route.IsManual)
        {
            SetupTitleText.Text = L10n.SetupTitle();
            ShowNote(route.Reason is { } reason ? ReasonNote(reason) : null);
            ImapChoice.IsChecked = true;
        }
        else
        {
            SetupTitleText.Text = L10n.SetupDetectFoundTitle();
            switch (route.Tab)
            {
                case DetectTab.Jmap:
                    JmapServer.Text = route.JmapServer;
                    JmapChoice.IsChecked = true;
                    ShowNote(L10n.SetupDetectFoundJmapNote());
                    break;
                case DetectTab.Microsoft:
                    MicrosoftChoice.IsChecked = true;
                    ShowNote(L10n.SetupDetectMicrosoftHint());
                    break;
                case DetectTab.Google:
                    GoogleChoice.IsChecked = true;
                    ShowNote(L10n.SetupDetectGoogleHint());
                    break;
                default:
                    ImapHost.Text = route.ImapHost;
                    SmtpHost.Text = route.SmtpHost;
                    // A discovered CalDAV endpoint is prefilled (opt-out, clear it to skip calendar);
                    // it reuses the IMAP credentials at connect.
                    CaldavUrl.Text = route.CaldavUrl;
                    ImapChoice.IsChecked = true;
                    ShowNote(L10n.SetupDetectAppPasswordHint());
                    break;
            }
        }

        DetectPanel.Visibility = Visibility.Collapsed;
        SetupPanel.Visibility = Visibility.Visible;
        // Selecting a tab above fires OnAccountTypeChanged (which lays out the sections); ensure the
        // IMAP default (already checked) is laid out too, and re-gate Connect.
        OnAccountTypeChanged(this, new RoutedEventArgs());
    }

    // "Set up manually" from a detected result: reveal the account-type picker with the detected
    // settings still prefilled, so an advanced user can edit servers or switch protocol, without
    // re-running detection. Like the manual form on every platform, this drops the untrusted-approval
    // gate: the settings are now shown in editable fields the user is reviewing by hand, which is the
    // review that gate stands in for (the same reason Android's manual screen carries no gate).
    private void OnSetUpManuallyFromFound(object sender, RoutedEventArgs e)
    {
        AccountTypeRow.Visibility = Visibility.Visible;
        SetupManualButton.Visibility = Visibility.Collapsed;
        SetupTitleText.Text = L10n.SetupTitle();
        ShowNote(null);
        _needsApproval = false;
        ApprovalPanel.Visibility = Visibility.Collapsed;
        // This IS the manual path, the one "Set up manually" exists to reach, so the secret and
        // server come back even where a sign-in is on offer.
        _jmapSignIn.CardChanged(detected: false);
        UpdateJmapSignIn();
        UpdateCanConnect();
    }

    private void OnFieldChanged(object sender, TextChangedEventArgs e) => UpdateCanConnect();

    private void OnPasswordChanged(object sender, RoutedEventArgs e) => UpdateCanConnect();

    private void OnApprovalChanged(object sender, RoutedEventArgs e) => UpdateCanConnect();

    // What gates Connect depends on the active tab and, for a detected result, the approval: IMAP
    // needs mail server + email + password; JMAP needs email + one secret (server is discovered);
    // an untrusted result also needs the approval box. A connect in flight disables it.
    private void UpdateCanConnect()
    {
        if (Model is not { IsSubmitting: false } || ImapSection is null)
        {
            if (ConnectButton is not null)
            {
                ConnectButton.IsEnabled = false;
            }
            return;
        }
        var approvalOk = !_needsApproval || ApprovalCheck.IsChecked == true;
        var fieldsOk = JmapChoice.IsChecked == true
            ? JmapSetupForm.CanConnect(JmapEmail.Text, JmapPassword.Password)
            : !string.IsNullOrWhiteSpace(ImapHost.Text)
                && !string.IsNullOrWhiteSpace(Username.Text)
                && !string.IsNullOrEmpty(Password.Password);
        ConnectButton.IsEnabled = approvalOk && fieldsOk;
    }

    // Show the fields for the chosen account type, and re-gate Connect (requirements differ per tab).
    private void OnAccountTypeChanged(object sender, RoutedEventArgs e)
    {
        if (ImapSection is null)
        {
            return;
        }
        var jmap = JmapChoice.IsChecked == true;
        var microsoft = MicrosoftChoice.IsChecked == true;
        var google = GoogleChoice.IsChecked == true;
        var imap = !jmap && !microsoft && !google;
        ImapSection.Visibility = imap ? Visibility.Visible : Visibility.Collapsed;
        JmapSection.Visibility = jmap ? Visibility.Visible : Visibility.Collapsed;
        MicrosoftSection.Visibility = microsoft ? Visibility.Visible : Visibility.Collapsed;
        GoogleSection.Visibility = google ? Visibility.Visible : Visibility.Collapsed;
        // Connect is the IMAP/JMAP submit; the browser-flow providers (Microsoft, Google) use their
        // own sign-in button instead.
        ConnectButton.Visibility = microsoft || google ? Visibility.Collapsed : Visibility.Visible;
        MicrosoftButton.Visibility = microsoft ? Visibility.Visible : Visibility.Collapsed;
        GoogleButton.Visibility = google ? Visibility.Visible : Visibility.Collapsed;
        UpdateCanConnect();
        UpdateGoogleSignInEnabled();
        // The JMAP sign-in button is its own gate (does this server even offer sign-in?), so
        // arriving on the tab, by detection or by picking it, is what starts that lookup.
        UpdateJmapSignIn();
        if (jmap)
        {
            ScheduleJmapProbe();
        }
    }

    private void OnConnect(object sender, RoutedEventArgs e)
    {
        if (JmapChoice.IsChecked == true)
        {
            Model?.SubmitJmapSetup(JmapEmail.Text, JmapServer.Text, JmapPassword.Password);
        }
        else
        {
            Model?.SubmitSetup(ImapHost.Text, Username.Text, Password.Password, SmtpHost.Text, CaldavUrl.Text, _imapSecurity, _smtpSecurity);
        }
    }

    // Pass the address the user typed in the email-first step (empty on a purely manual pick), so
    // Microsoft targets that account rather than a different one already signed in in the browser.
    private void OnSignInMicrosoft(object sender, RoutedEventArgs e) =>
        Model?.SignInWithMicrosoft(DetectEmail.Text);

    // Pass the typed address as the login hint, same as Microsoft. The Early Access checkbox has
    // already gated this button to enabled, so no extra check is needed here.
    private void OnSignInGoogle(object sender, RoutedEventArgs e) =>
        Model?.SignInWithGoogle(DetectEmail.Text);

    // The Early Access checkbox gates the Google sign-in button: the user must confirm they've
    // signed up (Gmail is allow-listed while Google reviews the app) before we can start the flow.
    private void OnGoogleEarlyAccessChanged(object sender, RoutedEventArgs e) => UpdateGoogleSignInEnabled();

    // Open the Early Access sign-up page in the default browser.
    private async void OnOpenGoogleEarlyAccess(object sender, RoutedEventArgs e) =>
        await Windows.System.Launcher.LaunchUriAsync(new System.Uri(L10n.SetupGoogleEarlyAccessUrl()));

    // The Google sign-in button is enabled only once the Early Access box is checked AND nothing is
    // already submitting (so it can't fire twice or while a connect is in flight).
    private void UpdateGoogleSignInEnabled()
    {
        if (GoogleButton is null)
        {
            return;
        }
        GoogleButton.IsEnabled = Model is { IsSubmitting: false } && GoogleEarlyAccessCheck.IsChecked == true;
    }

    // Cancel means "abort the browser sign-in" while one is outstanding (it can hang forever), and
    // "back out of adding an account" otherwise. During a sign-in this leaves the user on the form
    // to retry; a second Cancel then backs out as usual.
    private void OnCancel(object sender, RoutedEventArgs e)
    {
        if (Model?.IsSigningIn == true)
        {
            // Only one browser sign-in runs at a time; cancelling the others is a safe no-op.
            Model.CancelMicrosoftSignIn();
            Model.CancelGoogleSignIn();
            Model.CancelJmapSignIn();
        }
        else
        {
            Model?.CancelAddAccount();
        }
    }

    private void SetDetecting(bool detecting)
    {
        DetectSpinner.IsActive = detecting;
        DetectSpinner.Visibility = detecting ? Visibility.Visible : Visibility.Collapsed;
        DetectingText.Visibility = detecting ? Visibility.Visible : Visibility.Collapsed;
        DetectEmail.IsEnabled = !detecting;
        ManualButton.IsEnabled = !detecting;
        ContinueButton.IsEnabled = !detecting && !string.IsNullOrWhiteSpace(DetectEmail.Text);
    }

    private void ShowNote(string? text)
    {
        DetectNote.Text = text ?? string.Empty;
        DetectNote.Visibility = string.IsNullOrEmpty(text) ? Visibility.Collapsed : Visibility.Visible;
    }

    private static string ReasonNote(MissReason reason) => reason switch
    {
        MissReason.NetworkError => L10n.SetupDetectReasonNetwork(),
        MissReason.OauthOnlyProvider => L10n.SetupDetectReasonOauthOnly(),
        _ => L10n.SetupDetectReasonNothing(),
    };

    // Reset to the email-first step when the form reopens to add another account.
    private void ResetToDetect()
    {
        DetectEmail.Text = Model?.SetupStartEmail ?? string.Empty;
        ClearManualFields();
        _needsApproval = false;
        ApprovalPanel.Visibility = Visibility.Collapsed;
        DetectNote.Visibility = Visibility.Collapsed;
        SetupPanel.Visibility = Visibility.Collapsed;
        DetectPanel.Visibility = Visibility.Visible;
        ContinueButton.IsEnabled = !string.IsNullOrWhiteSpace(DetectEmail.Text);
        // An offer opened from elsewhere, the Settings list, lands on its own route, the same as
        // one pressed on this screen.
        if (Model?.SetupStartOffer is { } offer)
        {
            ApplyRoute(AccountDetectForm.Route(MailcalBindingsMethods.SetupFromOffer(offer)));
        }
    }

    private void ClearManualFields()
    {
        ImapChoice.IsChecked = true;
        ImapHost.Text = string.Empty;
        Username.Text = string.Empty;
        Password.Password = string.Empty;
        SmtpHost.Text = string.Empty;
        CaldavUrl.Text = string.Empty;
        JmapEmail.Text = string.Empty;
        JmapPassword.Password = string.Empty;
        JmapServer.Text = string.Empty;
        ResetJmapSignIn();
        GoogleEarlyAccessCheck.IsChecked = false;
        ConnectButton.IsEnabled = false;
    }
}
