// Raises local new-mail notifications from a background-sync outcome (docs/background-sync.md).
// One notification PER MESSAGE, keyed by the message's stable provider key and grouped per account
// (Gmail/Outlook-style), with a per-account group summary. Keying each notification by message key
// (not by account) is deliberate: a later background pass reports different messages, so its
// notifications never REPLACE an earlier still-unseen message's, the high-water-mark advances past
// reported mail, so a clobbered notification would otherwise be lost forever. Tapping opens the
// specific message in the app (EXTRA_ACCOUNT_ID + EXTRA_MESSAGE_KEY on the launch intent).
// Content (sender + subject) is deliberately shown, the user chose it; the OS hides previews on the
// lock screen per their system setting. This is distinct from the never-log-content diagnostic-log
// rule (docs/logging.md).
package eu.allodia.mailcal

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import java.time.Instant
import uniffi.mailcal_bindings.AccountNewMail
import uniffi.mailcal_bindings.BackgroundSyncOutcome
import uniffi.mailcal_bindings.NewMailPreview

object MailNotifier {
    private const val CHANNEL_ID = "new_mail"
    // A constant numeric id: the message key (as the notification *tag*) is what makes each unique,
    // so tags never collide and no id hashing is needed. The account group summary uses its own id.
    private const val MESSAGE_ID = 1
    private const val SUMMARY_ID = 2
    // Intent extras for the per-message deep-link: the Rust account id and the stable message key.
    const val EXTRA_ACCOUNT_ID = "eu.allodia.mailcal.ACCOUNT_ID"
    const val EXTRA_MESSAGE_KEY = "eu.allodia.mailcal.MESSAGE_KEY"

    /// Posts one notification per newly-arrived message (grouped per account, with a summary). A
    /// no-op if nothing arrived, notifications aren't permitted (Android 13+ runtime permission), or
    /// the toggle is off (checked by the caller).
    fun notifyNewMail(context: Context, outcome: BackgroundSyncOutcome) {
        if (outcome.accounts.isEmpty()) return
        if (!canPost(context)) return
        ensureChannel(context)
        val manager = NotificationManagerCompat.from(context)
        for (account in outcome.accounts) {
            val group = groupKey(account.accountId)
            // Each message under its own stable key, so passes stack rather than overwrite.
            for (message in account.messages) {
                manager.notify(message.messageKey, MESSAGE_ID, buildMessage(context, account, message, group))
            }
            // A group summary collapses the account's messages into one expandable stack. Its tag is
            // the (stable) account id, updating it each pass is fine; the keyed child notifications
            // persist independently, so nothing is lost.
            if (account.messages.size > 1) {
                manager.notify(account.accountId, SUMMARY_ID, buildSummary(context, account, group))
            }
        }
    }

    /// The notification group key for one account, bundles that account's per-message notifications.
    private fun groupKey(accountId: String): String = "new_mail:$accountId"

    private fun buildMessage(
        context: Context,
        account: AccountNewMail,
        message: NewMailPreview,
        group: String,
    ): android.app.Notification {
        val builder = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_mail)
            .setContentTitle(senderOf(message))
            .setContentText(message.subject)
            .setSubText(account.accountLabel)
            .setGroup(group)
            .setAutoCancel(true)
            .setContentIntent(openMessage(context, account.accountId, message.messageKey))
        // Show the email's sent time rather than the moment the notification fired, so the
        // timestamp in the notification shade matches the date shown in the message list.
        receivedMillis(message.received)?.let { builder.setWhen(it).setShowWhen(true) }
        return builder.build()
    }

    private fun buildSummary(
        context: Context,
        account: AccountNewMail,
        group: String,
    ): android.app.Notification {
        val style = NotificationCompat.InboxStyle().setBigContentTitle(account.accountLabel)
        account.messages.forEach { style.addLine(lineOf(it)) }
        val extra = account.newCount.toInt() - account.messages.size
        if (extra > 0) style.setSummaryText("+$extra more")
        return NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_mail)
            .setContentTitle(account.accountLabel)
            .setContentText("${account.newCount} new messages")
            .setStyle(style)
            .setGroup(group)
            .setGroupSummary(true)
            .setAutoCancel(true)
            .setContentIntent(openApp(context))
            .build()
    }

    // Opens the app without a specific message (used by the group summary notification).
    private fun openApp(context: Context): PendingIntent {
        val intent = Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
        return PendingIntent.getActivity(
            context,
            0,
            intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }

    /// "Sender, Subject" for one InboxStyle line, preferring the display name.
    private fun lineOf(preview: NewMailPreview): CharSequence =
        "${senderOf(preview)}, ${preview.subject}"

    private fun senderOf(preview: NewMailPreview): String =
        preview.senderName?.takeIf { it.isNotBlank() } ?: preview.sender

    // Parses the RFC3339/ISO-8601 UTC string from the message header to epoch milliseconds.
    // Returns null when the field is absent or unparseable so the OS falls back to the current time.
    private fun receivedMillis(received: String): Long? =
        if (received.isBlank()) null
        else try { Instant.parse(received).toEpochMilli() } catch (_: Exception) { null }

    // Opens the app to a specific message. Each (accountId, messageKey) pair gets its own
    // PendingIntent, using the message key as the request code makes them distinct so Android
    // doesn't coalesce them into a single cached intent and lose the extras.
    private fun openMessage(context: Context, accountId: String, messageKey: String): PendingIntent {
        val intent = Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            .putExtra(EXTRA_ACCOUNT_ID, accountId)
            .putExtra(EXTRA_MESSAGE_KEY, messageKey)
        // Request code must be unique per message so the system does not reuse a cached intent
        // with different extras; a stable hash of the key is sufficient since keys are stable.
        val requestCode = messageKey.hashCode()
        return PendingIntent.getActivity(
            context,
            requestCode,
            intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }

    private fun ensureChannel(context: Context) {
        val channel = NotificationChannel(
            CHANNEL_ID,
            "New mail",
            NotificationManager.IMPORTANCE_DEFAULT,
        ).apply { description = "Notifies you when new mail arrives in the background." }
        context.getSystemService(NotificationManager::class.java)
            .createNotificationChannel(channel)
    }

    private fun canPost(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED
}
