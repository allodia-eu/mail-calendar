// Turning the core's resolved calendar colours into SwiftUI colours.
//
// The client picks the theme and nothing else. The core resolves each calendar to a light and a dark
// swatch, fill, label, border, and the label is already guaranteed to clear WCAG AA against its
// fill, so no client ever computes contrast and three clients cannot disagree about whether a chip is
// readable. All that is left here is parsing `#rrggbb` and choosing a theme.

import MailcalBindings
import SwiftUI

/// The chip a block falls back to when its calendar is missing from the page's list. That should not
/// happen, the core lists every calendar it draws from, but a block with no colour at all would be
/// invisible, which is a worse failure than a grey one.
private let fallbackSwatch = Swatch(background: "#6b7280", text: "#ffffff", border: "#4b5563")

/// Parses the core's `#rrggbb`, falling back to a visible grey rather than drawing nothing.
func parseHexColor(_ hex: String) -> Color {
    var body = hex
    if body.hasPrefix("#") { body.removeFirst() }
    guard body.count == 6, let value = UInt32(body, radix: 16) else {
        return Color(red: 0.42, green: 0.45, blue: 0.50)
    }
    return Color(
        red: Double((value >> 16) & 0xFF) / 255.0,
        green: Double((value >> 8) & 0xFF) / 255.0,
        blue: Double(value & 0xFF) / 255.0
    )
}

extension CalendarColor {
    /// The swatch to draw this calendar with in the active theme.
    func swatch(dark: Bool) -> Swatch { dark ? self.dark : self.light }
}

extension Array where Element == CalendarRow {
    /// The calendar a block belongs to, for its colour *and* its spoken label ("…, Work").
    ///
    /// Keyed on account **and** id: a provider key is unique only within its account, so two accounts
    /// can mint the same calendar id, and matching on the id alone would paint one account's event in
    /// the other's colour.
    func row(account: String, calendar: String) -> CalendarRow? {
        first { $0.account == account && $0.id == calendar }
    }
}

extension Optional where Wrapped == CalendarRow {
    /// The swatch for a looked-up calendar, or the grey fallback when the page didn't list it.
    func swatchOrFallback(dark: Bool) -> Swatch {
        self?.color.swatch(dark: dark) ?? fallbackSwatch
    }
}

/// An event block's colours (`calendarEventSwatch`), parsed. The look of Apple's Calendar: a light
/// tint fills the block, the calendar's colour runs down its leading edge, and the title is a deep
/// shade of it, pale on dark. The core resolves the hexes, contrast included; this only parses them.
struct EventColors: Sendable {
    let fill: Color
    let accent: Color
    let text: Color
}

/// The event colours a calendar's `hex` resolves to in a theme, asked of the core once per colour
/// and theme rather than on every frame a block is drawn.
func eventColors(hex: String, dark: Bool) -> EventColors {
    eventColorCache.colors(hex: hex, dark: dark)
}

extension Optional where Wrapped == CalendarRow {
    /// The event colours for a looked-up calendar, or the fallback grey's when the page didn't list it.
    func eventColorsOrFallback(dark: Bool) -> EventColors {
        eventColors(hex: self?.color.hex ?? fallbackSwatch.background, dark: dark)
    }
}

private final class EventColorCache: @unchecked Sendable {
    private let lock = NSLock()
    private var cached: [String: EventColors] = [:]

    func colors(hex: String, dark: Bool) -> EventColors {
        let key = "\(hex)|\(dark)"
        lock.lock()
        defer { lock.unlock() }
        if let held = cached[key] { return held }
        let swatch = calendarEventSwatch(hex: hex, dark: dark)
        let colors = EventColors(
            fill: parseHexColor(swatch.fill),
            accent: parseHexColor(swatch.accent),
            text: parseHexColor(swatch.text)
        )
        cached[key] = colors
        return colors
    }
}

private let eventColorCache = EventColorCache()
