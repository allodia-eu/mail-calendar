// Dev/verification-only account boot: `MAILCAL_DEV_ACCOUNT` / `MAILCAL_DEMO` override which
// accounts the app connects at launch, the local seeded Stalwart harness or the in-memory demo
// provider, instead of the developer's real Keychain accounts. Split out of MailcalModel.swift to
// keep that file under 500 lines; the whole file is `#if DEBUG`, so nothing here ships in a release
// build.
#if DEBUG
import Foundation
import MailcalBindings

extension MailboxModel {
    /// The canned IMAP account config injected when `MAILCAL_DEV_ACCOUNT=stalwart-imap`. IMAP
    /// exercises the full mail-action surface (mark-read/flag, archive, delete, move) plus IDLE
    /// push, unlike the JMAP config. Hand-written rather than built through the shared config
    /// builder (as the JMAP config below is): the harness dials the implicit-TLS IMAP by IP
    /// (`127.0.0.1:12993`) but must present `server_name = localhost`, the only SAN on Stalwart's
    /// self-signed cert, and the builder always derives `server_name` from the dialed host with no
    /// override (adding one purely for the harness would be the sort of test-server knob
    /// `AGENTS.md` forbids). The debug-only TLS policy adds that cert as a custom root from
    /// `MAILCAL_EXTRA_CA`.
    ///
    /// The `[smtp]` and `[caldav]` halves are what make this the *shape* it resembles: mail in a
    /// mailbox beside a calendar on a different server, which is what every IMAP+CalDAV provider
    /// is and what meeting invitations break on (`docs/invitations.md`). IMAP alone, this mode
    /// could not reach that code at all, with no CalDAV there is nothing to answer *on*, and with
    /// no SMTP no reply can be sent, so the invitation card correctly reported that the account
    /// could not answer. Loopback throwaway credentials, debug builds only, in their own store and
    /// credential namespace, the same guarantees as the IMAP half above.
    static let stalwartDevImapToml = """
    [imap]
    addr = "127.0.0.1:12993"
    server_name = "localhost"
    username = "alice@test.local"
    password = "harness-alice-pw"

    [smtp]
    addr = "127.0.0.1:12587"
    server_name = "localhost"
    security = "starttls"

    [caldav]
    base_url = "http://127.0.0.1:28080"
    username = "alice@test.local"
    password = "harness-alice-pw"
    """

    /// One harness JMAP account config, built through the shared config builder, the same FFI the
    /// real setup form uses, so this fixture can't silently drift from the `[jmap]` schema.
    ///
    /// Loopback-only throwaway credentials, never a real account; the harness serves plaintext JMAP
    /// on `127.0.0.1:28080` (shared with the iOS/iPad simulators), and the explicit `http://` is
    /// preserved for this loopback fixture (`docs/jmap.md` rule 4). The connect is made by the Rust
    /// core (reqwest), bypassing App Transport Security, no Info.plist exception. Force-try: the
    /// inputs are constant and valid, so the builder never throws; a throw would be a real bug
    /// worth surfacing loudly.
    static func stalwartDevJmapToml(
        email: String = "alice@test.local",
        password: String = "harness-alice-pw"
    ) -> String {
        try! jmapAccountConfigToml(setup: JmapSetup(
            email: email,
            serverUrl: "http://127.0.0.1:28080",
            password: password
        ))
    }

    /// If a dev-account override is requested via `MAILCAL_DEV_ACCOUNT` / `MAILCAL_DEMO`, boot it
    /// (into an isolated engine store, so harness data never mixes with real accounts) and return
    /// `true` so `start()` returns early. Returns `false` for `personal`/unset, the caller then
    /// connects the real Keychain accounts.
    func startDevAccountIfRequested(observer: SurfaceObserver) -> Bool {
        let devAccount = ProcessInfo.processInfo.environment["MAILCAL_DEV_ACCOUNT"]
        // `MAILCAL_DEMO=1` (or `=demo`) boots the in-memory demo provider, a seeded sample mailbox,
        // no credentials, so the UI can be driven on the simulators.
        if ProcessInfo.processInfo.environment["MAILCAL_DEMO"] == "1" || devAccount == "demo" {
            let app = MailcalApp.newDemo(
                observer: observer,
                logger: CoreLogger(),
                logLevel: DiagnosticsPrefs.coreLogLevel,
                deviceTimezone: deviceTimeZone()
            )
            self.app = app
            self.timezone = app.timezoneSettings()
            needsSetup = false
            app.dispatch(intent: .refreshMail)
            return true
        }
        // `stalwart` → the harness over JMAP; `stalwart-multi` → the same over TWO accounts;
        // `stalwart-imap` → over IMAP (full mail actions + IDLE, needs the dev-harness custom root
        // to trust the self-signed cert). Each gets its own engine store so the harness's test data
        // never mixes with (or lingers among) real accounts.
        if devAccount == "stalwart" {
            needsSetup = false
            // The store directory comes from `DevNamespace`, which reads the same
            // `MAILCAL_DEV_ACCOUNT` this function switched on, `mailcal-dev` here. Deriving it in
            // one place is what keeps the harness stores from colliding with the `personal` one.
            connect([Self.stalwartDevJmapToml()] + storedDevConfigs())
            return true
        }
        // Two harness accounts at once (`mailcal-dev-multi`). It exists for contacts: the engine
        // merges people across accounts on a shared address, and a single-account boot cannot show
        // that, the seeded `shared-*` card is filed in alice's book AND bob's precisely so this
        // mode renders it as one row marked "In 2 accounts". Additive: the single-account loop
        // above is untouched.
        if devAccount == "stalwart-multi" {
            needsSetup = false
            connect([
                Self.stalwartDevJmapToml(),
                Self.stalwartDevJmapToml(email: "bob@test.local", password: "harness-bob-pw"),
            ] + storedDevConfigs())
            return true
        }
        if devAccount == "stalwart-imap" {
            needsSetup = false
            // → `mailcal-dev-imap`, see above
            connect([Self.stalwartDevImapToml] + storedDevConfigs())
            return true
        }
        return false
    }

    /// The Allodia account a previous session in this dev mode signed in to, if any, nothing else.
    ///
    /// A dev launch injects its harness account *instead of* reading the store, which is right for
    /// mail: the canned account is synthesised fresh each time, and connecting a stored copy of it
    /// too would add the same account twice. But the Allodia entry is not a mail account, it holds
    /// no mailbox, and the core takes it back out of the list before anything reads it as one, so
    /// dropping it only made a sign-in made in harness mode look like it had never stuck.
    ///
    /// Which entry that is, is the core's question to answer: matching on the stored shape here
    /// would be a second reader of it, free to disagree the moment either moves. Android does the
    /// same; Windows reads its whole dev namespace, which its own store's per-mode isolation makes
    /// safe there.
    private func storedDevConfigs() -> [String] {
        KeychainStore.configs().filter { isAllodiaAccountConfig(config: $0) }
    }
}
#endif
