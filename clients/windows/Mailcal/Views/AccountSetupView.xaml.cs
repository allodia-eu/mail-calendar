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
    // Each server's port and connection security. A detected route fills them in; on the manual
    // form the user picks, and the port follows the picker until they type one of their own.
    private readonly ManualServerField _imap = ManualServerField.For(MailServerKind.Imap);
    private readonly ManualServerField _smtp = ManualServerField.For(MailServerKind.Smtp);
    // The issuer the detected route's provider named for itself, read by the IMAP pre-flight and
    // the sign-in, which must describe the same account the connect will dial.
    private string? _detectedOauthIssuer;

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
        // The panel below is built in code, so its colours are resolved once rather than tracked by
        // the framework; an appearance change has to rebuild it (ThemePalette).
        ActualThemeChanged += (_, _) => RenderOnboarding();
    }

    /// <summary>Binds the form to the shared model and resets it to the email-first step when reused.</summary>
    public void Init(MailboxModel model)
    {
        Model = model;
        model.PropertyChanged += (_, e) =>
        {
            // Adding another account, or the last one removed: the form starts from the address.
            if ((e.PropertyName == nameof(MailboxModel.AddingAccount) && model.AddingAccount)
                || (e.PropertyName == nameof(MailboxModel.NeedsSetup) && model.NeedsSetup))
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
            // A connect refused for a certificate is answered on this form, so the panel is
            // drawn when one arrives and taken away when the next attempt clears it
            // (docs/certificate-exceptions.md).
            if (e.PropertyName == nameof(MailboxModel.SetupRejectedCertificate))
            {
                ShowRefusedCertificate();
            }
        };
        model.SetupLinksChanged += OnSetupLinksChanged;
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
            var setup = await Model.DetectAsync(DetectEmail.Text);
            ApplyRoute(AccountDetectForm.Route(setup, DetectEmail.Text));
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
        _detectedTab = route.IsManual ? null : route.Tab;
        _detectedUses = route.Uses ?? UseOffer.None;
        _imap.AdoptDetected(route.ImapHost, route.ImapSecurity);
        _smtp.AdoptDetected(route.SmtpHost, route.SmtpSecurity);
        ShowServerSettings();
        _detectedOauthIssuer = route.OauthIssuer;
        // Whether the JMAP fields are a detected result or the manual form decides whether an
        // offered sign-in stands beside the secret field or replaces it. Set before the tab is
        // selected below, since selecting one lays the section out immediately.
        _jmapSignIn.CardChanged(detected: !route.IsManual);
        ApprovalPanel.Visibility = route.NeedsApproval ? Visibility.Visible : Visibility.Collapsed;
        ApprovalCheck.IsChecked = false;
        ShowRefusedCertificate();
        Username.Text = route.Email;
        JmapEmail.Text = route.Email;
        DavLogin.Text = route.Email;
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
                case DetectTab.Dav:
                    DavChoice.IsChecked = true;
                    ShowNote(null);
                    break;
                default:
                    ShowServerFields(
                        ManualServerField.SplitHost(route.ImapHost).Host,
                        ManualServerField.SplitHost(route.SmtpHost).Host);
                    // A discovered CalDAV endpoint is prefilled (opt-out, clear it to skip calendar);
                    // it reuses the IMAP credentials at connect.
                    CaldavUrl.Text = route.CaldavUrl;
                    ImapChoice.IsChecked = true;
                    // The app-password hint travels with the password field, which a server
                    // offering a sign-in keeps behind "Use a password instead".
                    ShowNote(null);
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
        // The manual form types its servers rather than choosing them, starting from what the
        // card had found.
        _detectedTab = null;
        CaldavUrl.Text = DavCaldavUrl.Text = _detectedUses.CaldavUrl;
        CarddavUrl.Text = DavCarddavUrl.Text = _detectedUses.CarddavUrl;
        OnAccountTypeChanged(this, new RoutedEventArgs());
        // This IS the manual path, the one "Set up manually" exists to reach, so the secret and
        // server come back even where a sign-in is on offer.
        _jmapSignIn.CardChanged(detected: false);
        UpdateJmapSignIn();
        UpdateCanConnect();
    }

    private void OnFieldChanged(object sender, TextChangedEventArgs e)
    {
        UpdateCanConnect();
        // TextChanged can fire while the tree is still being built, before the later fields exist.
        if (ImapSignInPanel is null)
        {
            return;
        }
        _imapSignIn.FieldsChanged(Username.Text, ImapHost.Text);
        UpdateImapSignIn();
        ScheduleImapProbe();
    }

    private void OnPasswordChanged(object sender, RoutedEventArgs e) => UpdateCanConnect();

    private void OnApprovalChanged(object sender, RoutedEventArgs e) => UpdateCanConnect();

    private void OnCertificateAcceptedChanged(object sender, RoutedEventArgs e) => UpdateCanConnect();

    /// <summary>
    /// Draws the certificate the last connect was refused for, or takes the panel away when
    /// nothing was refused. Only the IMAP route offers acceptance: a JMAP account's stored config
    /// carries no exception, so a refusal there keeps its plain error instead
    /// (docs/certificate-exceptions.md).
    /// </summary>
    private void ShowRefusedCertificate()
    {
        if (CertificatePanel is null)
        {
            return;
        }
        var refused = RefusedCertificate();
        CertificatePanel.Visibility = refused is null ? Visibility.Collapsed : Visibility.Visible;
        CertificateCheck.IsChecked = false;
        if (refused is not null)
        {
            CertificateWarning.Text = L10n.SetupCertificateWarning(refused.ServerName);
            CertificateClaims.Text = CertificateClaimLines(refused);
        }
        UpdateCanConnect();
    }

    /// <summary>The refused certificate this tab can act on, if any.</summary>
    private RejectedCertificate? RefusedCertificate() =>
        AccountDetectForm.OffersCertificateException(ActiveTab()) ? Model?.SetupRejectedCertificate : null;

    /// <summary>
    /// Which route the form is on, named as the tab the shared gates are stated in terms of, so
    /// the button and the certificate panel read the same rules the unit suite drives.
    /// </summary>
    private DetectTab ActiveTab() =>
        JmapChoice?.IsChecked == true ? DetectTab.Jmap
        : MicrosoftChoice?.IsChecked == true ? DetectTab.Microsoft
        : GoogleChoice?.IsChecked == true ? DetectTab.Google
        : DavChoice?.IsChecked == true ? DetectTab.Dav
        : DetectTab.Imap;

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
        // The gate itself lives in AccountDetectForm, which the unit suite drives: deciding it
        // twice would let a test pass over a button that does something else.
        var tab = ActiveTab();
        ConnectButton.IsEnabled = AccountDetectForm.CanConnect(
            tab,
            _needsApproval,
            ApprovalCheck.IsChecked == true,
            ImapHost.Text,
            tab switch { DetectTab.Jmap => JmapEmail.Text, DetectTab.Dav => DavLogin.Text, _ => Username.Text },
            tab == DetectTab.Dav ? DavPassword.Password : Password.Password,
            JmapPassword.Password,
            certificateRefused: RefusedCertificate() is not null,
            certificateAccepted: CertificateCheck.IsChecked == true,
            // On a server that refuses passwords the field is not on screen, and gating Connect
            // on it would disable a button nobody is looking at anyway.
            passwordShown: _imapSignIn.ShowPassword);
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
        // Switching tabs changes whether a refused certificate can be acted on at all.
        ShowRefusedCertificate();
        var google = GoogleChoice.IsChecked == true;
        var dav = DavChoice.IsChecked == true;
        var imap = !jmap && !microsoft && !google && !dav;
        ImapSection.Visibility = imap ? Visibility.Visible : Visibility.Collapsed;
        DavSection.Visibility = dav ? Visibility.Visible : Visibility.Collapsed;
        ShowDavSection(detected: _detectedTab == DetectTab.Dav);
        RefreshUses(ActiveTab());
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

    // Cancel means "abort the browser sign-in" while one is outstanding (it can hang forever), and
    // "back out of adding an account" otherwise. During a sign-in this leaves the user on the form
    // to retry; a second Cancel then backs out as usual. The address step's Cancel shares this
    // handler, and nothing signs in from there.
    private void OnCancel(object sender, RoutedEventArgs e)
    {
        if (Model?.IsSigningIn == true)
        {
            CancelSignIns();
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
        ShowDetectStep();
        // An offer opened from elsewhere, the Settings list, lands on its own route, the same as
        // one pressed on this screen.
        if (Model?.SetupStartOffer is { } offer)
        {
            ApplyRoute(AccountDetectForm.Route(MailcalBindingsMethods.OfferedSetup(offer), offer.Email));
        }
    }

    private void ClearManualFields()
    {
        ImapChoice.IsChecked = true;
        ImapHost.Text = string.Empty;
        Username.Text = string.Empty;
        Password.Password = string.Empty;
        SmtpHost.Text = string.Empty;
        _imap.Reset();
        _smtp.Reset();
        ShowServerFields(string.Empty, string.Empty);
        CaldavUrl.Text = string.Empty;
        CarddavUrl.Text = string.Empty;
        DavLogin.Text = string.Empty;
        DavPassword.Password = string.Empty;
        DavCaldavUrl.Text = string.Empty;
        DavCarddavUrl.Text = string.Empty;
        _detectedTab = null;
        _detectedUses = UseOffer.None;
        JmapEmail.Text = string.Empty;
        JmapPassword.Password = string.Empty;
        JmapServer.Text = string.Empty;
        ResetJmapSignIn();
        ResetImapSignIn();
        GoogleEarlyAccessCheck.IsChecked = false;
        ConnectButton.IsEnabled = false;
    }
}
