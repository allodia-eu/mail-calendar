// The pill a calendar grid shows over itself while its period is loading.
//
// A product decision, the same on every client (docs/calendar.md, "Showing a period that is
// loading"): a period the core has not materialized keeps its grid, and a spinner beside "loading
// this period" floats over it, pinned to what is on screen. The caller places it over a container
// that does not scroll, so it never moves with the day.

import MailcalBindings
import SwiftUI

/// The identifier a test finds the pill by.
let calendarLoadingPillID = "CalendarLoadingPill"

struct CalendarLoadingPill: View {
    var body: some View {
        HStack(spacing: 6) {
            ProgressView().controlSize(.small)
            Text(L10n.calendar_loading_range())
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 5)
        .background(.regularMaterial, in: Capsule())
        .shadow(color: .black.opacity(0.15), radius: 3, y: 1)
        // Nothing on Apple speaks the period's state but this, so the pill is one spoken element.
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L10n.calendar_loading_range())
        .accessibilityAddTraits(.isStaticText)
        .accessibilityIdentifier(calendarLoadingPillID)
        // A press on the grid under the pill reaches the grid.
        .allowsHitTesting(false)
    }
}

extension View {
    /// `CalendarLoadingPill` over the top of this view while `loading`.
    func calendarLoadingPill(_ loading: Bool) -> some View {
        overlay(alignment: .top) {
            if loading {
                CalendarLoadingPill().padding(.top, 8)
            }
        }
    }
}
