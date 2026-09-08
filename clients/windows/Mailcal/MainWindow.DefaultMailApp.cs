// The one-time offer to become the machine's default mail app (docs/os-integration.md).
//
// The decision to ask is the shared core's and reaches this file as one boolean; what is here is
// the prompt and the hand-off to Windows' own settings. The permanent way back is
// Settings → General (SettingsDialog.cs), which is what makes asking exactly once acceptable.
using System;
using System.Threading;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;
using Windows.System;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // Guards against a second prompt: the account collection changes in bursts (one signal per
    // account as they connect), and every one of them reaches the same check. Without this the
    // core's answer would still be `true` while the first dialog is on screen but not yet
    // answered, and two dialogs would stack.
    private int _offeringDefaultMailApp;

    /// <summary>
    /// Puts the offer up, if the core says it is due. Called when the account list changes, which
    /// is the earliest honest moment: before there is an account the app cannot send mail, so the
    /// offer would be asking for a commitment to something the user has not seen work.
    /// </summary>
    private async void OfferDefaultMailAppIfDue()
    {
        if (!Model.ShouldOfferDefaultMailApp())
        {
            return;
        }
        // Never while something else owns the screen, and never ahead of a launch the user did
        // aim at us. DialogHelper drops a second show and answers `None`, which is also what its
        // close button answers, so an offer put now would be recorded as declined without anyone
        // having been asked, and this one is put once. It waits for the next account change, or
        // the next launch, both of which come back here.
        if (DialogHelper.IsShowing || _pendingShare is not null || _pendingMailLink is not null)
        {
            return;
        }
        if (Interlocked.Exchange(ref _offeringDefaultMailApp, 1) == 1)
        {
            return;
        }
        try
        {
            var taken = await DialogHelper.ConfirmAsync(
                Content.XamlRoot,
                L10n.DefaultMailAppOfferTitle(),
                L10n.DefaultMailAppOfferMessage(),
                L10n.DefaultMailAppOfferAccept(),
                L10n.DefaultMailAppOfferDecline()) == ContentDialogResult.Primary;
            if (taken)
            {
                OpenDefaultAppsSettings();
            }
            // Both answers end the offer, closing it included: a question dismissed without an
            // answer has still been answered, and asking again is how a prompt becomes nagging.
            Model.RecordDefaultMailAppOffer(
                taken ? DefaultMailAppOutcome.Accepted : DefaultMailAppOutcome.Declined);
        }
        finally
        {
            Volatile.Write(ref _offeringDefaultMailApp, 0);
        }
    }

    /// <summary>
    /// Opens Windows' Default apps settings, at this app's own page where the OS supports it.
    /// </summary>
    /// <remarks>
    /// Here rather than in <see cref="DefaultMailApp"/> because it needs WinRT, and that class is
    /// kept linkable by the plain net10.0 test suite. Fire-and-forget by design: the user changes
    /// the association in Windows' own UI, in their own time, and there is nothing to come back
    /// for. A page that will not open is not worth an error dialog either, the user can reach it
    /// themselves, and the offer has been spent whichever way it went.
    /// </remarks>
    internal static async void OpenDefaultAppsSettings()
    {
        try
        {
            await Launcher.LaunchUriAsync(new Uri(DefaultMailApp.SettingsUri(AppIdentity.Aumid)));
        }
        catch (Exception error)
        {
            Log.Warn($"could not open the default-apps settings: {error.GetType().Name}");
        }
    }
}
