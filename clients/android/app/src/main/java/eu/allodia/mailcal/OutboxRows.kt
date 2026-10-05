// The Outbox list's one decision: what each queued send says about itself, and what it may be
// asked to do (docs/sending.md).
//
// Compose-free on purpose, so the JVM suite can pin it without composing a screen. The failure it
// exists to catch is silent on the screen: a state mapped to the wrong word tells someone their
// message is waiting when it is already on its way, and offering "Send now" on a message that may
// have been delivered is how it arrives twice. Neither looks wrong.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.OutboxIntent
import uniffi.mailcal_bindings.QueuedRow
import uniffi.mailcal_bindings.QueuedState

/** What a queued send can be asked to do. Each is exactly one [OutboxIntent]. */
internal enum class QueuedAction { SEND_NOW, EDIT, CANCEL, CONFIRM_SENT, CONFIRM_NOT_SENT }

/** One entry in a queued send's menu: the words it shows, and the action it asks for. */
internal data class QueuedMenuItem(val label: String, val action: QueuedAction)

/** The glyph a row draws beside its state, for the two states that need the user. */
internal enum class QueuedStateMark { NONE, WARNING, ERROR }

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
    /** The glyph drawn beside [stateText]. */
    val mark: QueuedStateMark,
    /** The address of the account it goes out from, drawn on every row. */
    val accountText: String,
    /** The row's menu, in order. Empty means the row draws no menu at all. */
    val actions: List<QueuedMenuItem>,
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
            mark = queuedStateMark(row.state),
            accountText = account,
            actions = queuedActions(row.state, row.editable, ctx),
        )
    }
}

/** What a queued send's state is called on screen. */
internal fun queuedStateText(state: QueuedState, ctx: Context): String = when (state) {
    QueuedState.SENDING -> L10n.outbox_sending(ctx)
    QueuedState.UNCONFIRMED -> L10n.outbox_unconfirmed(ctx)
    QueuedState.WAITING -> L10n.outbox_waiting(ctx)
    QueuedState.NOT_SENT -> L10n.outbox_not_sent(ctx)
}

/**
 * The glyph beside a state. An unconfirmed delivery is a warning, not a failure: the message may
 * well be with its recipients. A refused one is an error, and the only state nothing will move on
 * its own.
 */
internal fun queuedStateMark(state: QueuedState): QueuedStateMark = when (state) {
    QueuedState.WAITING, QueuedState.SENDING -> QueuedStateMark.NONE
    QueuedState.UNCONFIRMED -> QueuedStateMark.WARNING
    QueuedState.NOT_SENT -> QueuedStateMark.ERROR
}

/**
 * What a queued send offers, in menu order.
 *
 * A message in flight cannot be called back, so it offers nothing. One whose delivery is
 * unconfirmed may already be in front of its recipients, so it offers only the two answers to that
 * question and never Send Now, Edit or Cancel: its Send Again is the user saying it did not
 * arrive, which is a different intent from a refused message's Send Again. Edit appears only where
 * the core says a composer can hold the message (`editable`); an invitation answer is not one.
 */
internal fun queuedActions(
    state: QueuedState,
    editable: Boolean,
    ctx: Context,
): List<QueuedMenuItem> {
    val edit = QueuedMenuItem(L10n.action_edit_queued(ctx), QueuedAction.EDIT).takeIf { editable }
    return when (state) {
        QueuedState.WAITING -> listOfNotNull(
            QueuedMenuItem(L10n.action_send_now(ctx), QueuedAction.SEND_NOW),
            edit,
            QueuedMenuItem(L10n.action_cancel_send(ctx), QueuedAction.CANCEL),
        )
        QueuedState.SENDING -> emptyList()
        QueuedState.UNCONFIRMED -> listOf(
            QueuedMenuItem(L10n.action_mark_sent(ctx), QueuedAction.CONFIRM_SENT),
            QueuedMenuItem(L10n.action_send_again(ctx), QueuedAction.CONFIRM_NOT_SENT),
        )
        QueuedState.NOT_SENT -> listOfNotNull(
            QueuedMenuItem(L10n.action_send_again(ctx), QueuedAction.SEND_NOW),
            edit,
            QueuedMenuItem(L10n.action_discard(ctx), QueuedAction.CANCEL),
        )
    }
}

/**
 * Whether an action asks the user before it is sent to the core. Only answering that an
 * unconfirmed message did not arrive does: if it did, the recipients receive it twice. A refused
 * message's Send Again is an ordinary send and asks nothing.
 */
internal fun queuedActionNeedsConfirming(action: QueuedAction): Boolean =
    action == QueuedAction.CONFIRM_NOT_SENT

/**
 * The intent an action sends, naming the queued send by its account **and** its op id: an op id
 * is unique only within its own account's queue, and the Outbox holds every account's at once.
 *
 * Edit moves the message back into Drafts: the core saves it there, writes its files into
 * `stagingDirectory` and offers it back through `Surface::ComposeRequest`, which
 * `WithdrawnMessagePane` answers. The directory is read by no other action.
 */
internal fun outboxIntent(
    account: String,
    op: ULong,
    action: QueuedAction,
    stagingDirectory: String,
): OutboxIntent = when (action) {
    QueuedAction.SEND_NOW -> OutboxIntent.SendNow(account, op)
    QueuedAction.EDIT -> OutboxIntent.Edit(account, op, stagingDirectory)
    QueuedAction.CANCEL -> OutboxIntent.Cancel(account, op)
    QueuedAction.CONFIRM_SENT -> OutboxIntent.ConfirmSent(account, op)
    QueuedAction.CONFIRM_NOT_SENT -> OutboxIntent.ConfirmNotSent(account, op)
}
