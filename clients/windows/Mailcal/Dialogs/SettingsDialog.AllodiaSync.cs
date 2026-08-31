// Settings → Accounts: what the person's other devices have to say, above their own mail accounts.
// Its Apple, Android and Linux twins draw the same three things in the same order, keep the states
// and the wording in step.
//
// It sits in Accounts rather than in the Allodia-account category because what it is about is mail
// accounts: one arriving is an account to set up, and that is where somebody looks for it.
//
// Its own partial, like every other category here, so SettingsDialog.cs stays clear of the
// 500-line limit.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The last "keep this device's settings" failure, in the store's own words, or null.
    private string? _allodiaSyncFailure;

    /// <summary>
    /// The section, or null when there is nothing to say, including before the first pass has
    /// run, which must not look like a pass that found nothing.
    /// </summary>
    private UIElement? BuildAllodiaSync()
    {
        var report = _model.AllodiaSync;
        var hasSomethingToSay = report is not null
            && (report.Offers.Length > 0
                || report.ChangedElsewhere.Length > 0
                || report.RemovedElsewhere.Length > 0);
        var failure = _allodiaSyncFailure ?? _model.AllodiaSyncFailure;
        if (!_model.AllodiaSyncing && failure is null && !hasSomethingToSay)
        {
            return null;
        }

        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(Heading(L10n.SettingsAllodiaSyncHeading()));
        panel.Children.Add(Description(L10n.SettingsAllodiaSyncDescription()));
        if (_model.AllodiaSyncing)
        {
            var busy = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            busy.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
            busy.Children.Add(new TextBlock
            {
                Text = L10n.SettingsAllodiaSyncChecking(),
                Opacity = 0.7,
            });
            panel.Children.Add(busy);
        }
        if (report is not null)
        {
            foreach (var offer in report.Offers)
            {
                var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                row.Children.Add(new TextBlock
                {
                    Text = offer.Email,
                    VerticalAlignment = VerticalAlignment.Center,
                });
                var setUp = new Button { Content = L10n.SettingsAllodiaSyncSetUp() };
                var taken = offer;
                setUp.Click += (_, _) => StartAddAccount(taken);
                row.Children.Add(setUp);
                panel.Children.Add(row);
            }
            // Both of these are questions, and the only answer this device can act on today is
            // "keep what I have". Applying the other side's settings needs a path for editing a
            // connected account's server details, which does not exist yet.
            foreach (var change in report.ChangedElsewhere)
            {
                panel.Children.Add(
                    Question(L10n.SettingsAllodiaChangedElsewhere(change.Email), change.AccountId));
            }
            foreach (var change in report.RemovedElsewhere)
            {
                panel.Children.Add(
                    Question(L10n.SettingsAllodiaRemovedElsewhere(change.Email), change.AccountId));
            }
        }
        if (failure is not null)
        {
            panel.Children.Add(BuildAllodiaFailure());
        }
        return panel;
    }

    /// <summary>
    /// What a failed pass is allowed to put on screen.
    /// </summary>
    /// <remarks>
    /// The core's typed answer decides, never the failure text. A grant that predates a permission
    /// and one the service revoked are different sentences with different remedies, and everything
    /// else is a bad afternoon that says nothing about either, so it gets one plain line and the
    /// detail goes to the diagnostic log alone.
    /// <para>
    /// This is what stops the next unclassified error reaching a person the way `oauth endpoint
    /// error: invalid_scope, unable to issue scope mailcal:accounts:read` did: there is no longer
    /// a path from an exception's text to the screen.
    /// </para>
    /// </remarks>
    private UIElement BuildAllodiaFailure()
    {
        switch (_model.AllodiaGrantHealth)
        {
            case AllodiaGrantHealth.NeedsReauth:
            {
                // An offer, not an error: the person is signed in, and one feature is asleep.
                var panel = new StackPanel { Spacing = 6 };
                panel.Children.Add(Heading(L10n.SettingsAllodiaReauth()));
                panel.Children.Add(Description(L10n.SettingsAllodiaReauthHint()));
                var again = new Button { Content = L10n.SettingsAllodiaReauthAction() };
                again.Click += async (_, _) => await SignInToAllodiaAgainAsync();
                panel.Children.Add(again);
                return panel;
            }
            case AllodiaGrantHealth.SignedOut:
            {
                var panel = new StackPanel { Spacing = 6 };
                panel.Children.Add(Heading(L10n.SettingsAllodiaSignedOutElsewhere()));
                panel.Children.Add(Description(L10n.SettingsAllodiaSignedOutElsewhereHint()));
                var again = new Button { Content = L10n.SettingsAllodiaSignIn() };
                again.Click += async (_, _) => await SignInToAllodiaAgainAsync();
                panel.Children.Add(again);
                return panel;
            }
            default:
            {
                var line = Description(L10n.SettingsAllodiaSyncUnavailable());
                line.Opacity = 1;
                line.Foreground = (Microsoft.UI.Xaml.Media.Brush)
                    Application.Current.Resources["SystemFillColorCriticalBrush"];
                return line;
            }
        }
    }

    /// <summary>
    /// Runs the ordinary sign-in again, which is the whole of the remedy for both states.
    /// </summary>
    /// <remarks>
    /// No separate flow: the sign-in asks for the full current scope set every time, so re-running
    /// it is what widens a grant that predates one, and what replaces a grant the service refused.
    /// A pass follows so the account list is there before the person looks for it.
    /// </remarks>
    private async Task SignInToAllodiaAgainAsync()
    {
        var failure = await _model.SignInToAllodiaAsync();
        if (failure is null)
        {
            await _model.SyncAllodiaAccountsAsync();
        }
        Apply(() => _allodiaSyncFailure = failure);
    }

    /// <summary>
    /// Opens the setup form on the offered address. Settings closes first: the form takes over the
    /// window, and leaving a dialog in front of it would hide the thing that just opened.
    /// </summary>
    private void StartAddAccount(AllodiaAccountOffer offer)
    {
        Hide();
        _model.BeginAddAccount(offer.Email, offer);
    }

    private UIElement Question(string text, string accountId)
    {
        var panel = new StackPanel { Spacing = 4 };
        panel.Children.Add(Description(text));
        var keep = new Button { Content = L10n.SettingsAllodiaKeepLocal() };
        // "Keep what I have" is Paused: the other devices keep the account, and this one stops
        // exchanging changes about it, which is exactly what the question asked.
        keep.Click += async (_, _) =>
        {
            var failure = await _model.SetAllodiaAccountSyncModeAsync(
                accountId, AllodiaAccountSyncMode.Paused);
            Apply(() => _allodiaSyncFailure = failure);
        };
        panel.Children.Add(keep);
        return panel;
    }
}
