// The binding a `.sheet(item:)` hands its content, and the one rule it has to obey: a sheet that
// has been dismissed must stay dismissed.
//
// Written as a free function over `Binding` rather than inline at the call site so `swift test`
// can drive it: what that pins is the binding's own rule, with no sheet in the picture. The
// presentation itself is driven by `ComposerPresentationTests` in the UI suite
// (`clients/apple/UITests`), which opens the composer, dismisses it and opens it again, because a
// binding that is never cleared and one that is look identical until the control is used twice.

import SwiftUI

/// A binding into the item a sheet is presenting, which **cannot bring the sheet back**.
///
/// `.sheet(item:)` keeps its content alive while the sheet animates away, and that content is
/// still writing: a `TextField` commits its value, a `@FocusState` tears down, a `DatePicker`
/// settles. Those writes land *after* the item was cleared. A binding that read through to the
/// value the sheet was presented with would hand that value straight back to storage, the item
/// would be non-`nil` again, and `.sheet(item:)` would present it a second time. On iOS that is
/// exactly what happened: Cancel and Save both dismissed the event editor and both immediately
/// reopened it.
///
/// So the read falls back to `presented` and the write does not. The fallback is still needed:
/// SwiftUI evaluates a sheet's content closure on the frame where the item has just been cleared,
/// and force-unwrapping there traps.
///
/// `presented` is the value `.sheet(item:)` passed to its content closure.
///
/// `Item: Sendable` is SwiftUI's requirement, not ours: `Binding(get:set:)` takes `@Sendable`
/// closures, and both the storage binding and `presented` are captured by them.
func sheetItemBinding<Item: Sendable>(_ storage: Binding<Item?>, presented: Item) -> Binding<Item> {
    Binding(
        get: { storage.wrappedValue ?? presented },
        set: { next in
            // Cleared means dismissing. A late write from the content on its way out is not a
            // request to present anything.
            guard storage.wrappedValue != nil else { return }
            storage.wrappedValue = next
        }
    )
}
