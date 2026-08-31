// Screenshot/showcase mode: the `MAILCAL_SHOWCASE` launch flag brings the app up on the in-memory
// showcase dataset (two fictional accounts, seeded sample content) instead of the Keychain's real
// accounts, so store screenshots need no personal mail. The Store wants a screenshot set per listing
// language, so the flag doubles as the language switch: `MAILCAL_SHOWCASE=nl` seeds Dutch sample mail
// to sit under Dutch chrome. Nothing is persisted. The Apple twin of Windows' Services/ShowcaseMode.cs.
//
// `isOn` is hard-`false` outside DEBUG, so no showcase path can run in a shipped build, the call
// sites stay free of `#if DEBUG` and the compiler drops the branches.

import Foundation
import MailcalBindings

/// Reads `MAILCAL_SHOWCASE` / `MAILCAL_SHOWCASE_SCREEN` and resolves what the showcase seeds and shows.
enum ShowcaseMode {
    /// Which screen a screenshot run should drive to once the mailbox has loaded.
    enum Screen: String {
        /// The message list. On a platform with a persistent reading pane the first message is
        /// opened into it; on iPhone the list stands alone.
        case list
        /// The reply composer for the showcase's designated message, pre-filled with sample text.
        case reply
        /// The settings surface.
        case settings
        /// The add-an-account form.
        case addAccount = "add-account"
        /// The calendar grid, opened on today's week.
        case calendar
        /// The showcase's meeting invitation, opened in the reading view, the mail-and-calendar
        /// surface: Accept / Maybe / Decline over a preview of the day the meeting would land on.
        case invitation

        // ---- documentation screens (docs/user-docs.md) ------------------------------------
        //
        // Captured by `showcase.sh <platform> --set docs` for the user guides, not for a store
        // listing. The four `setup*` screens are one flow photographed at four moments, and they
        // are driven by *one* seam, the address to type and whether to run detection, because
        // in a showcase build detection answers from a script keyed on the domain
        // (`MailcalApp::detect_account_settings`). So the core decides which screen appears, and
        // the client never has to fake a result it would not really show.

        /// The email-first setup step with an address typed and detection not yet run.
        case setupEmail = "setup-email"
        /// The detected-settings card for a domain that published everything over HTTPS:
        /// the trusted, calendar-included happy path.
        case setupDetected = "setup-detected"
        /// The same card for settings that were only reachable over a plain-HTTP hop: the
        /// approval gate a user must tick before any password is sent.
        case setupUntrusted = "setup-untrusted"
        /// The manual IMAP/SMTP form, reached because nothing was published for the domain.
        case setupManual = "setup-manual"
        /// Agent access (MCP) in Settings, off, which is how every install starts.
        case mcpOff = "mcp-off"
        /// Agent access switched on, showing the endpoint an assistant connects to.
        case mcpOn = "mcp-on"
        /// The per-account allow list, empty, turning the server on and granting a mailbox
        /// are deliberately two decisions.
        case mcpAccounts = "mcp-accounts"
        /// Direct send, the third toggle, whose tool is absent from the listing while it is off.
        case mcpSend = "mcp-send"
    }

    /// How a documentation run should open the account-setup flow: the address to prefill, and
    /// whether to run detection on it immediately.
    ///
    /// `nil` for every screen that is not part of the setup walkthrough, and for every real
    /// launch, where the field starts empty as it should.
    struct SetupSeed {
        let email: String
        let runDetection: Bool
    }

    /// The Settings category a documentation run should open on, or `nil` to keep the default.
    ///
    /// Addressed by category rather than by adding a screen case per settings pane: the panes are
    /// already an enum (`SettingsCategory`), and a second list of them would be one more thing to
    /// keep in step.
    static var settingsCategory: SettingsCategory? {
        switch screen {
        case .mcpOff, .mcpOn, .mcpAccounts, .mcpSend: return .advanced
        default: return nil
        }
    }

    /// The showcase's fictional addresses, chosen to match what the core's detection script
    /// answers for (see `SHOWCASE_TRUSTED_DOMAIN` / `SHOWCASE_UNTRUSTED_DOMAIN` in the core).
    /// `northwind.example` is also the showcase's own work account, so the setup walkthrough and
    /// the mailbox screenshots tell one story.
    static var setupSeed: SetupSeed? {
        switch screen {
        case .setupEmail: return SetupSeed(email: "eva@northwind.example", runDetection: false)
        case .setupDetected: return SetupSeed(email: "eva@northwind.example", runDetection: true)
        case .setupUntrusted: return SetupSeed(email: "bram@oldschool.example", runDetection: true)
        case .setupManual: return SetupSeed(email: "eva.jansen@example.com", runDetection: true)
        default: return nil
        }
    }

    /// The trimmed, lower-cased flag; nil when unset.
    private static var raw: String? {
        ProcessInfo.processInfo.environment["MAILCAL_SHOWCASE"]?
            .trimmingCharacters(in: .whitespaces)
            .lowercased()
    }

    /// Whether showcase mode is on, `MAILCAL_SHOWCASE` set to anything other than
    /// `0`/`false`/`no`/`off`. Always `false` in a release build.
    static var isOn: Bool {
        #if DEBUG
        guard let raw, !["", "0", "false", "no", "off"].contains(raw) else { return false }
        return true
        #else
        return false
        #endif
    }

    /// The language the flag pins the sample content to (any locale the catalog ships, e.g.
    /// `de`), or nil when it names none (`MAILCAL_SHOWCASE=1`) and the app's own language
    /// choice stands.
    static var languageOverride: String? {
        raw.flatMap { L10n.locales.contains($0) ? $0 : nil }
    }

    /// The locale to seed the showcase mailbox and calendar in: the pinned language when the flag
    /// names one, else whatever the chrome resolved to. Dutch chrome over English mail reads as a
    /// broken screenshot, so the sample content always follows the UI. The code → locale mapping
    /// lives in the core, so all three clients seed the same content for the same language.
    ///
    /// The chrome's language is fixed at process start from `Locale.preferredLanguages`, which a
    /// screenshot run pins with the `-AppleLanguages` launch argument, so it never rewrites the
    /// developer's stored Settings preference.
    static var seedLocale: ShowcaseLocale {
        showcaseLocaleForLanguage(code: languageOverride ?? chromeLanguage())
    }

    /// The screen to drive to; `.list` when unset or unrecognized.
    static var screen: Screen {
        let name = ProcessInfo.processInfo.environment["MAILCAL_SHOWCASE_SCREEN"]?
            .trimmingCharacters(in: .whitespaces)
            .lowercased()
        return name.flatMap(Screen.init(rawValue:)) ?? .list
    }

    /// The sample reply text to open the composer pre-filled with when replying to the showcase's
    /// designated message, else nil, every other message, and every non-showcase run, keeps the
    /// normal empty composer. Plain text; see `docs/composer-security.md` Gate 11.
    static func replyText(account: String, key: String) -> String? {
        guard isOn else { return nil }
        let reply = showcaseReply(locale: seedLocale)
        return account == reply.account && key == reply.messageKey ? reply.text : nil
    }

    /// The first of the app's shipped languages among the OS's preferred ones, the same rule the
    /// generated `L10n` uses to pick its table (whose own resolver is private to it), read off the
    /// same `L10n.locales` list so the two cannot drift.
    private static func chromeLanguage() -> String {
        for language in Locale.preferredLanguages {
            let lower = language.lowercased()
            for locale in L10n.locales where lower == locale || lower.hasPrefix(locale + "-") {
                return locale
            }
        }
        return "en"
    }
}
