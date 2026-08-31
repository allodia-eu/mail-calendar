// The dev/prod split (DevNamespace.swift). A dev build and the shipped Developer ID build share
// one login keychain, one home directory, and one UserDefaults domain (same bundle id), so what
// keeps them apart is entirely these names. They are asserted against the injected bundle id
// (docs/branding.md) rather than a literal: an unbranded build must keep them apart too. Two failure modes are invisible until they have
// already corrupted something on a real machine, dev reaching production state, and two dev modes
// sharing a store, so both are pinned here.

import MailcalBindings
import XCTest

@testable import MailcalUI

final class DevNamespaceTests: XCTestCase {
    /// Every mode a DEBUG build can run in, including the ones that touch real data.
    private let allModes: [String?] = [
        nil, "personal", "demo", "stalwart", "stalwart-multi", "stalwart-imap", "unrecognized",
    ]

    /// No dev mode may resolve to production, not the harness ones, and above all not `personal`,
    /// which is the mode that reads and writes real credentials and real mail.
    func testNoDevModeTouchesProduction() {
        for mode in allModes {
            let label = mode ?? "nil"
            XCTAssertNotEqual(
                DevNamespace.keychainService(for: mode), DevNamespace.prodKeychainService,
                "mode \(label) would share the shipped app's Keychain items")
            XCTAssertNotEqual(
                DevNamespace.dataDirName(for: mode), DevNamespace.prodDataDirName,
                "mode \(label) would share the shipped app's engine store")
        }
    }

    /// Distinct dev modes must not share a store. This is the one that nearly went wrong: the
    /// obvious name for `personal` was the base `mailcal-dev`, which the JMAP harness already
    /// owns, two modes silently writing one SQLite database.
    func testDistinctModesNeverShareADataDirectory() {
        let distinct: [String?] = [nil, "stalwart", "stalwart-multi", "stalwart-imap"]
        let dirs = distinct.map { DevNamespace.dataDirName(for: $0) }
        XCTAssertEqual(
            Set(dirs).count, dirs.count,
            "two dev modes resolve to the same engine store: \(dirs)")

        let services = distinct.map { DevNamespace.keychainService(for: $0) }
        XCTAssertEqual(
            Set(services).count, services.count,
            "two dev modes resolve to the same Keychain namespace: \(services)")
    }

    /// `personal`, unset, and `demo` are one namespace: none of them is a harness run, and a
    /// developer switching between them expects the same dev accounts.
    func testPersonalUnsetAndDemoShareTheBaseDevNamespace() {
        for mode in [nil, "personal", "demo"] as [String?] {
            XCTAssertEqual(DevNamespace.keychainService(for: mode), "\(Brand.appID).dev")
            XCTAssertEqual(DevNamespace.dataDirName(for: mode), "mailcal-dev-personal")
        }
    }

    /// The harness store names are documented in `docs/debugging.md` and mirrored by the other
    /// clients, so they are a contract rather than an implementation detail.
    func testHarnessNamesAreUnchanged() {
        XCTAssertEqual(DevNamespace.dataDirName(for: "stalwart"), "mailcal-dev")
        XCTAssertEqual(DevNamespace.dataDirName(for: "stalwart-multi"), "mailcal-dev-multi")
        XCTAssertEqual(DevNamespace.dataDirName(for: "stalwart-imap"), "mailcal-dev-imap")
        XCTAssertEqual(
            DevNamespace.keychainService(for: "stalwart"), "\(Brand.appID).dev.stalwart")
        XCTAssertEqual(
            DevNamespace.keychainService(for: "stalwart-multi"),
            "\(Brand.appID).dev.stalwart-multi")
        XCTAssertEqual(
            DevNamespace.keychainService(for: "stalwart-imap"),
            "\(Brand.appID).dev.stalwart-imap")
    }

    /// Production names are what the shipped app's existing state is stored under; changing either
    /// would orphan every user's accounts and mailbox on upgrade.
    func testProductionNamesAreUnchanged() {
        XCTAssertEqual(DevNamespace.prodKeychainService, Brand.appID)
        XCTAssertEqual(DevNamespace.prodDataDirName, "mailcal")
    }

    /// A DEBUG test bundle must itself resolve to a dev namespace, proof the `#if DEBUG` wiring,
    /// not just the pure functions, keeps this build off production state.
    func testThisBuildResolvesToADevNamespace() {
        XCTAssertNotEqual(DevNamespace.currentKeychainService, DevNamespace.prodKeychainService)
        XCTAssertNotEqual(DevNamespace.currentDataDirName, DevNamespace.prodDataDirName)
    }

    /// The AppKit autosave name is varied per namespace, so a dev build neither restores nor
    /// overwrites the installed app's saved split-view layout.
    func testAutosaveNameIsNamespacedInDebug() {
        XCTAssertNotEqual(AppPrefs.autosaveName("AllodiaMailMacSidebarV3"), "AllodiaMailMacSidebarV3")
    }
}
