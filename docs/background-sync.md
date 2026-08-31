# Background mail delivery + new-mail notifications: cross-platform contract

**Scope.** How every Allodia Mail & Calendar client keeps delivering mail when it is **not in the
foreground**, and raises a **local new-mail notification** when it finds some. On the desktop
(macOS, Windows, Linux) the process keeps running while the window is open or minimised, so the live
runtime (standing IMAP `IDLE` watches + poll timers) already delivers. On **mobile** (Android,
iOS/iPadOS) the OS suspends or kills the process shortly after it leaves the foreground, freezing
that runtime, so those clients schedule an OS background task that runs one **bounded sync pass**
through the shared core and notifies from the result.

This is also the **groundwork** for a future, opt-in, **paid push add-on**: the server-triggered
wake will call the exact same core entry point (see "Future: push" below), so nothing needs
re-architecting.

**Principle.** Background delivery is **best-effort and intermittent by design**: the OS may defer
or drop a run under battery/usage pressure, and that is acceptable. A notification deliberately
shows **content** (sender + subject); this is distinct from the *never-log-content* diagnostic-log
rule ([`logging.md`](logging.md)); the user opts into notifications and the OS hides the preview on
the lock screen per their system setting.

## The port (shared) · `crates/mailcal-bindings/src/background_sync.rs` + `crates/mailcal-app/src/background_sync.rs`

The bounded mobile entry point every client drives from its OS background mechanism is:

```
MailcalApp::run_background_sync(budget_seconds: u32) -> BackgroundSyncOutcome
```

Desktop notification hosts use the same detection path without introducing a second network
schedule:

```
MailcalApp::collect_cached_new_mail() -> BackgroundSyncOutcome
```

They call it after the live IDLE/poll runtime publishes a mailbox change. It scans only the updated
cache and shares the same persisted marks, so delivery and notification cadence remain the user's
configured live-runtime cadence.

- **Bounded + awaited.** It runs one pass of the same `App::refresh_mail` the live runtime uses
  (honouring each account's push/poll settings, the sync-depth window, and offline gating), wrapped
  in a `budget_seconds` timeout (clamped to a sane band), then returns. The host blocks on it, then
  raises notifications and marks its OS task complete.
- **New-mail detection is a persisted per-account high-water-mark.** After the pass, the core scans
  each account's **Inbox** (resolved by role) for **inbound** messages (the owner's own Sent copies
  excluded, identical to the list's "Sent" badge) received **strictly after** the stored mark, and
  returns them as `NewMailPreview`s (sender, sender name, subject, received, stable message key).
  "Received" is the delivery date, falling back to the `Date` header when the provider gave none:
  the same instant the list row is ordered by and shows, so a notification and its row can never
  disagree about when a message arrived. It
  advances the mark to the newest reported message, so nothing is ever reported twice. The mark is
  stored in the shared `preferences.toml` (`notify_marks`, RFC3339), read-modify-write like the
  other settings.
- **One notification per message, keyed by the message key.** Each client raises **one notification
  per `NewMailPreview`**, identified by its **stable message key**, grouped per account (Android
  group + summary; iOS `threadIdentifier`). Keying by message, not by account, is a contract
  requirement: because the mark advances past reported mail, a per-account notification would be
  **replaced** by the next pass's, silently losing an earlier still-unseen message. The app-icon
  badge (iOS) carries the whole pass's total, not one account's.
- **First run seeds, never floods.** An account with no mark yet is seeded to its newest existing
  Inbox message (or "now" over an empty inbox) and reports **nothing**: enabling the feature never
  notifies the whole existing inbox.
- **Reuse the live core; cold-build only when there is none.** While the process is still alive
  (Android backgrounded-but-not-killed; iOS suspended-but-resident) the foreground core already holds
  the store open, so the background task **reuses that instance** via a process-global weak handle
  (`MailcalApplication.liveCore` / `LiveCore.shared`) rather than opening a **second** engine store +
  runtime over the same SQLite file. Reuse also keeps the notify high-water-marks in one place, so no
  cross-core `preferences.toml` race. Only a genuinely cold process (the app was killed while
  backgrounded) builds a headless core with `MailcalApp::new_background_worker(...)`: it connects
  accounts and opens the store like `new_accounts`, but does **not** start the standing IDLE/poll
  runtime (one bounded pass, then quiesce). Either path is safe: `refresh_mail` drops the accounts
  read-guard before any network round-trip, and per-scope contention with a live poll is absorbed as
  a skipped (`Busy`) sync.
- **A cold core is handed the OS-secure-store writers, as constructor parameters.** A background
  pass refreshes OAuth access tokens exactly like a foreground one, so the server can hand it a
  **rotated refresh token**, and this core is dropped when the pass ends. `new_background_worker`
  therefore takes the three credential stores (Microsoft · Google · JMAP) as **required
  arguments**, not setters, so they are installed before its first connect and forgetting them is
  a compile error. Both mobile hosts *did* forget them: for as long as the cold worker has
  existed, every rotation in a cold pass was computed and silently discarded, leaving the stored
  token behind the server's. That is survivable on a provider that keeps the superseded token
  valid (Microsoft) or never rotates (Google), and **fatal** on one that treats a replay as theft
  Fastmail answers `invalid_grant — ratchet or client_id mismatch` and revokes the whole grant,
  which killed a real JMAP account. See [`provider-oauth.md`](provider-oauth.md) rule 5.

## Per-platform mechanism matrix

| Aspect | macOS · Windows | Linux | Android · WorkManager | iOS/iPadOS · BGTaskScheduler |
|---|---|---|---|---|
| Mechanism | The always-on **foreground live runtime** (IMAP `IDLE` + poll) delivers while the app runs | Same live runtime; a mailbox change triggers a cache-only new-mail scan | A `PeriodicWorkRequest` (`~15 min`, `CONNECTED`) running a `CoroutineWorker` | A `BGAppRefreshTask` (`UIBackgroundModes: fetch`, id `eu.allodia.mailcal.refresh`) driven by the SwiftUI `.backgroundTask(.appRefresh:)` handler |
| Core call | n/a (live runtime) | `collect_cached_new_mail` after the live runtime publishes `MailboxList` | `run_background_sync`: **reuses the live core** while the process is alive (weak `MailcalApplication.liveCore`), else a headless core built for the run | `run_background_sync`: **reuses the live core** while the app is resident (weak `LiveCore.shared`), else a headless core built for the run |
| Budget | n/a | n/a (no network pass) | `120 s` (worker), well under the ~10-min cap | `25 s`, under iOS's ~30 s grant |
| Skips when foregrounded | n/a | n/a (desktop live runtime) | Yes: defers to the live runtime (`MailcalApplication.isForeground`) | Naturally (iOS won't run it while active) |
| Notifications | **not yet** (follow-up) | ✅ desktop portal (`ashpd`), one per message by stable key; a summary covers previews beyond the cap | `NotificationCompat` "New mail" channel, one **per message** (grouped per account + summary), `POST_NOTIFICATIONS` | `UNUserNotificationCenter`, one **per message** (grouped per account via `threadIdentifier`), `requestAuthorization` |
| Content | n/a | sender + subject | sender + subject | sender + subject |
| User toggle | n/a | Settings → Notifications (`HostPreferences`) | Settings → "New-mail notifications" (`NotificationPrefs`, SharedPreferences) | Settings → "New-mail notifications" (`NotificationPrefs`, UserDefaults) |
| Keeping its schedule | n/a (the process is always running) | n/a (the process is always running) | Settings → "Background mail delivery" → **Allow** (`BatteryOptimization.kt`, `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`); shown only while not exempt. Without it Doze/OEM sleeping defer the pass by **hours** | Nothing to ask for: `BGTaskScheduler`'s cadence is entirely iOS's call |
| Files | (live runtime, `crates/mailcal-bindings/src/background.rs`) | `ui/host_tasks.rs`, `ui/notifications.rs`, `preferences.rs` | `MailcalApplication.kt`, `MailSyncWorker.kt`, `MailNotifier.kt`, `NotificationPrefs.kt`, `BackgroundSupport.kt` | `BackgroundSync.swift`, `MailNotifier.swift`, `NotificationPrefs.swift` |

The **toggle only gates posting**: the pass still runs and advances the marks when off, so turning
notifications off then on never floods with a backlog.

### A failed pass reports success: never `retry`, never `failure` (Android)

`MailSyncWorker` returns **`Result.success()`** when a pass throws. That looks wrong and is not:

- **`Result.retry()`** hands the *periodic* work to WorkManager's exponential backoff (30s, 1m, 2m,
  4m … doubling to a **five-hour cap**), and `runAttemptCount` resets only on a pass that succeeds. By
  the sixth consecutive failure the backoff is already **longer than the 15-minute period it
  replaced**, and it stays there until something succeeds. A phone moving between 5G, Wi-Fi and no
  coverage fails passes often enough to get there, and delivery then decays to hours apart with **no
  error raised anywhere**, because backing off is exactly what `retry()` is meant to do. **The period
  is the retry**: the next tick is 15 minutes out, sooner than any backoff worth having.
- **`Result.failure()`** is worse: for periodic work it is **terminal** and cancels every future run,
  so background mail would stop for good.

The failure is logged (`[background-worker] failed: …`) and the next tick picks it up. Pinned by
`MailSyncRetryPolicyTest`. The same reasoning applies to any future platform whose scheduler backs off
per-task rather than per-tick.

## Forcing a pass on Android, and the trap

**`adb shell cmd jobscheduler run -f … <job-id>` does not reliably run the worker.** It forces
*JobScheduler* to dispatch the job, but WorkManager then applies its **own** period check and quietly
declines:

```
WM-WorkerWrapper: Delaying execution for MailSyncWorker because it is being executed before schedule.
WM-WorkerWrapper: Status … is ENQUEUED; not doing any work and rescheduling for later execution
```

Nothing is logged under our own tag, so it reads exactly like a worker that ran and did nothing. It
*appears* to work only when the job happens to be **overdue** already, which is why it works the
first time you try it and not the second. Watch `adb logcat -s WM-WorkerWrapper` to tell the two
apart. To actually drive a pass:

- **Debug build:** broadcast to `DebugSyncReceiver` (`app/src/debug`), which enqueues a
  `OneTimeWorkRequest` and therefore has no period to be "before".
- **Release build:** wait out the real period (the job's `TIME=+…` in `dumpsys jobscheduler` says how
  long), or observe a natural wake. The `[background-worker] conditions:` line reports the gap it was
  woken after, so the log alone shows whether the OS is honouring the period.

## Testing on a physical device

Background delivery **cannot be tested on a simulator**: `BGTaskScheduler` never runs there and
local-notification banners don't render. Use a real iPhone/iPad, driven by
[`scripts/dev/device.sh`](../scripts/dev/device.sh) (the `ios-device-bgsync`
skill). The one CLI-drivable trigger is the **`MAILCAL_RUN_BGSYNC=1`** DEBUG launch hook, which runs
one `handleBackgroundRefresh()` ~6 s after launch (an lldb `_simulateLaunchForTaskWithIdentifier` does
not work over the CoreDevice tunnel from the CLI, and the Darwin `notifyutil` trigger is simulator-only).

```
scripts/dev/device.sh doctor          # device + Developer Mode (manual, on-device) + signing team
scripts/dev/device.sh all             # build + install + launch; then add a real account in the app
scripts/dev/device.sh bgsync          # 1st pass → SEEDED (no notification, by design)
# …send a new email to the account…
scripts/dev/device.sh bgsync          # next pass → DETECTED → banner on the device
```

`bgsync` prints the per-account high-water **mark before/after**, the deterministic signal that a pass
ran and detected mail (`logs` / `marks` expose the on-device `mailcal.log` / `preferences.toml`). All
of this is **Debug-build only**: `MAILCAL_RUN_BGSYNC` and the foreground presenter are `#if DEBUG`.
This tooling is iOS-device-specific and complements the simulator/emulator `debug-app` harness.

## Future: push (opt-in, paid; designed, not built)

The paid add-on turns intermittent background sync into **real-time** delivery without changing this
port:

- A server watches the mailbox (server-side IMAP `IDLE` / JMAP push) and sends a **content-free**
  wake: APNs silent (`content-available: 1`) on iOS, an FCM data message on Android ("go sync").
  The device then connects to the **user's own** mail server and fetches: **mail content never
  touches Allodia's push server**, so the design stays sovereignty-preserving even in the paid tier.
- The push handler calls the **same `run_background_sync`** → identical path to the local schedulers
  built now.
- Not built here: the server, APNs (`aps-environment` entitlement + `registerForRemoteNotifications`
  + a real provisioning profile / app-group for a Notification-Service-Extension), FCM (dependency +
  token registration), billing/entitlement, and a `JurisdictionGate` review of routing the wake
  signal. The **local background sync stays free and always-on for everyone**; push is the opt-in
  upgrade. The free/paid boundary this sits inside is [`pledge.md`](pledge.md).

## Known gaps / follow-ups

- **macOS and Windows new-mail notifications aren't built.** They deliver in real time while
  running but do not raise a system notification yet; wiring their live runtime's new-mail signal to
  `UNUserNotificationCenter` / Windows toasts is a follow-up. Linux exercises the shared cache-only
  detection seam through its desktop portal adapter; its GNOME-runtime AT-SPI run proves that the
  disabled state emits nothing and enabling it posts only newly arrived mail.
- **iOS app-group store is deferred.** `BGAppRefreshTask` runs in the main app process, so it needs
  no app group. A future push Notification-Service-Extension will need one. That, and the app-group
  store move it forces, land with the paid push add-on and real signing, since an app-group
  entitlement would break the current ad-hoc simulator signing.
- **Android notification *content* isn't localised.** The Settings copy (the toggle and Android's
  battery-exemption card) comes from the shared catalog on Android, iOS, and Linux
  (`settings_notifications_*` / `settings_battery_*`); Linux's portal content is catalog-backed too.
  The Android notification channel name and notification texts themselves ("N new messages",
  "+N more") are still hardcoded English: they render in a background context where the per-app
  locale needs separate handling.
- **Android delivery is subject to OEM battery policies** and iOS cadence is entirely OS-controlled,
  both accepted (best-effort by design). Measured on a real Samsung (S24 Ultra, One UI / Android 16,
  five days of the on-device log): while the phone is **awake** the pass lands on its 15-minute period
  to the second; once it is **idle in a pocket** Doze + One UI's app sleeping stretch that to **1–3
  hours**, and **~11 hours overnight**. The scheduling is not at fault and there is nothing to fix in
  the worker: Android is simply choosing not to wake an app it has not exempted. The **battery
  exemption prompt** (below) is the only lever the platform offers; a user who declines it keeps the
  best-effort cadence. **Do not diagnose this as a bug in `MailSyncScheduler`**: the OS job history
  (`adb shell dumpsys jobscheduler | grep -A25 MailSyncWorker`) and the app's own
  `[background-worker]` log lines settle it in minutes.
- **The exemption is a prompt, not a guarantee** (Android · `BatteryOptimization.kt`). Settings shows
  a **Background mail delivery** card *only while the app is not exempt*, explaining plainly that
  Android is postponing its checks, with one **Allow** button raising the system dialog
  (`ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`, with the `package:` URI; without it the system
  shows the list of *every* app instead of a prompt for this one). Once granted the card disappears;
  the user can revoke it in Settings at any time and the sync degrades, it never breaks.
  **Play Store:** this permission has a restricted acceptable-use list; keeping mail delivered in the
  background is the app's core function, which is the declared basis. If that ever fails review, the
  fallbacks are `ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS` (deep-links to the list, needs no
  permission, more friction) or a foreground service, which is what **Thunderbird** does, and why its
  jobs keep their network while ours are slept: its process sits at `procState=FGS`.
- **iOS has no equivalent lever.** `BGTaskScheduler`'s cadence is entirely at iOS's discretion and
  there is nothing to ask the user for, so the Android exemption has no iOS counterpart by design.

## Enforcement

When you change background delivery or new-mail notifications:

1. Update this document (the port, the per-platform matrix, and known gaps) **and** the capability
   matrix in [`../README.md`](../README.md) in the same change.
2. Keep the core contract identical across platforms: the same `run_background_sync` port, the
   high-water-mark dedupe + first-run seeding, the inbound-Inbox-only semantics, and the notification
   content policy.
3. A new platform ships background delivery meeting this bar **before** it ships to users; any
   shortfall goes under "Known gaps" with a follow-up, never left silent.
