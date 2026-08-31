// The iPad shell: the three-column and two-column splits. Split from Mailcal.Layout.swift, which
// holds the shared pane and the macOS split.

#if os(iOS)
import MailcalBindings
import SwiftUI

extension ContentView {
    /// iPad: sidebar | message list | reading, all visible at regular width, except on the
    /// calendar, which takes the whole width beside the sidebar (as on macOS) rather than being
    /// penned into the middle column with an empty reading pane beside it.
    @ViewBuilder var iPadLayout: some View {
        switch model.destination {
        case .calendar: iPadCalendarLayout
        case .contacts: iPadContactsLayout
        case .mail: iPadMailLayout
        }
    }

    /// Contacts as the three-column split, sidebar | people | detail. The same shape as mail,
    /// because it is the same kind of screen: a list you pick one row of.
    private var iPadContactsLayout: some View {
        NavigationSplitView {
            sidebarList(showsCalendarAndContacts: true)
                .navigationSplitViewColumnWidth(min: 220, ideal: 240, max: 300)
        } content: {
            // The list carries its own title and search field, so the navigation bar stays out of
            // its way rather than showing the heading twice.
            contactsList
                .navigationBarTitleDisplayMode(.inline)
                .navigationSplitViewColumnWidth(min: 300, ideal: 360, max: 460)
        } detail: {
            contactsDetailPane
        }
    }

    /// The calendar as a two-column split, sidebar | full-width grid. Mirrors the macOS layout,
    /// where opening the calendar replaces both the list and the reading pane: a grid squeezed into
    /// the middle column, next to a "select a message" placeholder, wastes half an iPad.
    private var iPadCalendarLayout: some View {
        NavigationSplitView {
            sidebarList(showsCalendarAndContacts: true)
                .navigationSplitViewColumnWidth(min: 220, ideal: 240, max: 300)
        } detail: {
            // The calendar carries its own header (shape picker, back-to-today, new event, manage),
            // so the navigation bar stays out of its way.
            calendarDetail
                .navigationTitle(L10n.nav_calendar())
                .navigationBarTitleDisplayMode(.inline)
        }
    }

    /// Mail as the three-column split, sidebar | message list | reading.
    private var iPadMailLayout: some View {
        NavigationSplitView {
            sidebarList(showsCalendarAndContacts: true)
                // Keep the accounts/folders sidebar compact so the message list gets more room.
                .navigationSplitViewColumnWidth(min: 220, ideal: 240, max: 300)
        } content: {
            compactMessageList
                .navigationTitle(currentFolderName)
                .navigationBarTitleDisplayMode(.inline)
                // The default content-column width is too narrow for a mail list (hard-truncated
                // subjects, wrapping dates), give it a comfortable width; the reading pane takes
                // whatever is left.
                .navigationSplitViewColumnWidth(min: 340, ideal: 420, max: 560)
                .toolbar {
                    ToolbarItemGroup(placement: .topBarTrailing) {
                        messageListMenu
                        Button { compose = .new } label: { Image(systemName: "square.and.pencil") }
                    }
                }
        } detail: {
            readingPane
        }
    }
}
#endif
