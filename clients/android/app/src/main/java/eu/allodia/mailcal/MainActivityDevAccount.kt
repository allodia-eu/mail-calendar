// Dev/verification-only account injection, split out of MainActivityCore.kt: the canned configs a
// `MAILCAL_DEV_ACCOUNT` launch substitutes for the stored accounts, and the harness IMAP cert they
// need trusted.
package eu.allodia.mailcal

import android.util.Log
import uniffi.mailcal_bindings.JmapSetup
import uniffi.mailcal_bindings.isAllodiaAccountConfig
import uniffi.mailcal_bindings.jmapAccountConfigToml

private const val TAG = "Mailcal"

// Dev/verification only: for a `MAILCAL_DEV_ACCOUNT` string extra passed at launch
// (e.g. `adb shell am start -n eu.allodia.mailcal/.MainActivity -e MAILCAL_DEV_ACCOUNT stalwart`),
// boot against the local seeded Stalwart harness (docker/stalwart) by injecting a canned config:
// bypassing the secure store and the setup form, so a developer, or an automated debug run,
// targets the throwaway loopback mailbox instead of personal accounts. `devMode` is decoded once by
// the caller (null on a release/non-debuggable build or for `personal`/unset), so the stored
// accounts are used then. The Android build/run script maps device localhost back to the host
// harness with `adb reverse`; the connection is made by the Rust core (reqwest over raw sockets),
// so it isn't subject to the manifest cleartext policy.
internal fun MainActivity.devConfigs(devMode: String?): List<String>? {
    return when (devMode) {
        "stalwart" -> listOf(stalwartDevJmapToml())
        // Two harness accounts at once. It exists for contacts: the engine merges people across
        // accounts on a shared address, and a single-account boot cannot show that, the seeded
        // `shared-*` card is filed in alice's book AND bob's precisely so this mode renders it as
        // one row marked "In 2 accounts". Additive, so the single-account `stalwart` loop above is
        // untouched.
        "stalwart-multi" -> listOf(
            stalwartDevJmapToml(),
            stalwartDevJmapToml(email = "bob@test.local", password = "harness-bob-pw"),
        )
        "stalwart-imap" -> {
            // IMAP needs the harness's self-signed cert trusted. The host passes it base64-encoded
            // as an intent extra (env vars can't be set via `am start`); install it and point the
            // core's dev-harness custom-root loader at it before connecting.
            installHarnessCa()
            listOf(STALWART_DEV_IMAP_TOML)
        }
        else -> null
    }
}

// The Allodia account a previous dev session signed in to, if any, nothing else.
//
// A dev launch injects a canned harness account *instead of* the stored ones, which is right for
// mail: unlike Apple's and Windows's, this store is not namespaced per dev account, so appending it
// wholesale would drag the developer's real accounts into a harness run. But the Allodia entry is
// not a mail account, it holds no mailbox and the core takes it back out of the list before
// anything reads it as one, so dropping it only made a sign-in made in this mode look like it had
// never stuck.
//
// Which entry that is, is the core's question to answer: a client matching on the stored shape here
// would be a second reader of it, free to disagree the moment either moves. It arrives as a
// parameter so the JVM suite can reach this at all, that suite never loads the cdylib, so a
// function calling the core directly would be a decision no test could see.
internal fun devCarriedOverConfigs(
    stored: List<String>,
    isAllodiaAccount: (String) -> Boolean = ::isAllodiaAccountConfig,
): List<String> = stored.filter(isAllodiaAccount)

// Writes the harness IMAP cert (base64-PEM in the MAILCAL_EXTRA_CA_PEM intent extra) into the app's
// files dir and exports MAILCAL_EXTRA_CA via Os.setenv so the Rust core's dev-harness TLS policy
// adds it as a custom root. No-op if the extra is absent; a failure leaves the IMAP connect to fail
// visibly rather than crashing.
private fun MainActivity.installHarnessCa() {
    val b64 = intent?.getStringExtra("MAILCAL_EXTRA_CA_PEM") ?: return
    try {
        val pem = android.util.Base64.decode(b64, android.util.Base64.DEFAULT)
        val caFile = java.io.File(filesDir, "harness-ca.pem")
        caFile.writeBytes(pem)
        android.system.Os.setenv("MAILCAL_EXTRA_CA", caFile.absolutePath, true)
        Log.i(TAG, "installed harness CA -> ${caFile.absolutePath}")
    } catch (e: Exception) {
        Log.w(TAG, "failed to install harness CA: ${e.javaClass.simpleName}")
    }
}

// Builds the JMAP config for the local Stalwart harness through the shared config builder, the
// same FFI the real setup form uses (accountConfigToml's JMAP counterpart), so this canned fixture
// can't silently drift from the `[jmap]` schema. Loopback-only throwaway credentials, never a real
// account; `http://` is preserved for this local fixture (docs/jmap.md rule 4). Android dials
// localhost because build-and-run.sh maps it back to the host harness with `adb reverse`. The
// inputs are constant and valid, so the builder never throws; a throw would be a real bug worth
// surfacing.
private fun stalwartDevJmapToml(
    email: String = "alice@test.local",
    password: String = "harness-alice-pw",
): String = jmapAccountConfigToml(
    JmapSetup(
        email = email,
        serverUrl = "http://127.0.0.1:28080",
        password = password,
    ),
)

// The canned IMAP config for the local Stalwart harness (full mail actions + IDLE), injected for
// MAILCAL_DEV_ACCOUNT=stalwart-imap. Hand-written rather than built through the config builder
// (as the JMAP config above is): the harness is dialed through Android localhost, but must present
// server_name `localhost`, the only SAN on Stalwart's self-signed cert, and the builder always
// derives server_name from the dialed host with no override (adding one purely for the harness
// would be the sort of test-server knob AGENTS.md forbids). The cert is trusted as a dev-harness
// custom root via MAILCAL_EXTRA_CA. Loopback-only throwaway credentials.
//
// The [smtp] and [caldav] halves are what make this the *shape* it resembles: mail in a mailbox
// beside a calendar on a different server, which is what every IMAP+CalDAV provider is and what
// meeting invitations break on (docs/invitations.md). IMAP alone, this mode could not reach that
// code at all, with no CalDAV there is nothing to answer *on*, and with no SMTP no reply can be
// sent, so the invitation card correctly reported that the account could not answer.
// build-and-run.sh maps all three ports back to the host harness with `adb reverse`.
private val STALWART_DEV_IMAP_TOML = """
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
""".trimIndent()
