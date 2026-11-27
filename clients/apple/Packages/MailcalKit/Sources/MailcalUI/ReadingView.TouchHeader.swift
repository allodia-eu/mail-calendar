// The touch clients' reading header: the subject as the page's title, never cut short, then who
// sent it, when, and to whom, by name. Every address and the full date are one tap away, in the
// details, and the sender's own address is always shown, because it is what tells a real sender
// from one borrowing the name.
//
// iPhone and iPad only; the Mac keeps `identityHeader` and `recipientsHeader` (ReadingView.swift).

#if os(iOS)
import MailcalBindings
import SwiftUI

extension ReadingView {
    var touchHeader: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(message.subject.isEmpty ? L10n.mail_no_subject() : message.subject)
                .font(.title2.weight(.bold))
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
            HStack(alignment: .top, spacing: 12) {
                AvatarView(avatar: senderAvatar, diameter: 40)
                VStack(alignment: .leading, spacing: 2) {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text(senderName).font(.headline)
                        Spacer(minLength: 0)
                        // The list's own label; the full date is in the details
                        // (`docs/timestamps.md`).
                        Text(relativeDate(message.date, in: model.activeZone, use24Hour: model.use24Hour))
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .layoutPriority(1)
                    }
                    if let address = senderAddress {
                        Text(address)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                    }
                    if bodySnapshot != nil {
                        detailsToggle
                    }
                }
            }
            if showsMessageDetails, let body = bodySnapshot {
                details(body)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, readingInset)
        .padding(.top, 16)
        .padding(.bottom, 12)
    }

    /// The sender by name: the loaded header's, else the list row's while the body is fetched.
    private var senderName: String {
        guard let body = bodySnapshot else { return message.from }
        if !body.fromName.isEmpty { return body.fromName }
        if !body.fromAddress.isEmpty { return body.fromAddress }
        return message.from
    }

    /// The sender's address, when the name line is not already it.
    private var senderAddress: String? {
        guard let body = bodySnapshot, !body.fromName.isEmpty, !body.fromAddress.isEmpty else {
            return nil
        }
        return body.fromAddress
    }

    /// One line naming the recipients, which opens and closes the details. Drawn as one line and
    /// cut short when long: everything it leaves out is what it opens.
    private var detailsToggle: some View {
        Button {
            withAnimation(.snappy) { showsMessageDetails.toggle() }
        } label: {
            HStack(alignment: .firstTextBaseline, spacing: 4) {
                Text(recipientSummary)
                    .font(.subheadline)
                    .lineLimit(1)
                Image(systemName: "chevron.down")
                    .font(.caption.weight(.semibold))
                    .rotationEffect(.degrees(showsMessageDetails ? 180 : 0))
            }
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            // A 44-point target without a taller line.
            .padding(.vertical, 12)
            .contentShape(Rectangle())
            .padding(.vertical, -12)
        }
        .buttonStyle(.plain)
        // Who received it is what the line says, so it is what VoiceOver reads; what a press
        // does is the hint.
        .accessibilityLabel(Text(recipientSummary))
        .accessibilityHint(
            showsMessageDetails ? L10n.a11y_hide_message_details() : L10n.a11y_show_message_details()
        )
    }

    /// "To: Ada, Bob  Cc: Carol": names only, each person's address when they have none.
    private var recipientSummary: AttributedString {
        guard let body = bodySnapshot else { return AttributedString() }
        let groups = [
            (L10n.compose_to(), body.toPeople),
            (L10n.compose_cc(), body.ccPeople),
            (L10n.compose_bcc(), body.bccPeople),
        ].filter { !$0.1.isEmpty }
        var line = AttributedString()
        for (index, (label, people)) in groups.enumerated() {
            if index > 0 { line += AttributedString("   ") }
            line += AttributedString("\(label): ")
            line += AttributedString(
                people.map { $0.name.isEmpty ? $0.address : $0.name }.joined(separator: ", ")
            )
        }
        return line
    }

    /// Every person with their address, and the full date under the label a printout gives it.
    private func details(_ body: ReadingSnapshot) -> some View {
        Grid(alignment: .topLeading, horizontalSpacing: 12, verticalSpacing: 10) {
            detailRow(L10n.compose_from(), [MailPerson(name: body.fromName, address: body.fromAddress)])
            detailRow(L10n.compose_to(), body.toPeople)
            detailRow(L10n.compose_cc(), body.ccPeople)
            detailRow(L10n.compose_bcc(), body.bccPeople)
            GridRow {
                detailLabel(L10n.quote_sent())
                Text(headerDate).font(.subheadline).textSelection(.enabled)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 12))
        .transition(.opacity.combined(with: .move(edge: .top)))
    }

    @ViewBuilder
    private func detailRow(_ label: String, _ people: [MailPerson]) -> some View {
        let people = people.filter { !$0.address.isEmpty || !$0.name.isEmpty }
        if !people.isEmpty {
            GridRow {
                detailLabel(label)
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(Array(people.enumerated()), id: \.offset) { _, person in
                        VStack(alignment: .leading, spacing: 0) {
                            Text(person.name.isEmpty ? person.address : person.name)
                                .font(.subheadline)
                            if !person.name.isEmpty {
                                Text(person.address)
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .textSelection(.enabled)
                    }
                }
            }
        }
    }

    private func detailLabel(_ label: String) -> some View {
        Text(label)
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .gridColumnAlignment(.trailing)
    }
}
#endif
