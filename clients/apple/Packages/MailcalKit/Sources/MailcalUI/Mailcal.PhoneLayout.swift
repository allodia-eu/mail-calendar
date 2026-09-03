// The iPhone shell: three surfaces as tabs, each with its own navigation stack, and the
// accounts-and-folders pane as a drawer over all of it.
//
// Split from Mailcal.Layout.swift, which holds the shared pane and the desktop/iPad splits.

#if os(iOS)
import MailcalBindings
import SwiftUI

extension ContentView {
    /// iPhone: Mail, Calendar and Contacts as a tab bar; the sidebar slides in over them.
    ///
    /// The tab bar is what makes the three surfaces one tap apart from anywhere, and it is why the
    /// sidebar carries only accounts, folders and Settings, offering Calendar and Contacts in both
    /// places would be two routes to the same screen, one of them hidden behind a gesture.
    var iPhoneLayout: some View {
        SidebarDrawer(
            isOpen: $sidebarOpen,
            // Nothing pushed means nothing for the system back-swipe to do, so the edge is ours.
            edgeSwipeEnabled: openedMessage == nil && openedContact == nil
        ) {
            sidebarList(showsCalendarAndContacts: false)
        } content: {
            TabView(selection: phoneTab) {
                Tab(L10n.nav_mail(), systemImage: "tray.full", value: AppDestination.mail) {
                    phoneMail
                }
                Tab(L10n.nav_calendar(), systemImage: "calendar", value: AppDestination.calendar) {
                    phoneCalendar
                }
                Tab(L10n.nav_contacts(), systemImage: "person.2", value: AppDestination.contacts) {
                    phoneContacts
                }
            }
        }
        // The two sidebar rows that present something rather than navigate. Every row that *does*
        // navigate shuts the drawer in the action itself (`dismissSidebar`), which also covers
        // tapping the folder you are already in, a change-watcher there would see nothing happen
        // and leave the drawer standing open.
        .onChange(of: settingsCategory) { _, opening in if opening != nil { dismissSidebar() } }
        .onChange(of: model.addingAccount) { _, opening in if opening { dismissSidebar() } }
    }

    /// Which tab is showing. Mail is restored rather than re-selected, see `showMail()`.
    private var phoneTab: Binding<AppDestination> {
        Binding(
            get: { model.destination },
            set: { destination in
                switch destination {
                case .mail: showMail()
                case .calendar: showCalendar()
                case .contacts: showContacts()
                }
            }
        )
    }

    private var phoneMail: some View {
        NavigationStack {
            compactMessageList
                .navigationTitle(currentFolderName)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) { sidebarToggle }
                    ToolbarItemGroup(placement: .topBarTrailing) {
                        messageListMenu
                        Button { compose = .new } label: { Image(systemName: "square.and.pencil") }
                    }
                }
                .navigationDestination(item: $openedMessage) { opened in
                    readingView(for: opened)
                }
        }
    }

    private var phoneCalendar: some View {
        NavigationStack {
            // The calendar carries its own header (shape picker, back-to-today, new event,
            // manage), so the navigation bar stays out of its way.
            calendarDetail
                .navigationTitle(L10n.nav_calendar())
                .navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .topBarLeading) { sidebarToggle } }
        }
    }

    private var phoneContacts: some View {
        NavigationStack {
            contactsList
                .navigationTitle(L10n.contacts_title())
                .navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .topBarLeading) { sidebarToggle } }
                // Tapping a person pushes their detail; Back pops it. The same gesture the reading
                // view uses, so the phone has one way of opening a row.
                .navigationDestination(item: $openedContact) { opened in
                    ContactDetailView(
                        detail: opened.detail,
                        accountLabels: model.contactAccountLabels,
                        onEdit: opened.detail.editableCards.isEmpty
                            ? nil
                            : { beginEditContact(opened.detail) }
                    )
                    .navigationTitle(
                        opened.detail.displayName.isEmpty
                            ? L10n.contacts_no_name() : opened.detail.displayName
                    )
                    .navigationBarTitleDisplayMode(.inline)
                }
        }
    }

    /// Opens the sidebar. On every tab, because Settings lives on the pane and must not be
    /// something you first have to go back to the mailbox to reach.
    private var sidebarToggle: some View {
        Button { withAnimation { sidebarOpen = true } } label: {
            Image(systemName: "line.3.horizontal")
        }
        .accessibilityLabel(L10n.a11y_open_folders())
    }
}
#endif
