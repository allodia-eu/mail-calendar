// One contact's detail, as a modal bottom sheet, the same shape the event detail uses, so the
// two "tap a row to see everything about it" gestures behave alike.
//
// The section that earns the sheet its keep is "Also in": for a person merged from several
// accounts it names them, which is the *explanation* of the "In 2 accounts" badge on the list row.
// Per-value provenance is shown for the same reason, an address the user only has at work should
// be visibly the work one.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.ContactDetail
import uniffi.mailcal_bindings.ContactValue

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ContactDetailSheet(
    detail: ContactDetail,
    // Account id -> the address the user knows that account by. The core's account ids are
    // internal (`alice@test.local@jmap:127.0.0.1:18080`); showing one to a user is both ugly and
    // a leak of how ids are built. Falls back to the id if an account has since been removed.
    accountLabels: Map<String, String>,
    onDismiss: () -> Unit,
    // Editing one of this person's cards. Absent from the sheet entirely when the person has no
    // writable card: a directory contact, or a shared book this account may only read.
    onEdit: () -> Unit = {},
) {
    val ctx = LocalContext.current
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
                .padding(bottom = 32.dp)
                .testTag("contact-detail"),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                AvatarView(
                    avatar = detail.avatar,
                    diameter = 56.dp,
                    modifier = Modifier.testTag("contact-detail-avatar"),
                )
                Spacer(modifier = Modifier.size(12.dp))
                Text(
                    // Empty when every source card is nameless, the core has no locale, so the
                    // placeholder is ours to supply (docs/contacts.md §2).
                    text = detail.displayName.ifEmpty { L10n.contacts_no_name(ctx) },
                    style = MaterialTheme.typography.headlineSmall,
                )
            }
            Spacer(modifier = Modifier.height(16.dp))

            ValueSection(
                L10n.contacts_section_emails(ctx),
                detail.emails,
                detail.accounts.size,
                accountLabels,
            )
            ValueSection(
                L10n.contacts_section_phones(ctx),
                detail.phones,
                detail.accounts.size,
                accountLabels,
            )
            ValueSection(
                L10n.contacts_section_organizations(ctx),
                detail.organizations,
                detail.accounts.size,
                accountLabels,
            )
            ValueSection(
                L10n.contacts_section_titles(ctx),
                detail.titles,
                detail.accounts.size,
                accountLabels,
            )

            // Only shown for an actual merge: naming the single account a normal contact came from
            // is noise, and would make every contact look like a merge.
            if (detail.accounts.size > 1) {
                SectionHeading(L10n.contacts_section_accounts(ctx))
                detail.accounts.forEach { account ->
                    Text(
                        text = accountLabels[account] ?: account,
                        style = MaterialTheme.typography.bodyLarge,
                    )
                }
                Spacer(modifier = Modifier.height(16.dp))
            }

            HorizontalDivider()
            Spacer(modifier = Modifier.height(12.dp))
            // The edit affordance is conditional on there being a card to write, and the note
            // is what stands in its place: a person nothing here can change says so in as many
            // words rather than leaving it to be inferred from an absence (docs/contacts.md §3).
            if (detail.editableCards.isEmpty()) {
                Text(
                    text = L10n.contacts_not_editable(ctx),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.testTag("contact-not-editable"),
                )
            } else {
                TextButton(onClick = onEdit, modifier = Modifier.testTag("contact-edit")) {
                    Text(L10n.contacts_edit(ctx))
                }
            }
        }
    }
}

/**
 * One labelled group of values, each tagged with the accounts carrying it.
 *
 * [totalAccounts] is how many accounts the whole person spans: with only one there is nothing to
 * disambiguate, so the per-value account tags are suppressed rather than repeating the same
 * account name down the sheet.
 */
@Composable
private fun ValueSection(
    heading: String,
    values: List<ContactValue>,
    totalAccounts: Int,
    accountLabels: Map<String, String>,
) {
    if (values.isEmpty()) {
        return
    }
    SectionHeading(heading)
    values.forEach { value ->
        // Stacked, not side by side. Laid out as a Row, the provenance label, several full email
        // addresses joined by commas, takes whatever width it wants and squeezes the value column
        // to nothing, which rendered an address one character per line. A column cannot do that
        // whatever either string's length, and neither of these is short in practice.
        Column(modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
            Text(text = value.value, style = MaterialTheme.typography.bodyLarge)
            if (totalAccounts > 1) {
                Text(
                    text = value.accounts.joinToString(", ") { accountLabels[it] ?: it },
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
    Spacer(modifier = Modifier.height(16.dp))
}

@Composable
private fun SectionHeading(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
    )
    Spacer(modifier = Modifier.size(4.dp))
}
