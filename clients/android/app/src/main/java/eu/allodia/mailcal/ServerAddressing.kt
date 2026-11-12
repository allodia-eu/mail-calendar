package eu.allodia.mailcal

import uniffi.mailcal_bindings.serverAddress
import uniffi.mailcal_bindings.serverAddressParts

/**
 * How a mail server's address splits into its host and its port, and joins back: the core's rule
 * (docs/account-autodetect.md rule 11), so a form never shows one port while the core dials
 * another. An IPv6 literal is written in brackets before its port, and bare it names none.
 *
 * Passed in rather than called, for the reason the standard port is ([ManualServerField]): the
 * JVM suite that tests the forms loads no cdylib. [core] is what the app uses; the rule itself is
 * tested in `mailcal-account`.
 */
class ServerAddressing(
    val split: (String) -> Pair<String, String>,
    val join: (host: String, port: String) -> String,
) {
    companion object {
        /** The core's own split and join. */
        val core = ServerAddressing(
            split = { address -> serverAddressParts(address).let { it.host to it.port } },
            join = { host, port -> serverAddress(host, port) },
        )

        /**
         * No core to ask: the address as typed, with no port read from it or added to it. What a
         * preview or a test that composes a screen gets; nothing is dialled from either.
         */
        val asTyped = ServerAddressing(
            split = { address -> address.trim() to "" },
            join = { host, _ -> host.trim() },
        )
    }
}
