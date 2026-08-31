---
name: ios-device-bgsync
description: Test Allodia Mail background sync + new-mail notifications on a PHYSICAL iPhone/iPad, where BGTaskScheduler actually runs and a notification can be seen (the simulator can't do either). Wraps build/install/launch/log-pull and a seed→detect loop via scripts/dev/device.sh. Use when verifying or debugging background delivery, local notifications, or anything needing a real iOS device rather than a simulator.
---

# ios-device-bgsync: background sync + notifications on a real iOS device

The simulator-only `debug-app` skill (`scripts/dev/*`) can't test background delivery: `BGTaskScheduler`
never runs on a simulator, and local-notification banners don't render there. This skill drives a
**physical iPhone/iPad** through the whole loop with one script:

```
scripts/dev/device.sh <doctor|build|install|run|logs|marks|bgsync|all>
```

It encodes the gotchas that otherwise cost a round of trial-and-error (see `docs/background-sync.md`
→ "Testing on a physical device"): the right signing team (auto-derived), the Developer-Mode
precondition, terminate-before-launch (so a fresh `onAppear` fires the launch hook), and pulling the
on-device log + `preferences.toml` to verify the mark moved.

## 0. Preconditions (one-time, mostly manual: surfaced by `doctor`)

```
scripts/dev/device.sh doctor
```

- **Device connected + paired** (USB or same-network Wi-Fi). Auto-detected; override with
  `MAILCAL_DEVICE=<udid>` if more than one is attached.
- **Developer Mode ON** (on the device: *Settings → Privacy & Security → Developer Mode → On →
  restart → confirm*). This is manual and needs a reboot; it **cannot** be toggled from the CLI, and
  without it install fails. `doctor` flags it.
- **Device registered in the dev account** + an *Apple Development* signing identity in the keychain.
  The team id is auto-derived from the cert's OU (override with `DEVELOPMENT_TEAM=<id>`). The device
  must be added to the team's devices (do it once in Xcode, or it happens on the first device build).

## 1. Build, install, launch

```
scripts/dev/device.sh all        # build + install + fresh launch
# or step by step:
scripts/dev/device.sh build       # add --core to rebuild the Rust XCFramework
scripts/dev/device.sh install
scripts/dev/device.sh run         # terminate + FRESH launch
```

(`clients/apple/Scripts/build-and-run.sh --iphone` does build + install + launch too: it targets a
connected device by itself, and rebuilds the core every time. Use it to *run* the app; the verbs
below are what this loop needs beyond that.)

On a **fresh install there is no account**: the app opens on the setup screen. Add a real IMAP/JMAP
account **in the app** (only you can enter credentials; they live in the device Keychain). Accept the
notification-permission prompt that appears **after the first account connects** (no prompt = no
banners). Confirm it synced: `... logs --grep 'refresh_mail: syncing'` should show `1 account(s)`.

## 2. The background-sync test loop

`bgsync` runs one bounded background pass on the device via the `MAILCAL_RUN_BGSYNC=1` DEBUG launch
hook (the only CLI-drivable trigger: `BGTaskScheduler` can't be simulated over the CoreDevice tunnel
from the CLI, and the Darwin `notifyutil` trigger is simulator-only). It reports the per-account
high-water **mark before/after**, which makes the outcome unambiguous:

```
scripts/dev/device.sh bgsync      # 1st time  → RESULT: SEEDED  (no notification, by design)
# ...send a new email to the account (from anywhere)...
scripts/dev/device.sh bgsync      # next time → RESULT: DETECTED → banner on the device
```

- **SEEDED**: first pass per account, sets the mark to the newest existing message and reports
  nothing (so enabling the feature never floods the existing inbox). No banner is expected.
- **DETECTED**: a mark advanced, so new inbound Inbox mail was found and `notifyNewMail` fired.
  **Watch the device**: the banner shows even in-foreground via the DEBUG foreground presenter.
- **no mark change**: no new mail since the last pass, or the pass didn't run. Check
  `... logs --grep 'session start|refresh_mail'`: a new `session start` line per run confirms the
  fresh launch (and hence the trigger) fired.

## 3. Observe

```
scripts/dev/device.sh logs [--grep RE]   # pull & print the on-device mailcal.log
scripts/dev/device.sh marks              # print notify_marks (the high-water marks)
```

There is **no CLI screenshot** for a physical device (unlike simulators); for the banner itself,
look at the device. The log + marks give the deterministic, machine-checkable signal.

## Notes

- Everything here is **Debug-build only**: `MAILCAL_RUN_BGSYNC` and the foreground presenter are
  `#if DEBUG`, stripped from release.
- This complements, not replaces, the `debug-app` skill: use that for macOS/simulator/Android UI
  work against the Stalwart harness; use this when you specifically need a real iOS device.
- The same core path serves the live app (warm reuse) and a cold worker; see the `#1`/`#3` notes in
  `docs/background-sync.md`. On device, `bgsync` while foregrounded exercises the **live-core reuse**
  path (no second store opened).
