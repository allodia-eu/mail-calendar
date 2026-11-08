// How the core's notices are drawn (docs/notices.md): a card at the top of the sidebar on the Mac
// and the iPad, one row under the bar on iPhone, and every other notice in one Issues list behind
// either. Never a stack.

import MailcalBindings
import SwiftUI

/// The severity's glyph: its shape differs by severity, and only Blocked takes the warning colour.
struct NoticeGlyph: View {
    let severity: NoticeSeverity

    var body: some View {
        Image(systemName: NoticeRules.symbol(severity))
            .foregroundStyle(NoticeRules.isWarning(severity) ? AnyShapeStyle(.orange) : AnyShapeStyle(.secondary))
            .accessibilityHidden(true)
    }
}

/// One notice: what it says, why, and its action, with a question's other answer as the lesser
/// button beside it (rule 8), and a close button when it may be closed.
struct NoticeEntry: View {
    var model: MailboxModel
    let notice: Notice

    @State private var serverChange: ServerChangeReview?
    /// An answer is running, so neither button can start a second one.
    @State private var running = false

    private var held: Bool { running || notice.running }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                NoticeGlyph(severity: notice.severity)
                VStack(alignment: .leading, spacing: 2) {
                    Text(NoticeRules.text(notice.title))
                        .font(.callout.weight(.semibold))
                        .fixedSize(horizontal: false, vertical: true)
                    if let detail = NoticeRules.detail(notice) {
                        Text(detail)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                Spacer(minLength: 0)
                if notice.dismissible {
                    Button { model.dismissNotice(notice) } label: {
                        Image(systemName: "xmark").imageScale(.small)
                            #if os(iOS)
                            .frame(minWidth: 44, minHeight: 44)
                            .contentShape(Rectangle())
                            #endif
                    }
                    .buttonStyle(.borderless)
                    .accessibilityLabel(L10n.notices_dismiss())
                }
            }
            if let action = notice.action, let label = notice.actionLabel {
                HStack(spacing: 8) {
                    Spacer(minLength: 0)
                    if let second = notice.secondAction, let secondLabel = notice.secondActionLabel {
                        Button(NoticeRules.text(secondLabel)) { run(second) }
                            .buttonStyle(.borderless)
                            .disabled(held)
                    }
                    if held { ProgressView().controlSize(.small) }
                    Button(NoticeRules.text(label)) { run(action) }
                        .buttonStyle(.bordered)
                        .disabled(held)
                }
                #if os(macOS)
                .controlSize(.small)
                #endif
            }
        }
        .accessibilityElement(children: .contain)
        .serverChangeReview(model, $serverChange)
    }

    private func run(_ action: NoticeAction) {
        Task {
            running = true
            await model.runNoticeAction(notice, action) { serverChange = $0 }
            running = false
        }
    }
}

/// Every notice `destination` shows, each with its own action: the Issues list.
struct NoticeList: View {
    var model: MailboxModel
    let notices: [Notice]

    var body: some View {
        List(notices, id: \.id) { notice in
            NoticeEntry(model: model, notice: notice)
                .padding(.vertical, 4)
        }
        .navigationTitle(L10n.notices_issues())
    }
}

/// The card at the top of the sidebar on the Mac and the iPad: the first notice the screen on show
/// has, and "N more issues" opening the rest. Above the accounts it is about, and out of the
/// content column, so the list and its actions bar never move.
struct NoticeCard: View {
    var model: MailboxModel
    let destination: AppDestination

    @State private var showsAll = false

    var body: some View {
        let shown = model.shownNotices(on: destination)
        if let first = shown.first {
            VStack(alignment: .leading, spacing: 6) {
                NoticeEntry(model: model, notice: first)
                if shown.count > 1 {
                    Button(L10n.notices_more(count: shown.count - 1)) { showsAll = true }
                        .buttonStyle(.borderless)
                        .font(.caption)
                        .popover(isPresented: $showsAll, arrowEdge: .trailing) {
                            NavigationStack { NoticeList(model: model, notices: shown) }
                                .frame(minWidth: 340, minHeight: 240)
                        }
                }
            }
            .padding(10)
            .background(.background, in: RoundedRectangle(cornerRadius: 8))
            .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(.quaternary) }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .accessibilityIdentifier("notice")
        }
    }
}

#if os(iOS)
/// One row under the bar on iPhone: the first notice the tab has, and how many more. Tapping it
/// opens every one of them in a sheet, each with its own action, so the screen loses one row's
/// height and never a stack.
struct NoticeStrip: View {
    var model: MailboxModel
    let destination: AppDestination

    @State private var showsAll = false

    var body: some View {
        let shown = model.shownNotices(on: destination)
        if let first = shown.first {
            Button { showsAll = true } label: {
                HStack(spacing: 10) {
                    NoticeGlyph(severity: first.severity)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(NoticeRules.text(first.title))
                            .font(.subheadline.weight(.semibold))
                            .foregroundStyle(.primary)
                            .multilineTextAlignment(.leading)
                        if shown.count > 1 {
                            Text(L10n.notices_more(count: shown.count - 1))
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                        }
                    }
                    Spacer(minLength: 0)
                    Image(systemName: "chevron.right")
                        .foregroundStyle(.tertiary)
                        .accessibilityHidden(true)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .frame(minHeight: 44)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .bannerBackground()
            .overlay(alignment: .bottom) { Divider() }
            .accessibilityIdentifier("notice")
            .sheet(isPresented: $showsAll) {
                NavigationStack {
                    NoticeList(model: model, notices: model.shownNotices(on: destination))
                        .toolbar {
                            ToolbarItem(placement: .confirmationAction) {
                                Button(L10n.notices_dismiss()) { showsAll = false }
                            }
                        }
                }
                .presentationDetents([.medium, .large])
            }
        }
    }
}
#endif
