package eu.allodia.mailcal

import uniffi.mailcal_bindings.ConnectionSecurity
import uniffi.mailcal_bindings.MailServerKind

/**
 * One server's connection security and port on the manual setup form.
 *
 * The port starts at the standard one for the chosen security and follows the picker, until the
 * user types a port of their own; from then on it is theirs and the picker leaves it alone. A
 * server on a port nobody standardised is the whole reason the manual form exists, so the form
 * must never take back what was typed for it.
 *
 * The rule is `docs/account-autodetect.md`'s and binds every client; this is the Android client's
 * copy of it, Compose-free so it can be tested without composing a screen.
 *
 * The standard port is passed in rather than fetched, because the JVM suite that tests this loads
 * no cdylib: a call across the FFI here would crash every test that composes the screen, and the
 * rule would be one no test could reach. [ManualServerPair.fromCore] is what the app uses;
 * the numbers themselves are pinned against the core in `mailcal-bindings`.
 *
 * A null lookup means no port is suggested. That is what a preview or a test gets, and it is
 * still correct: a blank port submits a bare host, which the core resolves to the same standard
 * port it would have offered.
 */
class ManualServerField private constructor(
    private val standard: ((ConnectionSecurity) -> Int)?,
    val security: ConnectionSecurity,
    val port: String,
    private val typedByHand: Boolean,
) {
    constructor(standard: ((ConnectionSecurity) -> Int)?) : this(
        standard,
        ConnectionSecurity.IMPLICIT_TLS,
        standard.suggest(ConnectionSecurity.IMPLICIT_TLS),
        false,
    )

    /** Whether the picker may still move the port. */
    val followsSecurity: Boolean get() = !typedByHand

    /** The user picked a security. The port follows only while it is still ours. */
    fun choose(chosen: ConnectionSecurity): ManualServerField = ManualServerField(
        standard,
        chosen,
        if (typedByHand) port else standard.suggest(chosen),
        typedByHand,
    )

    /**
     * The user typed in the port field. Clearing it hands the port back to the picker: an empty
     * field submits a bare host, which the core resolves to the same standard port, so there is
     * nothing else a cleared field could mean.
     */
    fun typePort(typed: String): ManualServerField {
        val byHand = typed.isNotBlank()
        return ManualServerField(
            standard,
            security,
            if (byHand) typed else standard.suggest(security),
            byHand,
        )
    }

    /**
     * A detected route filled this server in. Its port travels inside the host (`host:port`), so
     * the field shows what detection found and stops following the picker.
     */
    fun adoptDetected(host: String, detected: ConnectionSecurity): ManualServerField {
        val found = splitHost(host).second
        return ManualServerField(
            standard,
            detected,
            found.ifEmpty { standard.suggest(detected) },
            found.isNotEmpty(),
        )
    }

    /** The `host:port` this field submits, or the bare host when it carries no port of its own. */
    fun dial(host: String): String {
        val typed = host.trim()
        // A host the user already wrote a port into wins: two ports would be a contradiction, and
        // the one in the host field is the one they can see beside the name.
        if (typed.isEmpty() || splitHost(typed).second.isNotEmpty()) return typed
        val chosen = port.trim()
        return if (chosen.isEmpty()) typed else "$typed:$chosen"
    }

    companion object {
        /**
         * Splits a typed server into host and port.
         *
         * Mirrors the core's own split (`mailcal-account`'s `host_and_addr`) deliberately: the
         * core decides the address actually dialled, so a client that split differently would
         * show one port and connect to another. A bare IPv6 literal is not a server either side
         * accepts.
         */
        fun splitHost(input: String): Pair<String, String> {
            val at = input.lastIndexOf(':')
            if (at <= 0 || at == input.length - 1) return input to ""
            val tail = input.substring(at + 1)
            if (!tail.all { it in '0'..'9' }) return input to ""
            return input.substring(0, at) to tail
        }
    }
}

/** The port this lookup offers for [security], or none when there is no lookup. */
private fun ((ConnectionSecurity) -> Int)?.suggest(security: ConnectionSecurity): String =
    this?.invoke(security)?.toString() ?: ""

/**
 * An account's two servers, so the form carries one value rather than four.
 *
 * Built from a lookup the caller supplies, so nothing here reaches the core on its own; the
 * default suggests no port, which is what a preview and a JVM test get.
 */
data class ManualServerPair(
    val imap: ManualServerField = ManualServerField(null),
    val smtp: ManualServerField = ManualServerField(null),
) {
    companion object {
        /** The pair the running app uses, its ports answered by the core. */
        fun fromCore(standardPort: (MailServerKind, ConnectionSecurity) -> Int) = ManualServerPair(
            imap = ManualServerField { security -> standardPort(MailServerKind.IMAP, security) },
            smtp = ManualServerField { security -> standardPort(MailServerKind.SMTP, security) },
        )
    }
}
