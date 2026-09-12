// The one-time offer to become the machine's default mail app (docs/os-integration.md).
//
// The decision to ask is the shared core's and reaches this file as one boolean; what is here is
// the prompt and the hand-off to Windows' own settings. The permanent way back is
// Settings → General (SettingsDialog.cs), which is what makes asking exactly once acceptable.
using System;
using System.Threading;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
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

    // Set while the offer is waiting for the window's content to reach the visual tree, so a burst
    // of account signals subscribes to Loaded once rather than once per account.
    private bool _offerAwaitingTree;

    /// <summary>
    /// Puts the offer up, if the core says it is due. Called when the account list changes, which
    /// is the earliest honest moment: before there is an account the app cannot send mail, so the
    /// offer would be asking for a commitment to something the user has not seen work.
    /// </summary>
    private async void OfferDefaultMailAppIfDue()
    {
        // The three refusals and their reasons are DefaultMailApp.WhenToAsk's, which is where they
        // can be unit-tested; what is here is only the acting on them. Both "wait" answers come
        // back: the screen frees on the next account change or the next launch, and the tree
        // arrives at Loaded, below.
        var content = Content as FrameworkElement;
        var timing = DefaultMailApp.WhenToAsk(
            due: Model.ShouldOfferDefaultMailApp(),
            screenTaken: DialogHelper.IsShowing
                || _pendingShare is not null
                || _pendingMailLink is not null,
            rooted: content?.XamlRoot is not null);
        if (timing is DefaultMailApp.Timing.WaitForVisualTree
            && content is not null
            && !_offerAwaitingTree)
        {
            _offerAwaitingTree = true;
            content.Loaded += OfferDefaultMailAppOnceRooted;
        }
        // The second half of the condition is the same question WhenToAsk was just asked; it is
        // here because the compiler cannot carry that answer across the call.
        if (timing is not DefaultMailApp.Timing.Ask || content?.XamlRoot is not { } root)
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
                root,
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

    /// <summary>Puts the offer once the content is rooted, having been asked for too early.</summary>
    private void OfferDefaultMailAppOnceRooted(object sender, RoutedEventArgs args)
    {
        ((FrameworkElement) sender).Loaded -= OfferDefaultMailAppOnceRooted;
        _offerAwaitingTree = false;
        OfferDefaultMailAppIfDue();
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
