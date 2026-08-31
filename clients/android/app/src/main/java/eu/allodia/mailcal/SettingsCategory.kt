// The settings taxonomy, the same categories, in the same order, under the same names as the
// macOS sidebar and the Windows source-list (docs/settings.md), so a support answer like
// "Settings → Reading" is true on every platform. On a phone the desktop sidebar+detail becomes a
// hub of category rows, each opening its own screen. NOTIFICATIONS is the one mobile-only
// category: it holds the new-mail notification toggle and Android's battery-exemption card:
// surfaces the desktops don't have yet (docs/background-sync.md known gap).
package eu.allodia.mailcal

import android.content.Context
import androidx.annotation.DrawableRes

internal enum class SettingsCategory {
    ALLODIA,
    GENERAL,
    CALENDAR,
    READING,
    COMPOSING,
    SIGNATURES,
    NOTIFICATIONS,
    PRIVACY,
    ACCOUNTS,
    ADVANCED,
    DIAGNOSTICS,
    ABOUT,
    ;

    fun title(ctx: Context): String = when (this) {
        // The category and the card inside it are the same thing named twice otherwise, and the
        // product's own name belongs in one string.
        ALLODIA -> L10n.settings_allodia_heading(ctx)
        GENERAL -> L10n.settings_category_general(ctx)
        CALENDAR -> L10n.settings_category_calendar(ctx)
        READING -> L10n.settings_category_reading(ctx)
        COMPOSING -> L10n.settings_category_composing(ctx)
        SIGNATURES -> L10n.settings_category_signatures(ctx)
        NOTIFICATIONS -> L10n.settings_category_notifications(ctx)
        PRIVACY -> L10n.settings_category_privacy(ctx)
        ACCOUNTS -> L10n.settings_category_accounts(ctx)
        ADVANCED -> L10n.settings_category_advanced(ctx)
        DIAGNOSTICS -> L10n.settings_category_diagnostics(ctx)
        ABOUT -> L10n.settings_category_about(ctx)
    }

    // One plain line under the title saying what's inside, the hub must answer "where would I
    // find X?" without the user having to open every category.
    fun summary(ctx: Context): String = when (this) {
        ALLODIA -> L10n.settings_category_allodia_summary(ctx)
        GENERAL -> L10n.settings_category_general_summary(ctx)
        CALENDAR -> L10n.settings_category_calendar_summary(ctx)
        READING -> L10n.settings_category_reading_summary(ctx)
        COMPOSING -> L10n.settings_category_composing_summary(ctx)
        SIGNATURES -> L10n.settings_category_signatures_summary(ctx)
        NOTIFICATIONS -> L10n.settings_category_notifications_summary(ctx)
        PRIVACY -> L10n.settings_category_privacy_summary(ctx)
        ACCOUNTS -> L10n.settings_category_accounts_summary(ctx)
        ADVANCED -> L10n.settings_category_advanced_summary(ctx)
        DIAGNOSTICS -> L10n.settings_category_diagnostics_summary(ctx)
        ABOUT -> L10n.settings_category_about_summary(ctx)
    }

    // The native icon set (Material Symbols), as macOS uses SF Symbols, the brand's Lucide is
    // for web/design assets, not the platform chrome.
    @get:DrawableRes
    val icon: Int
        get() = when (this) {
            // The person's own account takes account_circle; the mail accounts below take the
            // inbox, which is what they are. Two rows sharing an icon defeats the only thing a
            // hub's icons are for, which is finding a row without reading it.
            ALLODIA -> R.drawable.ic_account_circle
            GENERAL -> R.drawable.ic_settings
            CALENDAR -> R.drawable.ic_calendar_month
            READING -> R.drawable.ic_mail
            COMPOSING -> R.drawable.ic_edit
            SIGNATURES -> R.drawable.ic_signature
            NOTIFICATIONS -> R.drawable.ic_notifications
            PRIVACY -> R.drawable.ic_lock
            ACCOUNTS -> R.drawable.ic_inbox
            ADVANCED -> R.drawable.ic_build
            DIAGNOSTICS -> R.drawable.ic_troubleshoot
            ABOUT -> R.drawable.ic_info
        }

    companion object {
        /**
         * The categories this build shows, in order.
         *
         * [ALLODIA] is absent when the build carries no registration, and the whole category goes
         * rather than its contents: a row that opens an empty screen is worse than no row, and a
         * build from source then opens Settings on General exactly as it did before.
         */
        fun shown(allodiaAvailable: Boolean): List<SettingsCategory> =
            entries.filter { it != ALLODIA || allodiaAvailable }
    }
}
