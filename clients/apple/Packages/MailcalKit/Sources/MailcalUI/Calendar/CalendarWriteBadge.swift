// The small calendar write-status badge in the header: the Swift side of the core's
// `CalendarWriteStatus` (`Surface.calendarStatus`).
//
// The mapping from status to what the header shows is a plain value (`CalendarWriteIndicator`) so the
// state machine is unit-tested without rendering a view; the badge is a thin render of it. Both
// warnings tap to a refresh (a `.refreshCalendar`), never a re-send: a change that may have landed is
// reconciled by the next sync, and one that did not land is the user's to make again.

import MailcalBindings
import SwiftUI

/// What the calendar header should show for the most recent write.
enum CalendarWriteIndicator: Equatable {
    /// Nothing to show.
    case hidden
    /// A write is settling, a small spinner.
    case spinner
    /// The write settled and the local view holds the server's copy, a brief check.
    case saved
    /// The change was not saved: nothing reached the server, or the server refused it.
    case notSaved
    /// The change may have landed and could not be confirmed.
    case unconfirmed

    /// Maps a core ``CalendarWriteStatus`` to what the header shows. Total and pure.
    static func of(_ status: CalendarWriteStatus) -> CalendarWriteIndicator {
        switch status {
        case .idle: return .hidden
        case .saving: return .spinner
        case .saved: return .saved
        case .notSaved: return .notSaved
        case .unconfirmed: return .unconfirmed
        }
    }

    /// Whether tapping the indicator should trigger a retry (a `.refreshCalendar`).
    var offersRetry: Bool { warning != nil }

    /// What a warning says, or `nil` for an indicator that is not one.
    var warning: String? {
        switch self {
        case .notSaved: L10n.calendar_save_failed()
        case .unconfirmed: L10n.calendar_save_unconfirmed()
        case .hidden, .spinner, .saved: nil
        }
    }
}

/// The header badge. Renders the mapped ``CalendarWriteIndicator``: a spinner while `.saving`, a check
/// on `.saved`, a tap-to-retry warning on `.notSaved` and `.unconfirmed`. Nothing on `.idle`.
/// `onRetry` is a refresh.
struct CalendarWriteBadge: View {
    let status: CalendarWriteStatus
    let onRetry: () -> Void

    var body: some View {
        let indicator = CalendarWriteIndicator.of(status)
        switch indicator {
        case .hidden:
            EmptyView()
        case .spinner:
            ProgressView()
                .controlSize(.small)
                .help(L10n.calendar_saving())
                .accessibilityLabel(L10n.calendar_saving())
        case .saved:
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(.green)
                .help(L10n.calendar_saved())
                .accessibilityLabel(L10n.calendar_saved())
        case .notSaved, .unconfirmed:
            let label = indicator.warning ?? ""
            Button(action: onRetry) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.orange)
            }
            .buttonStyle(.borderless)
            .help(label)
            .accessibilityLabel(label)
        }
    }
}
