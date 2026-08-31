// One gesture, one owner, the pointer half of the shell.
//
// Split from CalendarSurface.cs to stay under the repo's 500-line file cap (AGENTS.md); it is the
// same class. The seam is deliberate: the other file is the swapchain, the clock and the draw; this
// one is every event a finger, a pen or the wheel raises, translated into the pure gesture owner.
//
// **Every touch contact is captured and released INDIVIDUALLY, by its own id.** A pinch is two
// contacts, and the first cut of this file gated the whole gesture on a single "is a finger down"
// flag: finger A's release flipped it, finger B's release took an early return, and B's capture was
// never let go. Windows then believed touch was still owned by a contact that had lifted, and routed
// every new touch to nowhere, the touchscreen went dead while the touchpad (which sends the wheel,
// not a captured pointer) kept working, and the leaks accumulated until the app fell over, a full
// minute of use later. The pure owner was blameless: it tracks pointers in a dictionary and finalises
// only when the LAST one lifts. This file is the shell catching up to it, and CalendarMultiTouchTests
// pins the invariant it depends on. See docs/calendar.md §6.
using System;
using System.Linq;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml.Input;
using Windows.System;

namespace Allodia.Mailcal.Calendar;

internal sealed partial class CalendarSurface
{
    private void OnPointerPressed(object sender, PointerRoutedEventArgs e) => Guarded(e, () =>
    {
        // Capture may be refused (another element already owns it, or a spurious re-press), only
        // track the ones we actually hold, or the release side leaks the opposite way.
        if (_panel.CapturePointer(e.Pointer))
        {
            _captured.Add(e.Pointer.PointerId);
        }
        _owner.PointerDown(Sample(e, e.GetCurrentPoint(_panel)));
    });

    private void OnPointerMoved(object sender, PointerRoutedEventArgs e)
    {
        if (!_captured.Contains(e.Pointer.PointerId))
        {
            return;
        }
        Guarded(e, () =>
        {
            // Every intermediate point, not just the latest. Windows coalesces pointer events, and a
            // flick reported as one 400px jump has no velocity the tracker can believe, a flick that
            // silently becomes a slow drag, and a week that does not turn.
            foreach (var p in e.GetIntermediatePoints(_panel))
            {
                _owner.PointerMoved(new PointerSample(
                    e.Pointer.PointerId, (float)p.Position.X, (float)p.Position.Y, p.Timestamp / 1000d));
            }
        });
    }

    private void OnPointerReleased(object sender, PointerRoutedEventArgs e)
    {
        if (!_captured.Contains(e.Pointer.PointerId))
        {
            return;
        }
        Guarded(e, () =>
        {
            ReleaseCapture(e.Pointer);
            _owner.PointerUp(Sample(e, e.GetCurrentPoint(_panel)));
            CalendarTrace.Gesture(_owner.Mode);
        });
    }

    /// <summary>
    /// The gesture was taken away, a system dialog, the window deactivating, capture stolen.
    /// </summary>
    /// <remarks><b>Between two weeks is never a resting place.</b> The week lands anyway (§6).</remarks>
    private void OnPointerCancelled(object sender, PointerRoutedEventArgs e)
    {
        if (_captured.Count == 0)
        {
            return;
        }
        Guarded(e, () =>
        {
            // A cancel is total: whatever contacts we held, they are all gone at once.
            _captured.Clear();
            _owner.PointerCancelled();
        });
    }

    /// <summary>
    /// The wheel, a mouse, or a precision touchpad's digested two-finger gesture.
    /// </summary>
    /// <remarks>
    /// This is the channel Android does not have, and it goes to the <b>same</b> owner as touch. Give
    /// it to a ScrollViewer instead and the four-handlers-one-finger bug is back, wearing a Windows
    /// hat.
    /// <para>
    /// Ctrl+wheel is a zoom, and on a touchpad that <i>is</i> the pinch: Windows never hands an app
    /// the touchpad's raw contacts, so it arrives as a scalar and only the hours can move. The
    /// diagonal pinch needs a touchscreen. That shortfall is logged in docs/calendar.md.
    /// </para>
    /// </remarks>
    private void OnPointerWheel(object sender, PointerRoutedEventArgs e) => Guarded(e, () =>
    {
        var p = e.GetCurrentPoint(_panel);
        var ctrl = (e.KeyModifiers & VirtualKeyModifiers.Control) != 0;
        var shift = (e.KeyModifiers & VirtualKeyModifiers.Shift) != 0;
        var horizontal = p.Properties.IsHorizontalMouseWheel || shift;

        _owner.Wheel(
            p.Properties.MouseWheelDelta,
            horizontal,
            ctrl,
            (float)p.Position.X,
            (float)p.Position.Y);
    });

    private void OnZoomSettled()
    {
        var settled = _state.SettleZoom(Viewport());
        ZoomSettled?.Invoke(settled, _state.SettledHours());
        Invalidate();
    }

    private void OnTap(float x, float y)
    {
        // A tap on an event opens its detail, hit-test with the renderer's own geometry (§7), so a tap
        // lands on exactly what is drawn. This takes precedence over the banner toggle: a drawn band is
        // on top of the banner region it sits in.
        if (EventAt(x, y) is { } hit)
        {
            OpenEvent?.Invoke(hit);
            return;
        }

        // Otherwise the banner is the one thing left to tap, and only when it is actually hiding lanes.
        var m = _state.Metrics(Viewport());
        var inBanner = y >= m.Viewport.HeaderHeight && y < m.ContentTop;
        if (inBanner && CalendarAllDay.Overflows(m.Viewport.Lanes))
        {
            _state.ToggleBanner();
            Clamp();
            Invalidate();
        }
    }

    private static PointerSample Sample(PointerRoutedEventArgs e, Microsoft.UI.Input.PointerPoint p) =>
        new(e.Pointer.PointerId, (float)p.Position.X, (float)p.Position.Y, p.Timestamp / 1000d);

    private void ReleaseCapture(Pointer pointer)
    {
        _captured.Remove(pointer.PointerId);
        try
        {
            _panel.ReleasePointerCapture(pointer);
        }
        catch (Exception ex)
        {
            // Releasing a capture Windows has already taken back throws; that is fine, the id is gone.
            Log.Info($"cal: release capture ignored ({ex.GetType().Name})");
        }
    }

    /// <summary>
    /// Runs a pointer handler so a fault can neither crash the app nor strand a capture.
    /// </summary>
    /// <remarks>
    /// An unhandled exception in a WinUI pointer handler takes the whole app down, and one thrown
    /// after capture but before release would strand the contact forever, the very failure this file
    /// exists to prevent, arriving by a different door. So any fault ends the gesture cleanly: every
    /// capture released, the owner told to settle the week, and the app still standing.
    /// </remarks>
    private void Guarded(PointerRoutedEventArgs e, Action body)
    {
        e.Handled = true;
        try
        {
            body();
            // An input event always owes the screen a frame. StartTicking alone left _dirty unset, so a
            // notch arriving with nothing else moving started the loop and then drew nothing.
            Invalidate();
        }
        catch (Exception ex)
        {
            Log.Info($"cal: pointer handler faulted ({ex.GetType().Name}), settling and releasing");
            _captured.Clear();
            try
            {
                _panel.ReleasePointerCaptures();
                _owner.PointerCancelled();
            }
            catch (Exception inner)
            {
                Log.Info($"cal: recovery also faulted ({inner.GetType().Name})");
            }
        }
    }
}
