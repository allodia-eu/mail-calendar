// The stored entries a client counts as mail accounts (ReservedConfigs.swift): the Allodia grant
// and the own endpoint's key are the core's own, so neither makes a first run into a later one,
// and a debug launch carries both over. Which entry is which is asked of the core, so these run
// against the real bindings.

import MailcalBindings
import Testing

@testable import MailcalUI

private let endpointKey = "[ai_endpoint]\nkey = \"sk-test\"\n"
private let allodiaGrant = """
    [allodia]
    email = "person@test.local"
    refresh_token = "refresh"
    """
private let mailAccount = """
    [imap]
    host = "imap.test.local"
    username = "person@test.local"
    """

@Suite struct ReservedConfigsTests {
    @Test func aSavedEndpointKeyIsNotAMailAccount() {
        #expect(holdsNoMailAccount([endpointKey]))
        #expect(holdsNoMailAccount([endpointKey, allodiaGrant]))
    }

    @Test func aMailAccountBesideThemIsCounted() {
        #expect(!holdsNoMailAccount([endpointKey, allodiaGrant, mailAccount]))
        #expect(holdsNoMailAccount([]))
    }

    @Test func aDebugLaunchCarriesTheKeyAndTheGrantOverAndNothingElse() {
        #expect(
            reservedConfigs([mailAccount, endpointKey, allodiaGrant])
                == [endpointKey, allodiaGrant])
    }
}
