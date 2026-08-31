// What the process was asked to do on the command line, the Release-safe cousin of the DEBUG-only
// MAILCAL_* launch hooks.
//
// `--calendar` (or `/calendar`, for Windows convention) opens the app straight on the calendar. It
// exists in every configuration, not just DEBUG, because it is a real product affordance, not a test
// hook: it is what lets a Start-menu shortcut, a secondary tile, or a "Calendar" jump-list entry
// drop the user into the grid without a click. The debug hook `MAILCAL_CALENDAR` still exists for the
// harness; this is the shipping equivalent, and both end at the same `MainWindow.ShowCalendarSurface`.
using System;
using System.Linq;

namespace Allodia.Mailcal.Services;

/// <summary>The startup switches parsed off the process command line.</summary>
internal static class StartupOptions
{
    /// <summary>Whether this launch asked to open on the calendar.</summary>
    /// <remarks>Read once from the process command line, the first launch's own arguments.</remarks>
    internal static bool CalendarAtLaunch => WantsCalendar(Environment.GetCommandLineArgs());

    /// <summary>
    /// Whether <paramref name="args"/> contains the calendar switch, in either spelling.
    /// </summary>
    /// <remarks>
    /// Split out and taking its arguments so a <b>redirected</b> launch, a shortcut clicked while the
    /// app is already running, whose command line arrives as one string on the activation, can ask
    /// the same question. Case-insensitive: a shortcut's casing is not the user's to get right.
    /// </remarks>
    internal static bool WantsCalendar(string[] args) =>
        args.Any(a =>
            a.Equals("--calendar", StringComparison.OrdinalIgnoreCase) ||
            a.Equals("/calendar", StringComparison.OrdinalIgnoreCase));

    /// <summary>
    /// Whether a redirected activation's raw argument string mentions the calendar switch.
    /// </summary>
    /// <remarks>
    /// A launch activation hands the tail of the command line as a single, unsplit string. A
    /// whitespace split is enough for a bare flag and avoids a full command-line parser for one token.
    /// </remarks>
    internal static bool WantsCalendar(string? argumentLine) =>
        !string.IsNullOrWhiteSpace(argumentLine) &&
        WantsCalendar(argumentLine.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries));
}
