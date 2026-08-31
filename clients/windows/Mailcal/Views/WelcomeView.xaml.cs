// The first-boot welcome screen, the Windows twin of macOS's WelcomeView.swift and Android's
// WelcomeScreen.kt. Keep the wording and the rules in step; docs/analytics.md is the contract.
//
// The decision is taken exactly once, when the user leaves the screen. Recording the "no" case
// matters as much as the "yes" one: it is what stops us asking again.

using System;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Views;

/// <summary>Welcomes the user, and asks the one usage-statistics question.</summary>
public sealed partial class WelcomeView : UserControl
{
    /// <summary>The shared app model (set by the host via <see cref="Init"/>).</summary>
    public MailboxModel? Model { get; private set; }

    /// <summary>
    /// The privacy policy, from the shared l10n catalog, so all four clients point at one place,
    /// and a localised policy page can diverge later without touching any client.
    /// </summary>
    public Uri PrivacyUri { get; } = new(L10n.WelcomePrivacyUrl());

    /// <summary>Initialises the control.</summary>
    public WelcomeView() => this.InitializeComponent();

    /// <summary>Binds the screen to the shared model.</summary>
    public void Init(MailboxModel model) => Model = model;

    /// <summary>
    /// Reveals (or hides) the literal payload. Pulled lazily: an unopened panel costs nothing, and
    /// before consent the preview honestly reports that no install id exists yet.
    /// <para>
    /// An inline reveal, not a second dialog, WinUI does not allow a nested ContentDialog, and the
    /// Settings copy of this panel has to work from inside one.
    /// </para>
    /// </summary>
    private void OnTogglePayload(object sender, RoutedEventArgs e)
    {
        if (PayloadPanel.Visibility == Visibility.Visible)
        {
            PayloadPanel.Visibility = Visibility.Collapsed;
            return;
        }
        PayloadText.Text = Model?.AnalyticsPayloadPreview() ?? string.Empty;
        PayloadPanel.Visibility = Visibility.Visible;
    }

    /// <summary>
    /// Records the decision and moves on. <c>ShareStats.IsOn</c> is false unless the user
    /// deliberately moved the switch, the affirmative action the consent has to rest on.
    /// </summary>
    private void OnGetStarted(object sender, RoutedEventArgs e) =>
        Model?.SetAnalyticsConsent(ShareStats.IsOn);
}
