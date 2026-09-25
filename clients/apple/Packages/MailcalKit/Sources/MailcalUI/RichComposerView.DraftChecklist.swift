// The card a drafted reply brings (docs/ai.md, "Where they show"): what the message asks and what
// is left to do, between the action bar and the editor, and the thumbs for feedback on the draft
// ("Feedback"). It is SwiftUI beside the editor, so it is never part of the mail. Send asks once
// while an item is open, and never blocks.

import Foundation
import MailcalBindings
import SwiftUI
#if os(iOS)
import UIKit
#endif

extension RichComposeView {
    /// The card, with what follows the editor and the question Send asks, for as long as it shows.
    /// It shows for a draft with nothing to list too, when there is feedback to offer on it.
    @ViewBuilder var draftCard: some View {
        let rating = draftRating
        if !draftStatus.checklist.isEmpty || rating != nil {
            DraftChecklistCard(status: draftStatus, rating: rating)
                .task(id: draftStatus.checklist.following) { await followPlaceholders() }
                .onChange(of: attachments.count) { _, count in
                    draftStatus.checklist.attachmentsChanged(to: count)
                }
                .alert(L10n.composer_send_open_title(), isPresented: $draftStatus.confirmingSend) {
                    Button(L10n.composer_send_anyway()) {
                        draftStatus.checklist.sendAsked()
                        prepareAndSend()
                    }
                    Button(L10n.composer_keep_editing(), role: .cancel) {
                        draftStatus.checklist.sendAsked()
                    }
                } message: {
                    Text(L10n.composer_send_open_message(count: draftStatus.checklist.openCount))
                }
        }
    }

    /// How the card keeps a rating of the draft in the editor, or `nil` while feedback is not
    /// offered (docs/ai.md, "Feedback").
    private var draftRating: ((DraftRating, Bool) -> Bool)? {
        guard let draftReply, let draftId = draftStatus.feedback.draftId, draftReply.feedbackOffered()
        else { return nil }
        return { rating, includeContent in draftReply.rate(draftId, rating, includeContent) }
    }

    /// Send, asking first while an item is open, with the placeholders read afresh.
    func requestSend() {
        guard !draftStatus.checklist.hasAsked, !draftStatus.checklist.items.isEmpty else {
            prepareAndSend()
            return
        }
        Task {
            await readPlaceholders()
            if draftStatus.checklist.asksBeforeSend {
                draftStatus.confirmingSend = true
            } else {
                prepareAndSend()
            }
        }
    }

    /// About once a second while a fill-in item is open; the card's task, so it ends with the
    /// composer.
    private func followPlaceholders() async {
        while draftStatus.checklist.awaitsPlaceholders {
            try? await Task.sleep(for: .seconds(1))
            if Task.isCancelled { return }
            await readPlaceholders()
        }
    }

    private func readPlaceholders() async {
        let placeholders = draftStatus.checklist.placeholders
        guard !placeholders.isEmpty,
              let left = await editor.placeholdersLeft(placeholders),
              !Task.isCancelled
        else {
            return
        }
        draftStatus.checklist.placeholdersLeft(left)
    }
}

private struct DraftChecklistCard: View {
    @Bindable var status: ComposerDraftStatus
    /// Keeps a rating of the draft; `nil` when no feedback is offered.
    let rating: ((DraftRating, Bool) -> Bool)?
    /// One column for the kind icons, so every row's text starts at the same place.
    @ScaledMetric(relativeTo: .callout) private var iconWidth = 18.0

    private var checklist: DraftChecklist { status.checklist }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if !checklist.isEmpty { disclosure }
            // Outside the disclosure, so a collapsed card still offers it.
            if let rating {
                DraftRatingControls(feedback: $status.feedback, isFeedback: true, keep: rating)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.fill.tertiary, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
    }

    private var disclosure: some View {
        DisclosureGroup(isExpanded: expanded) {
            VStack(alignment: .leading, spacing: 8) {
                if !checklist.summary.isEmpty {
                    Text(checklist.summary)
                        .font(.callout)
                        .fixedSize(horizontal: false, vertical: true)
                    if !checklist.items.isEmpty {
                        heading(L10n.composer_checklist_heading()).padding(.top, 4)
                    }
                }
                ForEach(checklist.items) { row($0) }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, 8)
        } label: {
            heading(
                checklist.summary.isEmpty
                    ? L10n.composer_checklist_heading()
                    : L10n.composer_draft_summary_heading()
            )
        }
    }

    private var expanded: Binding<Bool> {
        Binding(
            get: { !checklist.isCollapsed(onPhone: Self.onPhone) },
            set: { status.checklist.collapsedChoice = !$0 }
        )
    }

    private static var onPhone: Bool {
        #if os(iOS)
        UIDevice.current.userInterfaceIdiom == .phone
        #else
        false
        #endif
    }

    private func heading(_ text: String) -> some View {
        Text(text)
            .font(.subheadline.weight(.semibold))
            // A colour rather than `.secondary`, which would take the tint of the disclosure's
            // button on iOS.
            .foregroundStyle(Color.secondary)
            .accessibilityAddTraits(.isHeader)
    }

    /// A fill-in item ticks itself as the reply changes, so only the others are buttons.
    @ViewBuilder private func row(_ item: DraftChecklist.Item) -> some View {
        if item.kind == .fillIn {
            rowContent(item)
                .accessibilityElement(children: .combine)
                .accessibilityAddTraits(item.ticked ? .isSelected : [])
        } else {
            Button {
                status.checklist.toggle(item.id)
            } label: {
                rowContent(item).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(item.ticked ? [.isToggle, .isSelected] : .isToggle)
        }
    }

    private func rowContent(_ item: DraftChecklist.Item) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: item.ticked ? "checkmark.square.fill" : "square")
                .foregroundStyle(item.ticked ? Color.accentColor : Color.secondary)
                .accessibilityHidden(true)
            kindIcon(item.kind).frame(width: iconWidth)
            Text(item.title)
                .strikethrough(item.ticked)
                .foregroundStyle(item.ticked ? .secondary : .primary)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .font(.callout)
    }

    @ViewBuilder private func kindIcon(_ kind: DraftTaskKind) -> some View {
        switch kind {
        case .fillIn:
            Image(systemName: "character.cursor.ibeam").foregroundStyle(.secondary)
                .accessibilityHidden(true)
        case .attach:
            Image(systemName: "paperclip").foregroundStyle(.secondary)
                .accessibilityLabel(L10n.a11y_task_attach())
        case .do:
            Image(systemName: "checklist").foregroundStyle(.secondary)
                .accessibilityLabel(L10n.a11y_task_do())
        }
    }
}
