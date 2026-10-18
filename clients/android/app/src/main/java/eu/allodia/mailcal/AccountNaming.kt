// What Settings names an account by (docs/accounts.md rule 8): its address, and, where another
// account has the same address, what tells the two apart.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.AccountDistinction
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.LinkedAccount
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

/** Another account as a link, a picker or a removal names it. */
internal fun accountName(ctx: Context, account: LinkedAccount): String =
    when (val distinction = account.distinction) {
        null -> account.address
        is AccountDistinction.Kind -> L10n.settings_account_distinguished(
            ctx,
            account.address,
            kindLabel(ctx, distinction.kind),
        )
        is AccountDistinction.Server ->
            L10n.settings_account_distinguished(ctx, account.address, distinction.host)
    }

/**
 * The kind an account's own row and page name it by, with its server's host where another account
 * of its kind has its address. Its kind alone adds nothing there: the line already says it.
 */
internal fun kindLine(ctx: Context, entry: AccountEntry): String {
    val kind = kindLabel(ctx, entry.kind)
    val host = (entry.distinction as? AccountDistinction.Server)?.host ?: return kind
    return "$kind · $host"
}

/** The label for an account's kind, as the setup form names the same choice. */
internal fun kindLabel(ctx: Context, kind: CoreAccountKind): String = when (kind) {
    CoreAccountKind.IMAP -> L10n.setup_account_type_password(ctx)
    CoreAccountKind.DAV -> L10n.account_kind_dav(ctx)
    CoreAccountKind.JMAP -> L10n.setup_account_type_jmap(ctx)
    CoreAccountKind.MICROSOFT -> L10n.setup_account_type_microsoft(ctx)
    CoreAccountKind.GOOGLE -> L10n.setup_account_type_google(ctx)
}
