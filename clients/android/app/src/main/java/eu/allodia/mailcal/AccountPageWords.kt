// The words for what the core says on an account's page (`account_page`): what is wrong with a
// use, why its switch cannot be pressed, and what a change came to. The core decides each; this
// only names it.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountChangeNotice
import uniffi.mailcal_bindings.AccountUseBlock
import uniffi.mailcal_bindings.AccountUseStatus
import uniffi.mailcal_bindings.AccountUseSwitch
import uniffi.mailcal_bindings.UseProblem

/**
 * What a use that did not connect says, beneath its switch and on the screen that shows it. Only a
 * calendar or an address book fails on its own; mail decides whether the account connected at all.
 */
internal fun problemText(ctx: Context, capability: AccountCapability, problem: UseProblem): String {
    val calendar = capability == AccountCapability.CALENDAR
    return when (problem) {
        UseProblem.UNREACHABLE -> if (calendar) {
            L10n.settings_account_use_unreachable_calendar(ctx)
        } else {
            L10n.settings_account_use_unreachable_contacts(ctx)
        }
        UseProblem.SIGN_IN_REFUSED -> if (calendar) {
            L10n.settings_account_use_refused_calendar(ctx)
        } else {
            L10n.settings_account_use_refused_contacts(ctx)
        }
        UseProblem.NEEDS_OWN_SIGN_IN -> if (calendar) {
            L10n.settings_account_use_own_sign_in_calendar(ctx)
        } else {
            L10n.settings_account_use_own_sign_in_contacts(ctx)
        }
        UseProblem.SERVER_CHANGED -> if (calendar) {
            L10n.settings_account_use_server_changed_calendar(ctx)
        } else {
            L10n.settings_account_use_server_changed_contacts(ctx)
        }
        UseProblem.NONE_ON_SERVER -> if (calendar) {
            L10n.settings_account_use_none_on_server_calendar(ctx)
        } else {
            L10n.settings_account_use_none_on_server_contacts(ctx)
        }
    }
}

/**
 * What a use's switch says beneath it: what is wrong with the use, then why it cannot be pressed,
 * so neither hides the other. Null when there is nothing to say.
 */
internal fun switchNote(ctx: Context, switch: AccountUseSwitch): String? {
    val status = when (val status = switch.status) {
        is AccountUseStatus.Problem -> problemText(ctx, switch.capability, status.problem)
        AccountUseStatus.Withheld -> L10n.settings_account_use_withheld(ctx)
        null -> null
    }
    val blocked = when (switch.blocked) {
        AccountUseBlock.LAST_USE -> L10n.settings_account_use_last(ctx)
        AccountUseBlock.NEEDS_CONTACTS -> L10n.settings_account_colleagues_needs_contacts(ctx)
        AccountUseBlock.NEEDS_MAIL -> L10n.settings_account_shared_needs_mail(ctx)
        AccountUseBlock.NOT_OFFERED -> L10n.settings_account_use_not_offered(ctx)
        AccountUseBlock.NEEDS_SERVER -> L10n.settings_account_use_needs_endpoint(ctx)
        null -> null
    }
    return listOfNotNull(status, blocked).takeIf { it.isNotEmpty() }?.joinToString("\n")
}

/** What a change on an account's page came to, when the core says it needs saying. */
internal fun changeNotice(ctx: Context, notice: AccountChangeNotice): AccountNotice = when (notice) {
    is AccountChangeNotice.UseFailed -> AccountNotice(
        NoticeKind.FAILURE,
        L10n.settings_account_change_failed_title(ctx),
        notice.detail,
    )
    is AccountChangeNotice.LinkFailed -> AccountNotice(
        NoticeKind.FAILURE,
        L10n.settings_account_change_failed_title(ctx),
        notice.detail,
    )
    is AccountChangeNotice.RemoveFailed ->
        AccountNotice(NoticeKind.FAILURE, "", L10n.remove_account_failed(ctx, notice.detail))
}
