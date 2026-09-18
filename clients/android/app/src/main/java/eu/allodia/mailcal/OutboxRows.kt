// The Outbox list's one decision: what each queued send says about itself, and whether it may be
// acted on at all (docs/sending.md).
//
// Compose-free on purpose, so the JVM suite can pin it without composing a screen. The failure it
// exists to catch is silent on the screen: a state mapped to the wrong word tells someone their
// message is waiting when it is already on its way, and offering "Send now" on a message that may
// have been delivered is how it arrives twice. Neither looks wrong.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.QueuedRow
import uniffi.mailcal_bindings.QueuedState

/** One unsent message, as the Outbox list draws it. */
internal data class OutboxRowItem(
    /** The account it will be sent from, and the op id an action names together with it. */
    val account: String,
    val op: ULong,
    /** The recipients, comma-joined, or the account when the message is addressed by Bcc alone. */
    val recipients: String,
    /** The subject, or the words the message list uses for mail that has none. */
    val subject: String,
    /** Where the send has got to, in the app's own words. */
    val stateText: String,
    /** The address of the account it goes out from, drawn on every row. */
    val accountText: String,
    /**
     * Whether this row offers its three actions at all.
     *
     * A message in flight is on its way and cannot be called back; one awaiting confirmation may
     * already be in front of its recipients, and offering to send that again is how it arrives
     * twice (docs/sending.md). Both show their state and offer nothing.
     */
    val actionable: Boolean,
)

/**
 * Projects the core's queued sends into the rows the Outbox draws, in the order the core gave
 * them: oldest first, which is the order they will go out in.
 *
 * `accounts` resolves an account id to the address the user knows it by. Every row draws it
 * (docs/folder-pane.md, rule 18), because this list is the one place in the app holding every
 * account's mail at once and nothing else on the row says which identity a message is waiting on.
 */
internal fun outboxRows(
    queued: List<QueuedRow>,
    accounts: List<AccountRow>,
    ctx: Context,
): List<OutboxRowItem> {
    val emails = accounts.associate { it.id to it.email }
    return queued.map { row ->
        // An account the user has since removed still names something rather than nothing.
        val account = emails[row.account] ?: row.account
        OutboxRowItem(
            account = row.account,
            op = row.op,
            // Both lines can genuinely come back empty, and an empty line reads as a rendering
            // fault rather than as a fact. A blank subject is ordinary mail, which is why there
            // are already words for it; an empty To is a message addressed by Bcc alone, and the
            // account it goes from is then the only thing known about where it is going.
            recipients = row.to.ifEmpty { account },
            subject = row.subject.ifEmpty { L10n.mail_no_subject(ctx) },
            stateText = queuedStateText(row.state, ctx),
            accountText = account,
            actionable = isActionable(row.state),
        )
    }
}

/** What a queued send's state is called on screen. */
internal fun queuedStateText(state: QueuedState, ctx: Context): String = when (state) {
    QueuedState.SENDING -> L10n.outbox_sending(ctx)
    QueuedState.UNCONFIRMED -> L10n.outbox_unconfirmed(ctx)
    QueuedState.WAITING -> L10n.outbox_waiting(ctx)
}

/**
 * Whether a queued send may be acted on.
 *
 * Only `WAITING`. A state a later core adds arrives here as not actionable, which is the safe
 * way for this to fall: a new state read as actionable would offer a retry on a message nobody
 * has decided is safe to retry.
 */
internal fun isActionable(state: QueuedState): Boolean = state == QueuedState.WAITING
