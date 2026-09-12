// The macOS window toolbar: New Mail in the leading corner, the search field centred over the
// window. Its own file so Mailcal.Layout.swift stays under the repo line limit.
//
// A window toolbar rather than controls inside a pane, because it is the only surface on macOS
// that is *the window's* own top row. The traffic lights already own the leading corner, so a
// button placed at the top of the sidebar would either sit under them or be offset by hand from
// a size the system owns; the toolbar reserves that space itself. Centring is the same argument:
// the field belongs to the window, not to the message list, and only the toolbar has a centre
// that is not a pane's.

#if os(macOS)
import MailcalBindings
import SwiftUI

extension ContentView {
    @ToolbarContentBuilder var windowToolbar: some ToolbarContent {
        ToolbarItem(placement: .navigation) {
            Button {
                // Mail first: on macOS the composer takes the reading pane's place, and that
                // pane exists only on the mail surface, so writing from the calendar has to
                // bring the mailbox back with it or the draft would open behind the grid.
                showMail()
                compose = .new
            } label: {
                // Two nudges, both measured against the window buttons rather than judged.
                //
                // Trailing: a glyph's bounding box carries its own air and a word's does not,
                // so the toolbar's equal insets read as tight after the word.
                //
                // Bottom: SwiftUI centres the label's *line box*, which reserves descender
                // depth that "New Mail" never uses, so the ink it draws sits 1.5 pt below the
                // capsule's centre; the item's height is the toolbar's and does not grow, so
                // padding under the label lifts what you can see onto the line the close,
                // minimise and zoom buttons are on.
                Label(L10n.action_compose(), systemImage: "square.and.pencil")
                    .padding(.trailing, 2)
                    .padding(.bottom, 3)
            }
            // Word and glyph together. A macOS toolbar draws a `Label` icon-only by default, so
            // the one control this change exists to name would have arrived unnamed.
            .labelStyle(.titleAndIcon)
            .help(L10n.action_compose())
        }
        // Mail only. Search reads mail, and a field over the calendar would narrow nothing on
        // screen. New Mail stays either way, so the toolbar is never empty and the titlebar
        // never changes height as the user moves between surfaces.
        if model.destination == .mail {
            ToolbarItem(placement: .principal) { toolbarSearchField }
        }
    }

    /// The search field, as it was in the message list's header: the same control, the same
    /// dispatch, in the window's centre rather than above one column.
    private var toolbarSearchField: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField(L10n.search_placeholder(), text: $searchText)
                .textFieldStyle(.plain)
                .onChange(of: searchText) { _, query in model.search(query) }
            // The way out of search, and the only one this layout has: the phone's field carries
            // its own and the sidebar's back gesture is not on screen here. Emptying the field is
            // what leaves search (`onChange` above dispatches the clear), so the button has one
            // job and the core has one route in.
            if !searchText.isEmpty {
                Button { searchText = "" } label: {
                    Image(systemName: "xmark.circle.fill")
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
                .accessibilityLabel(L10n.search_clear())
                .help(L10n.search_clear())
            }
        }
        // Padding only, and no background of our own: the toolbar draws the item its own
        // capsule and hugs whatever is inside it, so a second one nests a pill in a pill, and
        // with no padding at all the magnifying glass sits hard against the rounded edge.
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .frame(minWidth: 200, idealWidth: 360, maxWidth: 460)
    }
}
#endif
