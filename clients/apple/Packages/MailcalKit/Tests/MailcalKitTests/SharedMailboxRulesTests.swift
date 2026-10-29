// What the Shared mailboxes section draws for each answer (docs/shared-mailboxes.md rules 3, 7 and
// 8). The Apple client's copy of the suite Linux carries (`shared_mailboxes_tests`,
// `account_shared`).

import MailcalBindings
import Testing

@testable import MailcalUI

private func mailbox(
    _ address: String?, _ name: String, selected: Bool = false, listed: Bool = true
) -> SharedMailboxEntry {
    SharedMailboxEntry(handle: "f", name: name, address: address, selected: selected, listed: listed)
}

private func view(_ support: SharedMailboxSupport, _ mailboxes: [SharedMailboxEntry] = []) -> SharedMailboxesView {
    SharedMailboxesView(support: support, mailboxes: mailboxes)
}

@Suite struct SharedMailboxRulesTests {
    @Test func aMailboxIsNamedByItsAddressWithTheServersLabelBesideItWhenItDiffers() {
        #expect(
            SharedMailboxRules.row(mailbox("support@example.org", "Support Team", selected: true))
                == SharedMailboxRow(title: "support@example.org", subtitle: "Support Team", active: true))
        #expect(SharedMailboxRules.row(mailbox("support@example.org", "support@example.org")).subtitle == nil)
        #expect(SharedMailboxRules.row(mailbox(nil, "team")).title == "team")
    }

    @Test func aChosenMailboxTheServerStoppedListingSaysSo() {
        #expect(
            SharedMailboxRules.row(mailbox("support@example.org", "Support", selected: true, listed: false))
                .subtitle == L10n.settings_account_shared_withdrawn())
    }

    @Test func theLineUnderTheSwitchesSaysWhyThereIsNothingMoreToChoose() {
        #expect(SharedMailboxRules.note(view(.listed)) == L10n.settings_account_shared_none())
        #expect(SharedMailboxRules.note(view(.listed, [mailbox(nil, "team")])) == nil)
        #expect(SharedMailboxRules.note(view(.byAddress)) == L10n.settings_account_shared_by_address())
        #expect(SharedMailboxRules.note(view(.notConnected)) == L10n.settings_account_shared_not_connected())
        #expect(SharedMailboxRules.note(view(.unreachable)) == L10n.settings_account_shared_unreachable())
    }

    @Test func anAccountThatCannotOpenOneHasNoSection() {
        #expect(!SharedMailboxRules.shows(nil))
        #expect(!SharedMailboxRules.shows(view(.unsupported)))
        // A chosen one stays switchable off, whatever the server says now.
        #expect(SharedMailboxRules.shows(view(.unsupported, [mailbox(nil, "team", selected: true)])))
        for support in [SharedMailboxSupport.listed, .byAddress, .notConnected, .unreachable] {
            #expect(SharedMailboxRules.shows(view(support)))
        }
    }

    @Test func aPageAsksAgainOnlyWhileTheAnswerMayStillChange() {
        #expect(SharedMailboxRules.mayAskAgain(nil))
        #expect(SharedMailboxRules.mayAskAgain(view(.notConnected)))
        #expect(SharedMailboxRules.mayAskAgain(view(.unsupported)))
        #expect(!SharedMailboxRules.mayAskAgain(view(.listed)))
        #expect(!SharedMailboxRules.mayAskAgain(view(.byAddress)))
        #expect(!SharedMailboxRules.mayAskAgain(view(.unreachable)))
    }

    @Test func anAddressIsAskedForOnlyOnceSomethingIsTyped() {
        #expect(SharedMailboxRules.address("  support@example.org\n") == "support@example.org")
        #expect(SharedMailboxRules.address("   ") == nil)
    }
}
