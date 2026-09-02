// The model's half of "make this your default mail app": asking the core whether to offer, and
// telling it what came of the offer. The platform call is DefaultMailApp.cs and the surfaces are
// the Settings row (SettingsDialog.cs) and the offer dialog (MainWindow.DefaultMailApp.cs).
//
// Contract: docs/os-integration.md. Split into its own partial to keep MailboxModel.cs under the
// 500-line limit.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Whether to put the one-time offer to become the default mail app in front of the user now.
    /// </summary>
    /// <remarks>
    /// Every condition is the core's: not before the first account exists, not when the app is
    /// already the default, and not twice. What this client contributes is only what the core
    /// cannot know, what this <em>build</em> can do and whether it is already the handler, so
    /// there is no second answer here that could disagree with what Settings shows.
    /// </remarks>
    internal bool ShouldOfferDefaultMailApp() =>
        _app?.ShouldOfferDefaultMailApp(DefaultMailApp.Support, DefaultMailApp.IsDefault) ?? false;

    /// <summary>Records what came of the offer, so it is never put again.</summary>
    /// <remarks>
    /// Recorded as accepted the moment the user takes it, not when Windows' own settings page
    /// reports back, because it never does: the user changes the association there, in their own
    /// time. What is remembered is that <em>we asked</em>, which is the question the once-only
    /// rule is about. The Settings row remains for anyone who changes their mind.
    /// </remarks>
    internal void RecordDefaultMailAppOffer(DefaultMailAppOutcome outcome) =>
        _app?.RecordDefaultMailAppOffer(outcome);

    /// <summary>
    /// What came of the offer, or <c>null</c> if it has not been put: <c>true</c> taken,
    /// <c>false</c> declined. The Settings row reads it to say where things stand.
    /// </summary>
    internal bool? DefaultMailAppOffer => _app?.DefaultMailAppOffer();
}
