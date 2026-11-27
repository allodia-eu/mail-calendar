// The touch clients' reading actions, in the navigation bar (docs/reading-actions.md): archive and
// delete; the default answer, with a chevron beside it for the other answers; then the More menu
// for what acts on the message as a document. Plain symbols on the bar's glass, three groups.
//
// The bottom of the screen is the tab bar's, which stays while a message is open so the calendar
// is one tap away. A second bar of actions above it would stack two bars of buttons, and a tab bar
// is for moving between sections, never for acting on what is shown.
//
// iPhone and iPad only; the Mac keeps the action row above the message (ReadingView.swift).

#if os(iOS)
import MailcalBindings
import SwiftUI

extension ReadingView {
    @ToolbarContentBuilder
    var touchActions: some ToolbarContent {
        ToolbarItemGroup(placement: .topBarTrailing) {
            barButton(L10n.action_archive(), "archivebox", action: onArchive)
            barButton(L10n.action_delete(), "trash", action: onDelete)
        }
        if #available(iOS 26, *) {
            ToolbarSpacer(.fixed, placement: .topBarTrailing)
        }
        ToolbarItemGroup(placement: .topBarTrailing) {
            // The answer the core makes the default: reply-all when it would reach someone a
            // plain reply does not, else reply. The other one and forward wait behind a chevron
            // that says there are more, so the usual answer is one tap and nothing waits behind
            // a long press nobody discovers.
            if replyAllByDefault { replyAllButton } else { replyButton }
            Menu {
                if replyAllByDefault { replyButton } else { replyAllButton }
                barButton(L10n.action_forward(), "arrowshape.turn.up.right", action: onForward)
            } label: {
                Label(L10n.a11y_more_replies(), systemImage: "chevron.down")
            }
            .accessibilityLabel(L10n.a11y_more_replies())
        }
        if #available(iOS 26, *) {
            ToolbarSpacer(.fixed, placement: .topBarTrailing)
        }
        ToolbarItem(placement: .topBarTrailing) { moreMenu }
    }

    /// Reply until the body says otherwise: the core decides only once it has the recipients.
    private var replyAllByDefault: Bool { bodySnapshot?.defaultReplyAll ?? false }

    private var replyButton: some View {
        barButton(L10n.action_reply(), "arrowshape.turn.up.left", action: onReply)
    }

    private var replyAllButton: some View {
        barButton(L10n.action_reply_all(), "arrowshape.turn.up.left.2", action: onReplyAll)
    }

    private func barButton(_ title: String, _ icon: String, action: @escaping () -> Void) -> some View {
        Button(action: action) { Label(title, systemImage: icon) }
    }

    /// The overflow, as the platform's More menu: the sun/moon choice in a dark app
    /// (docs/reading-dark-mode.md), then the actions on the message as a document.
    private var moreMenu: some View {
        Menu {
            if appScheme == .dark {
                let dark = messageScheme == .dark
                Button {
                    model.setDarkMessageBackground(!dark)
                } label: {
                    Label(
                        dark ? L10n.reading_show_light_background() : L10n.reading_show_dark_background(),
                        systemImage: dark ? "sun.max" : "moon"
                    )
                }
                Divider()
            }
            Button(action: export) {
                Label(L10n.action_save_as_eml(), systemImage: "square.and.arrow.down.on.square")
            }
            .disabled(exporting)
            Button(action: printMessage) {
                Label(L10n.action_print(), systemImage: "printer")
            }
            .disabled(printDocument == nil)
        } label: {
            if exporting {
                ProgressView()
            } else {
                Label(L10n.a11y_more_actions(), systemImage: "ellipsis")
            }
        }
        .accessibilityLabel(L10n.a11y_more_actions())
    }
}
#endif
