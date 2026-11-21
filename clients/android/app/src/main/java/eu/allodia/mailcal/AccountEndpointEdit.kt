// An account's servers and sign-in as Settings edits them (docs/accounts.md rule 14): what the
// fields show for a stored address, and what is sent back. Compose-free so the JVM suite reaches
// the rules; saving tries the servers first (SaveServers), so a typo never costs a working
// account.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountEndpoints
import uniffi.mailcal_bindings.ConnectionSecurity

/** The fields as the person left them; null for a server the form drew no field for. */
internal data class EndpointFields(
    val imapHost: String?,
    val imapSecurity: ConnectionSecurity?,
    val smtpHost: String?,
    val smtpSecurity: ConnectionSecurity?,
    val caldavUrl: String?,
    val carddavUrl: String?,
    val username: String,
    val password: String,
)

internal object AccountEndpointEdit {
    /**
     * The servers to save: [original] with what the fields changed. A server the account does not
     * have stays absent, one with no field keeps its value, and an empty password keeps the stored
     * one.
     */
    fun edited(original: AccountEndpoints, fields: EndpointFields): AccountEndpoints {
        fun kept(old: String?, typed: String?): String? = old?.let { typed ?: it }
        return AccountEndpoints(
            imapHost = kept(original.imapHost, fields.imapHost),
            imapSecurity = fields.imapSecurity ?: original.imapSecurity,
            smtpHost = kept(original.smtpHost, fields.smtpHost),
            smtpSecurity = fields.smtpSecurity ?: original.smtpSecurity,
            caldavUrl = kept(original.caldavUrl, fields.caldavUrl),
            carddavUrl = kept(original.carddavUrl, fields.carddavUrl),
            username = fields.username,
            password = fields.password.ifEmpty { null },
        )
    }

    /**
     * The address to store, joined as the core joins it: the bare host when the port is empty or
     * the standard one for [security], and `host:port` when it is not, a port the person wrote
     * into the host winning over the port field, as the setup form dials it.
     */
    fun joined(
        host: String,
        port: String,
        security: ConnectionSecurity,
        standardPort: ((ConnectionSecurity) -> Int)?,
        addressing: ServerAddressing,
    ): String {
        val (bareHost, inHost) = addressing.split(host)
        val chosen = inHost.ifEmpty { port.trim() }
        val standard = standardPort?.invoke(security)?.toString()
        return if (chosen.isEmpty() || chosen == standard) bareHost else addressing.join(bareHost, chosen)
    }

}
