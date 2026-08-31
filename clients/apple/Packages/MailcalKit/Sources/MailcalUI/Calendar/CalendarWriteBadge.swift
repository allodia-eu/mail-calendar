// The small calendar write-status badge in the header: the Swift side of the core's
// `CalendarWriteStatus` (`Surface.calendarStatus`).
//
// The mapping from status to what the header shows is a plain value (`CalendarWriteIndicator`) so the
// state machine is unit-tested without rendering a view; the badge is a thin render of it. `.warning`
// is deliberately NOT "your change was rejected": the core has confirmed the write reached the server,
// and a refresh reconciles the local view, so the warning offers a retry (a `.refreshCalendar`).

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
    /// The write could not be confirmed, a warning the user can tap to retry.
    case warning

    /// Maps a core ``CalendarWriteStatus`` to what the header shows. Total and pure.
    static func of(_ status: CalendarWriteStatus) -> CalendarWriteIndicator {
        switch status {
        case .idle: return .hidden
        case .saving: return .spinner
        case .saved: return .saved
        case .failed: return .warning
        }
    }

    /// Whether tapping the indicator should trigger a retry (a `.refreshCalendar`).
    var offersRetry: Bool { self == .warning }
}

/// The header badge. Renders the mapped ``CalendarWriteIndicator``: a spinner while `.saving`, a check
/// on `.saved`, a tap-to-retry warning on `.failed`. Nothing on `.idle`. `onRetry` is a refresh.
struct CalendarWriteBadge: View {
    let status: CalendarWriteStatus
    let onRetry: () -> Void

    var body: some View {
        switch CalendarWriteIndicator.of(status) {
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
        case .warning:
            Button(action: onRetry) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.orange)
            }
            .buttonStyle(.borderless)
            .help(L10n.calendar_save_unconfirmed())
            .accessibilityLabel(L10n.calendar_save_unconfirmed())
        }
    }
}
