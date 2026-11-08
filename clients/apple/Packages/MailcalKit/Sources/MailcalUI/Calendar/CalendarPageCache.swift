// What the grid has looked up, held until the calendar changes.
//
// The grid is drawn from the strip position, and SwiftUI evaluates its body on every frame of a
// scroll, a flick's landing and a pinch. A page pulled there is laid out by the core and copied
// across the FFI each time, several weeks per frame, which is the frame. So a page (and the week the
// grid builds from it) is looked up once per calendar version and display zone, the two things that
// change what the core would answer, and every frame after that reads it from here (docs/calendar.md
// §7, "Compose the neighbouring pages").

import Foundation
import MailcalBindings

/// Values held until the calendar version or the display zone changes, then all dropped at once.
final class CalendarCache<Key: Hashable, Value> {
    private struct Stamp: Equatable {
        let version: Int
        let zone: String
    }

    /// Enough for the weeks on screen, a flick's worth either side, and a pinch out to the whole week,
    /// without holding every week a long scroll has passed through.
    private static var capacity: Int { 32 }

    private var stamp: Stamp?
    private var values: [Key: Value] = [:]

    /// The value for `key`, made with `make` only when none is held for this `version` and `zone`.
    /// Nothing is held when `make` has nothing to give, so the next frame asks again.
    func value(_ key: Key, version: Int, zone: String, make: () -> Value?) -> Value? {
        let current = Stamp(version: version, zone: zone)
        if stamp != current {
            stamp = current
            values.removeAll(keepingCapacity: true)
        }
        if let held = values[key] { return held }
        guard let value = make() else { return nil }
        if values.count >= Self.capacity { values.removeAll(keepingCapacity: true) }
        values[key] = value
        return value
    }
}

/// A page the core laid out: `columns` days from `from`.
struct CalendarPageKey: Hashable {
    let from: String
    let columns: Int
}

/// A week of the strip: its index, and the day it starts.
struct CalendarWeekKey: Hashable {
    let index: Int
    let anchor: Date
}
