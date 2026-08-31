// The display-timezone UI: a zone-aware timestamp formatter, the selector menu, and the
// "your device moved zones" prompt. Split out of Mailcal.swift to keep files under
// 500 lines. The active zone is a core app preference (the engine resolves instants; the
// host localises them), so all of these take/route through an IANA zone id.

#if os(macOS)
import AppKit
#endif
import MailcalBindings
import SwiftUI

/// The shared ISO-8601 parser. `DateFormatter`/`ISO8601DateFormatter` are very expensive to
/// construct (they load ICU locale/tz data) but cheap and thread-safe to *use* once
/// configured, so they are created once and reused. Building a fresh pair per call was the
/// dominant cost janking list scroll, since `localDateTime` runs once per visible row on
/// every render.
private let isoParser: ISO8601DateFormatter = {
    let iso = ISO8601DateFormatter()
    iso.formatOptions = [.withInternetDateTime]
    return iso
}()

/// Cached display formatters keyed by IANA zone (the zone rarely changes, so this stays a
/// tiny map). Accessed only from the main thread (SwiftUI bodies + tap handlers).
private var zoneFormatters: [String: DateFormatter] = [:]

/// The reused display formatter for `zone`, built once and cached.
private func displayFormatter(for zone: String) -> DateFormatter {
    if let cached = zoneFormatters[zone] {
        return cached
    }
    let output = DateFormatter()
    output.dateFormat = "yyyy-MM-dd HH:mm"
    output.timeZone = TimeZone(identifier: zone) ?? .current
    zoneFormatters[zone] = output
    return output
}

/// Formats an engine timestamp for display in `zone` (an IANA id). A `Z`-suffixed UTC
/// instant (mail `received_at`, a resolved event start) is converted to `zone`; a naive
/// wall-clock (a floating event) is shown as-is; a bare date is shown as the date. The
/// view-model is tzdata-free, so this host-side conversion is where "shown in your chosen
/// time zone" happens.
func localDateTime(_ raw: String, in zone: String) -> String {
    if raw.isEmpty { return "" }
    if raw.hasSuffix("Z"), let date = isoParser.date(from: raw) {
        return displayFormatter(for: zone).string(from: date)
    }
    // A naive wall-clock "YYYY-MM-DDTHH:MM:SS" → "YYYY-MM-DD HH:MM"; else a date.
    if raw.count >= 16, raw.contains("T") {
        return String(raw.prefix(16)).replacingOccurrences(of: "T", with: " ")
    }
    return String(raw.prefix(10))
}

/// A `Z`-suffixed engine instant as a `Date`, or nil when the value is not one (a floating
/// wall-clock, a bare date, or an unparseable string). Lives here to share the cached parser
/// above, building an `ISO8601DateFormatter` per call is the cost that file header warns about.
func parseUtcInstant(_ raw: String) -> Date? {
    guard raw.hasSuffix("Z") else { return nil }
    return isoParser.date(from: raw)
}

/// Cached relative-label formatters keyed by "zone|pattern", same rationale as
/// `zoneFormatters`: building a `DateFormatter` per visible row on every render janks scroll.
/// Accessed only from the main thread (SwiftUI bodies).
private var relativeFormatters: [String: DateFormatter] = [:]

/// The reused, locale-aware formatter for `pattern` in `zone`, built once and cached.
private func relativeFormatter(_ pattern: String, for zone: String) -> DateFormatter {
    let key = "\(zone)|\(pattern)"
    if let cached = relativeFormatters[key] {
        return cached
    }
    let output = DateFormatter()
    output.locale = L10n.appLocale
    output.dateFormat = pattern
    output.timeZone = TimeZone(identifier: zone) ?? .current
    relativeFormatters[key] = output
    return output
}

/// The date-format pattern a `relativeDate` label uses for a message `dayDiff` calendar days in the
/// past (0 = today), in `sameYear` as now: today → the clock, the previous six days → short weekday,
/// this year → day + month, older → with the year. Day 7 falls to the date on purpose, it is the
/// same weekday as today. Pure (no clock, no tz), so the shared relative-label policy is
/// unit-testable, see docs/timestamps.md, mirrored on Android and Windows.
func relativeDatePattern(dayDiff: Int, sameYear: Bool) -> String {
    switch dayDiff {
    case 0: return "HH:mm"
    case 1...6: return "EEE"
    default: return sameYear ? "d MMM" : "d MMM yyyy"
    }
}

/// A compact, Thunderbird-style relative timestamp for a list row, in `zone` (an IANA id): today →
/// time, the previous six days → short weekday, this year → day + month, older → with the year.
/// Falls back to `localDateTime` for a naive/unparseable value; the reading view keeps the full
/// `localDateTime`. Mirrors Android's `relativeDate` and Windows's `RelativeDate`
/// (docs/timestamps.md). The time-of-day stays 24-hour, as `localDateTime` is, the 12/24h clock
/// setting reaches the mail list on Android only (a documented gap).
func relativeDate(_ raw: String, in zone: String) -> String {
    guard raw.hasSuffix("Z"), let date = isoParser.date(from: raw) else {
        return localDateTime(raw, in: zone)
    }
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: zone) ?? .current
    let now = Date()
    let dayDiff = calendar.dateComponents(
        [.day],
        from: calendar.startOfDay(for: date),
        to: calendar.startOfDay(for: now)
    ).day ?? 0
    let sameYear = calendar.component(.year, from: date) == calendar.component(.year, from: now)
    return relativeFormatter(relativeDatePattern(dayDiff: dayDiff, sameYear: sameYear), for: zone)
        .string(from: date)
}

/// The engine's authoritative IANA zone list, the bundled tzdb it localises against,
/// shared by every client (macOS/Android/Windows) over the FFI instead of each host's OS
/// zone set (which on Windows collapses cities like Europe/Amsterdam into one zone). A
/// Swift file-scope `let` is initialised lazily, exactly once, so the FFI is called once.
private let engineZones = availableTimeZones()

/// A compact selector for the active display zone: a menu of every IANA zone the engine
/// can localise against. Picking one dispatches `setTimeZone` so the core re-orders the agenda.
struct TimeZonePicker: View {
    let active: String
    let onSelect: (String) -> Void

    var body: some View {
        Picker(
            L10n.tz_picker_label(),
            selection: Binding(get: { active }, set: { onSelect($0) })
        ) {
            // The active zone may be one the list does not carry (rare); include it so the
            // menu always reflects the real selection.
            if !engineZones.contains(active) {
                Text(active).tag(active)
            }
            ForEach(engineZones, id: \.self) { id in
                Text(id).tag(id)
            }
        }
        .pickerStyle(.menu)
        .labelsHidden()
        .frame(maxWidth: 240)
    }
}

/// The app-language override, persisted natively: the user's choice is written to the
/// `AppleLanguages` user-default (the OS's per-app language mechanism), so the OS selects
/// the matching `L10n` table on the next launch. `system` clears the override and follows
/// the OS language. The OS language is always the default until a choice is made here.
enum LanguageOverride: Hashable {
    case system
    case code(String)

    /// The locales the app ships a language picker for, the catalog's locales, straight from
    /// the generated `L10n`, so adding a language to `messages/` adds it to the picker.
    static let codes = L10n.locales
    private static let key = "AppleLanguages"

    /// The current override, read from the `AppleLanguages` default (`system` if unset or
    /// not one of our shipped locales).
    static func current() -> LanguageOverride {
        guard let first = (AppPrefs.defaults.array(forKey: key) as? [String])?.first,
            codes.contains(first)
        else { return .system }
        return .code(first)
    }

    /// Applies the override: writes (or clears) `AppleLanguages`. Takes effect on relaunch.
    func apply() {
        switch self {
        case .system:
            AppPrefs.defaults.removeObject(forKey: Self.key)
        case let .code(code):
            AppPrefs.defaults.set([code], forKey: Self.key)
        }
    }
}

/// A compact language selector mirroring the time-zone picker: System, then one row per
/// language the catalog ships, each labelled with its own endonym ("Deutsch", never "German").
/// Selecting a language writes the native per-app override; the change applies after a
/// restart (the OS resolves the active language at launch).
struct LanguagePicker: View {
    @State private var selection = LanguageOverride.current()
    @State private var pendingRestart = false

    var body: some View {
        Picker(
            L10n.settings_language_heading(),
            selection: $selection
        ) {
            Text(L10n.settings_language_system()).tag(LanguageOverride.system)
            ForEach(LanguageOverride.codes, id: \.self) { code in
                Text(L10n.languageName(code)).tag(LanguageOverride.code(code))
            }
        }
        .pickerStyle(.menu)
        .labelsHidden()
        .frame(maxWidth: 200)
        // `AppleLanguages` is read once, at launch, so a change only takes effect after a
        // restart: apply it, then prompt (and offer to relaunch right away).
        .onChange(of: selection) { _, choice in
            choice.apply()
            pendingRestart = true
        }
        .alert(L10n.settings_language_restart_title(), isPresented: $pendingRestart) {
            Button(L10n.settings_language_restart_now()) { relaunch() }
            Button(L10n.settings_language_restart_later(), role: .cancel) {}
        } message: {
            Text(L10n.settings_language_restart_message())
        }
    }

    /// Relaunches the app so the new `AppleLanguages` value is picked up at startup. The
    /// replacement process inherits this one's environment (e.g. `DYLD_LIBRARY_PATH`, which
    /// locates the cdylib) and reads the just-written language from user defaults.
    private func relaunch() {
        #if os(macOS)
        guard let executable = Bundle.main.executableURL else { return }
        let task = Process()
        task.executableURL = executable
        try? task.run()
        NSApp.terminate(nil)
        #endif
        // iOS/iPadOS apps can't relaunch themselves; the new language applies on the next
        // (manual) launch, the restart alert already asks the user to reopen the app.
    }
}
