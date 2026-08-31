# Timestamp display: cross-platform contract

**Scope.** How every Allodia Mail & Calendar client turns an engine instant into the date/time a
user reads in the **mail list** and the **reading header**. The core is tzdata-free: it emits a
UTC ISO instant (`...Z`) or a naive wall-clock and nothing else; each client localises. This
contract fixes the two rendering shapes so the app never disagrees with itself across platforms.

**Principle.** The **list row** shows a compact, relative label; the **reading header** shows the
full absolute date. One policy, every client: it is decided **here**, never per client, because a
support answer ("the date on the row is today, so it shows the time") must hold everywhere.

**Why it is client-side, not core.** Deciding "today vs this week vs older" needs the user's active
time zone *and* the client's "now" to convert the UTC instant to a local calendar day, and the
core carries no tzdata by design (it resolves instants; the host localises). Weekday and month
names are locale text the core also does not hold. So the bucketing and the formatting are both the
client's job; the core's contribution is the unambiguous UTC instant.

## The two shapes

| Surface | Shape | Rule |
|---|---|---|
| **List row** (flat row, thread row, thread sub-row) | Relative label | **today** → time (`09:05`) · **previous six days** → short weekday (`Fri` / `vr`) · **this year** → day + month (`3 Jul` / `3 jul`) · **older** → day + month + year (`3 Jul 2025`). A naive/unparseable value falls back to the absolute shape. |
| **Reading header** (the opened message's date) | Absolute | The full localised date + time (`2026-07-20 09:05`), in the active display zone. |

**Day 7 is a date, not a weekday**, on purpose: it is the same weekday as today, so `Mon` for it
would read as *this* Monday. The relative window is deliberately the previous **six** days only.

The relative label's **time-of-day** is 24-hour today, matching each platform's absolute
`localDateTime`. The 12/24-hour clock **setting** ([`settings.md`](settings.md) → General) reaches
the mail list on **Android only**. See Known gaps.

## The names come from the app's language, not the host's

The weekday and month in these labels are **copy**, and they follow the app's own **Language**
setting ([`settings.md`](settings.md) → General), the same setting that picks the chrome. A client
that resolves the two independently will disagree with itself the moment they differ, and the
failure is silent: nothing crashes, the app is simply Dutch with English dates.

That is not hypothetical. Every client applies the language choice through a *resource-lookup*
mechanism, and each platform has a *separate* notion of formatting locale that the mechanism does
not touch:

| Platform | Language choice applies via | Formatting locale must come from |
|---|---|---|
| Android | `AppCompatDelegate.setApplicationLocales` | the same `configuration.locales[0]` every screen already reads |
| macOS / iOS / iPadOS | the `AppleLanguages` default → `L10n.current()` over `Locale.preferredLanguages` | **not** `Locale.current`: the bundle ships no `nl.lproj` (l10n is generated Swift), so the OS never resolves the app to Dutch and `Locale.current` stays English |
| Windows | an MRT-Core `ResourceContext` qualifier (`L10n.SetLanguage`) | **not** `CultureInfo.CurrentCulture`: that follows the OS *regional format*, a setting the picker does not write. `AppCulture.Apply` pins it |
| Linux | generated `l10n::active_locale()` lookup | the same resolved locale selects the bundled weekday and month names |

So: **resolve the language once, and format dates against that same resolution.** A `system` choice
follows the host on every platform: the host's own locale is then the single source, and there is
nothing to disagree with.

This binds the **calendar** identically (its period title and weekday headers are the same kind of
copy). See [`calendar.md`](calendar.md).

## Per-platform

| Platform | Relative (list) | Absolute (reading header) | Names follow app language | Where |
|---|---|---|:---:|---|
| Android | `relativeDate` → `relativeDatePattern` | `localDateTime` | ✅ | `clients/android/.../MailDialogs.kt` |
| macOS / iOS / iPadOS | `relativeDate` → `relativeDatePattern` | `localDateTime` | ✅ | `clients/apple/.../TimeZoneViews.swift`, `L10n.appLocale` |
| Windows | `TimeZones.RelativeDate` → `RelativePattern` | `TimeZones.LocalDateTime` | ✅ | `clients/windows/Mailcal/Services/{TimeZones,AppCulture}.cs` |
| Linux | `relative_date` → `relative_date_pattern` | `local_date_time` | ✅ | `clients/linux/src/ui/timestamps.rs` |

Each client implements the same policy in its own language; the pure bucket-selection
(`relativeDatePattern` / `RelativePattern` / `relative_date_pattern`) is factored out so it is
unit-tested rather than trusted (`RelativeDateTest.kt` runs in the Android JVM gate;
`RelativeDateTests.swift` / `RelativeDateTests.cs` in the Apple / Windows suites; Linux tests the
seam in `timestamps.rs`).

## Known gaps

- **The 12/24-hour clock setting does not reach the mail list on macOS/iOS/Windows/Linux.** Their
  list time-of-day is fixed 24-hour (as their absolute formatter is); only Android threads the core
  `use24Hour` setting into the row's today→time label. The calendar honours the setting on every
  platform. Closing this on the mail list is a follow-up.

## Enforcement

When you change how a client formats a mail timestamp:

1. Keep the two shapes intact: a **relative** label on the list row, the **full absolute** date in
   the reading header. Apply any policy change to **every** platform in the same change (or
   record the shortfall under Known gaps, never silently).
2. Change the bucket policy only in the pure `relativeDatePattern` / `RelativePattern` /
   `relative_date_pattern` seam, and update its unit test in the same change: the policy is a
   check that must be able to fail.
