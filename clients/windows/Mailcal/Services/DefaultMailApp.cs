// Becoming the OS's default mail app on Windows: what this build can do about it, and where the
// user has to be sent to do it.
//
// The *policy*, when to ask and remembering the answer, is the shared core's
// (ShouldOfferDefaultMailApp / RecordDefaultMailAppOffer). This file is only the platform half,
// and it is deliberately the decidable part of it: WinUI- and WinRT-free, so `Mailcal.Tests` can
// link it, the same split MailLink keeps from MainWindow.MailLink.cs. The launch itself needs
// WinRT and lives in MainWindow.DefaultMailApp.cs.
//
// Contract: docs/os-integration.md.
using System;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Making this app the machine's mail handler, as far as Windows permits.</summary>
internal static class DefaultMailApp
{
    /// <summary>
    /// What this build can do: open the OS's own settings, and nothing more.
    /// </summary>
    /// <remarks>
    /// Windows has had no API to set a default handler since Windows 10, deliberately: the choice
    /// is the user's, and an app may only ask to be considered. Declaring the <c>mailto</c>
    /// protocol in <c>Package.appxmanifest</c> is what puts this app in the list; the deep link
    /// below is what takes the user to it.
    /// </remarks>
    internal const DefaultMailAppSupport Support = DefaultMailAppSupport.OpenSettings;

    /// <summary>
    /// Whether this app is already the mail handler, or <c>null</c> when that cannot be told.
    /// </summary>
    /// <remarks>
    /// Always <c>null</c>. Reading the current association means reading
    /// <c>HKCU\…\UrlAssociations\mailto\UserChoice</c>, which is deliberately tamper-protected and
    /// whose shape is undocumented and has changed between Windows versions; a wrong answer here
    /// would either suppress the offer for someone who needs it or repeat it for someone who does
    /// not. The core treats an unknown answer as "not the default", which is the recoverable way
    /// round: at worst the offer appears once to someone who has already accepted.
    /// </remarks>
    internal static bool? IsDefault => null;

    /// <summary>
    /// The Settings page to send the user to, deep-linked to this app where Windows supports it.
    /// </summary>
    /// <remarks>
    /// <c>registeredAUMID</c> is the parameter for a packaged (MSIX) app; the other two
    /// (<c>registeredApp</c>, <c>registeredAppUser</c>) are for an installer that writes its own
    /// <c>RegisteredApplications</c> key, which this app does not have. Supported from Windows 11
    /// 21H2 with the 2023-04 cumulative update; an older build ignores the query string and opens
    /// the Default apps page itself, which is a worse landing but not a broken one.
    /// <para>
    /// A blank <paramref name="aumid"/> (the unpackaged dev build, which has no package identity)
    /// falls back to that same plain page: a deep link naming nothing lands nowhere.
    /// </para>
    /// </remarks>
    internal static string SettingsUri(string? aumid) =>
        string.IsNullOrEmpty(aumid)
            ? "ms-settings:defaultapps"
            : $"ms-settings:defaultapps?registeredAUMID={Uri.EscapeDataString(aumid)}";

    /// <summary>What to do about the offer right now.</summary>
    internal enum Timing
    {
        /// <summary>Nothing: the core says the question is settled, or too early to ask.</summary>
        NotDue,

        /// <summary>Not now, because something the user aimed at owns the screen.</summary>
        WaitForScreen,

        /// <summary>Not now, because there is nowhere on screen to put it yet.</summary>
        WaitForVisualTree,

        /// <summary>Ask.</summary>
        Ask,
    }

    /// <summary>
    /// Whether the offer may be put, given what else the window is doing.
    /// </summary>
    /// <remarks>
    /// Here, rather than inline at the one call site, because each of the three refusals was
    /// learnt the hard way and only this file can be unit-tested. In order:
    /// <list type="bullet">
    /// <item>a dialog already showing, or a share or mail link the user did aim at us, would get
    /// this offer dropped and recorded as declined, and it is put once;</item>
    /// <item><paramref name="rooted"/> false is the crash. The signal that asks this is raised
    /// while the window is still being constructed, so an account that connects in tens of
    /// milliseconds arrives before the window has a visual tree, and showing a dialog on an
    /// unrooted element throws out of an <c>async void</c>, which reaches no catch and takes the
    /// process down.</item>
    /// </list>
    /// Both refusals are "wait", never "no": the caller comes back on the next signal that makes
    /// them false, so a deferred offer is put, not lost.
    /// </remarks>
    internal static Timing WhenToAsk(bool due, bool screenTaken, bool rooted) =>
        !due ? Timing.NotDue
        : screenTaken ? Timing.WaitForScreen
        : !rooted ? Timing.WaitForVisualTree
        : Timing.Ask;
}
