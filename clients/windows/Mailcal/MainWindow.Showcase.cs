// Drives a showcase (screenshot) run to the screen MAILCAL_SHOWCASE_SCREEN names, so the store
// screenshot set is captured without a single tap, scripts/dev/showcase.sh relaunches the app once
// per screen per language. The Windows twin of the Apple client's Mailcal.Showcase.swift and the
// Android client's MainActivityCore showcase driver.
//
// Inert unless ShowcaseMode.IsOn, which is hard-false in a release build, so every branch here is
// dropped by the compiler when the app ships, and the call sites stay free of #if DEBUG.

using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;
using Windows.Graphics;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // The frame a screenshot run pins, in logical units, the same 1440x900 the macOS client sizes
    // itself to, so the two desktop listings share one layout. ScaleToDpi turns it into the physical
    // pixels PrintWindow captures and the Store measures: 1440x900 at 100% scale, 2160x1350 at 150%,
    // 2880x1800 at 200%, all inside the Store's 1366x768..3840x2160 bounds. Sizing in *logical*
    // units (rather than pinning 1920x1080 physical) also keeps the reading pane above its MinWidth
    // on a HiDPI display, where 1920 physical pixels is only 960 logical ones and the pane collapses.
    private static readonly SizeInt32 ShowcaseLogicalSize = new(1440, 900);

    // The window's own top border row, which the app does not paint and PrintWindow does not
    // render, it comes back pure white under a light theme and pure black under a dark one, never
    // the grey the border actually is. screenshot.ps1 crops it off both ways (Test-BorderRow), so
    // the window has to carry one row more than the frame we want kept, exactly as it carries the
    // invisible resize border on the other three sides.
    private const int ShowcaseBorderRows = 1;

    // One-shot guards, so a hook fires once per launch even though rows arrive over several
    // snapshots (and OpenMessage itself dispatches an intent that triggers another reload).
    private bool _showcaseDriven;
    private bool _showcaseReplied;

    // Whether the window's content has loaded, and so has a XamlRoot. A ContentDialog cannot open
    // without one, and the showcase dataset is *fast*, served from memory, it can deliver rows and
    // a message body within ~170ms of launch, well before the content tree is up. Both dialog
    // screens therefore wait for this rather than assuming the UI is ready by the time data is.
    private bool _showcaseContentLoaded;

    /// <summary>
    /// Arms the showcase driver: pins the window to a store-valid frame, then drives to the
    /// requested screen as soon as *both* the data it needs and the UI it needs exist. Called from
    /// the constructor before <c>Model.Start()</c>, so no row can arrive before we are listening.
    /// </summary>
    private void ShowcaseInit()
    {
        if (!ShowcaseMode.IsOn)
        {
            return;
        }
        ShowcaseSizeWindow();

        ((FrameworkElement)Content).Loaded += (_, _) =>
        {
            _showcaseContentLoaded = true;
            // Logged here, not in ShowcaseSizeWindow: Log.Init runs inside Model.Start(), which the
            // constructor reaches only after ShowcaseInit, so anything logged there goes nowhere.
            // The painted size is what the shutter keeps, so log that, AppWindow.Size alone would
            // read 26 px wider than every PNG on disk and send the next reader hunting.
            var inset = ShowcaseFrameInset();
            Log.Info($"showcase: window pinned to {AppWindow.Size.Width - (2 * inset.Width)}x"
                + $"{AppWindow.Size.Height - inset.Height - ShowcaseBorderRows} px painted "
                + $"({AppWindow.Size.Width}x{AppWindow.Size.Height} incl. the invisible resize "
                + "border and the window's border row)");
            // Data may well have arrived before the UI did, re-check both drivers now.
            ShowcaseDriveIfNeeded();
            ShowcaseReplyIfNeeded();
        };
        Model.Rows.CollectionChanged += (_, _) => ShowcaseDriveIfNeeded();
        Model.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(MailboxModel.Reading))
            {
                ShowcaseReplyIfNeeded();
            }
        };
    }

    // Sizes and centres the window for the shutter. Centring matters because screenshot.ps1 falls
    // back to a screen-region BitBlt when PrintWindow can't render the frame, and a window hanging
    // off the edge of the work area would be captured clipped.
    //
    // The size asked for is the *painted* frame, so the window rect is inflated by everything the
    // capture crops off: the invisible resize border on three sides (ShowcaseFrameInset) and the
    // border row along the top (ShowcaseBorderRows). Without that the pinned 1440x900 is the outer
    // rect, the app really gets ~1427x893 logical to lay out in, and the capture is that much
    // smaller than the size the store README documents.
    private void ShowcaseSizeWindow()
    {
        var painted = ScaleToDpi(ShowcaseLogicalSize);
        var inset = ShowcaseFrameInset();
        var size = new SizeInt32(
            painted.Width + (2 * inset.Width),
            painted.Height + inset.Height + ShowcaseBorderRows);
        AppWindow.Resize(size);
        var work = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        AppWindow.Move(new PointInt32(
            work.X + ((work.Width - size.Width) / 2),
            work.Y + ((work.Height - size.Height) / 2)));
        _restoredBounds = new RectInt32(AppWindow.Position.X, AppWindow.Position.Y, size.Width, size.Height);
        _maximized = false;
    }

    // How much bigger the window rect is than the app actually paints: `Width` per side (left and
    // right), `Height` at the bottom only, a restored window's caption reaches its top edge.
    //
    // Since Vista a sizeable window's rect is inflated by an *invisible* resize border that exists
    // only to give the mouse something to grab; nothing paints it. AppWindow.Size, GetWindowRect and
    // PrintWindow all speak in that inflated rect, which is why a capture taken at it came out
    // framed in an unpainted black L (13 px at 200%). screenshot.ps1 crops that margin off, and
    // inflating by the same inset here makes what survives the crop the size we asked for.
    //
    // Measured from system metrics rather than DWM: this runs from the constructor, before the
    // window has ever been shown, so DWMWA_EXTENDED_FRAME_BOUNDS has nothing to report yet, and
    // DWM's frame is 2 px narrower than the app's paint area anyway, since DWM draws the visible
    // border itself. ...ForDpi over the plain call, which would answer for the primary display's
    // scale on a mixed-DPI desktop.
    private SizeInt32 ShowcaseFrameInset()
    {
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        var dpi = GetDpiForWindow(hwnd);
        if (dpi == 0)
        {
            dpi = 96;
        }
        var padded = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
        return new SizeInt32(
            GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) + padded,
            GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + padded);
    }

    private const int SM_CXSIZEFRAME = 32;
    private const int SM_CYSIZEFRAME = 33;
    private const int SM_CXPADDEDBORDER = 92;

    [DllImport("user32.dll")] private static extern int GetSystemMetricsForDpi(int index, uint dpi);

    // Opens a showcase dialog on the next dispatcher tick, off whatever event is running now (a
    // ContentDialog shown from inside a Loaded or PropertyChanged handler races the layout pass).
    // A fire-and-forget `_ = SomeAsync()` would park any exception in an unobserved Task and the
    // screen would just be silently missing from the set, so failures are logged instead.
    private void ShowcaseShowDialog(string what, Func<Task> show) => DispatcherQueue.TryEnqueue(async () =>
    {
        try
        {
            Log.Info($"showcase: opening {what}");
            await show();
        }
        catch (Exception ex)
        {
            Log.Warn($"showcase: could not open {what}: {ex.GetType().Name}: {ex.Message}");
        }
    });

    // Drives to the requested screen once the surface it needs exists. Re-entered on every row
    // change until it fires, so a screen that needs a row simply waits for one.
    private void ShowcaseDriveIfNeeded()
    {
        if (!ShowcaseMode.IsOn || _showcaseDriven)
        {
            return;
        }
        switch (ShowcaseMode.Screen)
        {
            case ShowcaseScreen.Settings:
                // The settings dialog needs a XamlRoot; re-entered from the Loaded handler.
                if (!_showcaseContentLoaded)
                {
                    return;
                }
                _showcaseDriven = true;
                ShowcaseShowDialog("settings", () => OpenSettingsAsync());
                break;

            case ShowcaseScreen.Signatures:
                // The same dialog, opened on the Signatures category, the showcase library is
                // seeded, so this shows a real library and its per-account defaults rather than
                // the empty state (crates/mailcal-bindings/src/boot/inmemory.rs).
                //
                // Waiting for an ACCOUNT, not just for the content to load, is what makes that
                // true. The settings panels are built imperatively, once, and never rebuilt when
                // the core arrives, so opening the dialog during the async bring-up renders the
                // empty state permanently, and the capture is a screenshot of "you haven't written
                // a signature yet" over a core holding two. Nothing about the image says so; the
                // driver is re-entered as the model reloads, so this simply waits.
                if (!_showcaseContentLoaded || Model.Accounts.Count == 0)
                {
                    return;
                }
                _showcaseDriven = true;
                ShowcaseShowDialog("signatures", () => OpenSettingsAsync("signatures"));
                break;

            case ShowcaseScreen.AddAccount:
                _showcaseDriven = true;
                Log.Info("showcase: opening the add-account form");
                Model.BeginAddAccount();
                break;

            case ShowcaseScreen.List:
                // Windows has a persistent reading pane, so the list screenshot opens the first
                // message into it (as macOS and iPad do; iPhone and Android leave the list alone).
                if (Model.Rows.Count == 0)
                {
                    return;
                }
                _showcaseDriven = true;
                Log.Info("showcase: opening the first row into the reading pane");
                Model.OpenMessage(Model.Rows[0]);
                break;

            case ShowcaseScreen.Reply:
                var target = MailcalBindingsMethods.ShowcaseReply(ShowcaseMode.SeedLocale);
                if (FindShowcaseRow(target.Account, target.MessageKey) is not { } row)
                {
                    return;
                }
                _showcaseDriven = true;
                Log.Info("showcase: opening the designated message to reply to");
                Model.OpenMessage(row);
                break;

            case ShowcaseScreen.Invitation:
                // Opening the message is the whole drive: the invitation card is part of the
                // reading pane, and the core primed the calendar at boot, so the card comes up
                // with its day preview expanded rather than "we haven't looked at your calendar".
                var invite = MailcalBindingsMethods.ShowcaseInvitation();
                if (FindShowcaseRow(invite.Account, invite.MessageKey) is not { } inviteRow)
                {
                    return;
                }
                _showcaseDriven = true;
                Log.Info("showcase: opening the meeting invitation");
                Model.OpenMessage(inviteRow);
                break;

            case ShowcaseScreen.Calendar:
                // The calendar needs no mail rows, only a realised content tree, re-entered from
                // the Loaded handler once that exists (like the settings screen).
                if (!_showcaseContentLoaded)
                {
                    return;
                }
                _showcaseDriven = true;
                // Enqueued off the current Loaded/CollectionChanged handler: ShowCalendarSurface
                // swaps the detail pane and OnShown scrolls the Win2D grid to now, both of which
                // race an in-flight layout pass if run inline (the same reason the reply composer
                // below is enqueued rather than called on the spot).
                DispatcherQueue.TryEnqueue(() =>
                {
                    Log.Info("showcase: showing the calendar");
                    ShowCalendarSurface();
                });
                break;

            default:
                break;
        }
    }

    // Opens the reply composer once the target message's body has arrived, the quoted original,
    // and the sample reply text seeded above it, only exist then. Fires once per launch.
    //
    // The composer is no longer a dialog: it renders in the reading-pane slot, so this goes through
    // the shell's ComposeReply, the same call the reading pane's Reply button makes, so the store
    // listing shows the composer the user actually gets. Still enqueued rather than called inline,
    // because we are inside a PropertyChanged/Loaded handler and BeginCompose swaps the detail
    // column's content mid-layout-pass otherwise.
    private void ShowcaseReplyIfNeeded()
    {
        if (!ShowcaseMode.IsOn || ShowcaseMode.Screen != ShowcaseScreen.Reply || _showcaseReplied
            || !_showcaseContentLoaded)
        {
            return;
        }
        // Reading is cleared to null on each open and refilled when that message's body lands, so
        // matching the keys rejects a body left over from a previous open rather than racing it.
        if (Model.OpenedMessage is not { } opened
            || Model.Reading is not { } reading
            || reading.Key != opened.Key)
        {
            return;
        }
        _showcaseReplied = true;
        DispatcherQueue.TryEnqueue(() =>
        {
            Log.Info("showcase: opening the reply composer");
            ComposeReply(opened.Account, opened.Key, replyAll: false);
        });
    }

    // The showcase's designated message is a standalone (unthreaded) message in both locale seeds,
    // so it always lists as a flat row. Null until that row has loaded.
    private MailRow? FindShowcaseRow(string account, string key) => Model.Rows
        .FirstOrDefault(row => !row.IsThread && row.Account == account && row.LatestKey == key);
}
