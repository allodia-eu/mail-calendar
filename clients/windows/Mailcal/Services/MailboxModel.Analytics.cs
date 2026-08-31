// The usage-statistics surface of the model. Split into its own partial to keep MailboxModel.cs
// under the 500-line limit. Consent lives in Rust (persisted in the shared preferences file), so
// this only forwards, there is no second copy of the decision to drift.
//
// The rules the callers must honour are legal conditions, not preferences (docs/analytics.md):
// default off, never pre-ticked, and withdrawal as easy as giving (GDPR Art. 7(3)).

using Microsoft.UI.Xaml;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Whether the usage-statistics question is settled. <c>false</c> puts the welcome screen up,
    /// it is the first thing a new user sees, ahead of setup. It also covers a returning user
    /// upgrading into this version, who has accounts already but has never been asked.
    /// <para>
    /// <c>true</c> before the app has connected, so no welcome screen can flash while the core is
    /// still starting: we do not ask on a guess.
    /// </para>
    /// </summary>
    public bool AnalyticsAsked => _app?.AnalyticsConsent().Asked ?? true;

    /// <summary>Whether usage statistics are on. Read fresh, the core owns the value.</summary>
    public bool AnalyticsEnabled => _app?.AnalyticsConsent().Enabled ?? false;

    /// <summary>Show the welcome screen only while the question is open.</summary>
    public Visibility WelcomeVisibility => AnalyticsAsked ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>
    /// Records the decision, and it <em>is</em> a decision either way: passing <c>false</c> stores
    /// a decline, which is what stops us asking again. Opting in mints the install id; opting out
    /// clears it and asks the backend to erase everything held under it (GDPR Art. 17).
    /// <para>
    /// Call this only from a deliberate, affirmative action, a switch the user moved, a button
    /// they pressed. Never from a pre-checked box or from accepting terms.
    /// </para>
    /// </summary>
    public void SetAnalyticsConsent(bool enabled)
    {
        _app?.SetAnalyticsConsent(enabled);
        // The core raises no Settings surface for this, so nothing else would tell the shell that
        // the welcome screen is done and the app proper can show.
        Raise(nameof(AnalyticsAsked));
        Raise(nameof(AnalyticsEnabled));
        Raise(nameof(WelcomeVisibility));
        Raise(nameof(SetupVisibility));
        Raise(nameof(MainVisibility));
    }

    /// <summary>
    /// The literal JSON the core would put on the wire, for the "see exactly what we send" panel.
    /// Built from the same type the sink serializes, so it is the payload rather than a description
    /// of one.
    /// </summary>
    public string AnalyticsPayloadPreview() => _app?.AnalyticsPayloadPreview() ?? string.Empty;

    /// <summary>
    /// The retention signal, once per launch, after boot. A no-op until the user opts in, and on
    /// the very first launch the opt-in itself reports the session, so consenting is never a launch
    /// we fail to count.
    /// </summary>
    public void ReportAppOpened() => _app?.ReportAppOpened();
}
