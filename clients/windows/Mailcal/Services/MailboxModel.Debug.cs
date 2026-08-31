// The DEBUG-only launch-hook half of MailboxModel (split out to keep each file under the
// 500-line limit, and so this whole verification surface compiles out of Release). Reads
// MAILCAL_* environment variables at launch and drops the app into a known state without
// pixel-tapping, the reliable, layout-independent control primitive the debug-app tooling
// prefers. The Windows counterpart of the Apple client's hooks in Mailcal.swift
// (MAILCAL_OPEN_FIRST / MAILCAL_CALENDAR); driven from scripts/dev/control.sh windows (which
// relaunches the built exe with the env set, see clients/windows/control.ps1). Debug-build
// only; never present in a release binary.

#if DEBUG
namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // One-shot guards so a hook fires once per launch, even though Reload runs on every snapshot
    // (and ShowCalendar/OpenMessage themselves dispatch intents that trigger another Reload).
    private bool _hookCalendarApplied;
    private bool _hookOpenFirstApplied;

    private static bool HookOn(string name) =>
        Environment.GetEnvironmentVariable(name)?.Trim() == "1";

    /// <summary>
    /// Applies any pending MAILCAL_* launch hook, called at the tail of each <see cref="Reload"/>
    /// so it fires once the relevant surface has populated (e.g. the first row exists before
    /// <c>MAILCAL_OPEN_FIRST</c> can open it, mirroring the Apple client's onChange(rows.count)).
    /// Each hook is one-shot; a later refresh won't re-fire it. A no-op in Release (the
    /// implementing declaration is compiled out, so the call site in Reload is elided).
    /// </summary>
    partial void ApplyLaunchHooks()
    {
        // The calendar hook doesn't depend on rows; apply it as soon as we reach a reload.
        if (!_hookCalendarApplied && HookOn("MAILCAL_CALENDAR"))
        {
            _hookCalendarApplied = true;
            Log.Info("launch hook: MAILCAL_CALENDAR -> showing calendar");
            ShowCalendar();
            return; // the calendar replaces the mail detail, don't also open a message under it
        }
        // Open the first message once a row is actually loaded.
        if (!_hookOpenFirstApplied && HookOn("MAILCAL_OPEN_FIRST") && Rows.Count > 0)
        {
            _hookOpenFirstApplied = true;
            Log.Info("launch hook: MAILCAL_OPEN_FIRST -> opening the first row");
            OpenMessage(Rows[0]);
        }
    }
}
#endif
