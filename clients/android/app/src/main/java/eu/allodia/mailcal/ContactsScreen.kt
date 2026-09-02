// The contacts list: a search field, an A–Z list of unified people, and the sticky section
// headers it scrolls under.
//
// Every row here is one PERSON, not one provider card, the core has already merged the cards that
// share an address, across accounts. A merged row says so ("In 2 accounts"), which is a
// cross-platform product rule, not a decoration: a user who filed a contact twice and now sees it
// once must be able to find out why (docs/contacts.md).
//
// Contacts can be created and edited here, and both affordances are CONDITIONAL: the create
// button only where there is a writable address book to file one in, the edit button only where
// this person has a card that can be written (docs/contacts.md §3).
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.ContactDetail
import uniffi.mailcal_bindings.ContactRow

/**
 * The contacts screen.
 *
 * [rows] is the core's already-ordered, already-filtered snapshot, the screen does no sorting and
 * no matching of its own, so every client agrees on both. [onSearch] pushes the query **into the
 * core**, which is what lets a match outside the loaded page still be found.
 *
 * [detailFor] is a synchronous local read (no network), so tapping a row opens immediately.
 */
@Composable
internal fun ContactsScreen(
    rows: List<ContactRow>,
    onSearch: (String) -> Unit,
    detailFor: (String) -> ContactDetail?,
    // Account id -> the address the user knows it by, for the detail sheet's provenance labels.
    accountLabels: Map<String, String> = emptyMap(),
    // Whether there is anywhere at all to save a contact. No writable address book, no create
    // button: offering one produces a save that fails after the user has typed everything in.
    canCreate: Boolean = false,
    onCreate: () -> Unit = {},
    // A word about the most recent write, or null when there is nothing to say. "Couldn't confirm"
    // is not "rejected": the change has landed and the next sync heals the local copy.
    writeLine: String? = null,
    // Editing one card of the open person. The screen has already resolved *which* card, or
    // asked, before this is called.
    onEdit: (ContactDetail) -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    var query by remember { mutableStateOf("") }
    var openContact by remember { mutableStateOf<ContactDetail?>(null) }

    Box(modifier = modifier.fillMaxSize()) {
        Column(modifier = Modifier.fillMaxSize()) {
            OutlinedTextField(
                value = query,
                onValueChange = { typed ->
                    query = typed
                    onSearch(typed)
                },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp)
                    .testTag("contacts-search"),
                singleLine = true,
                label = { Text(L10n.contacts_search_placeholder(ctx)) },
                leadingIcon = {
                    Icon(painterResource(R.drawable.ic_search), contentDescription = null)
                },
                trailingIcon = {
                    if (query.isNotEmpty()) {
                        IconButton(
                            onClick = {
                                query = ""
                                // Clearing resets the filter in the CORE as well as the field, so
                                // a narrowing the user can no longer see can never shrink the next
                                // search (the rule mail search follows, docs/search.md).
                                onSearch("")
                            },
                        ) {
                            Icon(
                                painter = painterResource(R.drawable.ic_close),
                                contentDescription = L10n.contacts_search_clear(ctx),
                            )
                        }
                    }
                },
            )

            if (writeLine != null) {
                Text(
                    text = writeLine,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier
                        .padding(horizontal = 16.dp)
                        .padding(bottom = 8.dp)
                        .testTag("contacts-write-status"),
                )
            }

            when {
                rows.isNotEmpty() -> ContactList(rows = rows) { row ->
                    openContact = detailFor(row.id)
                }
                // An empty list means two different things, and saying the wrong one is
                // unhelpful: "no contacts yet" to someone who searched reads as though their
                // contacts vanished.
                query.isNotEmpty() -> EmptyState(
                    title = L10n.contacts_no_results(ctx),
                    body = null,
                )
                else -> EmptyState(
                    title = L10n.contacts_empty(ctx),
                    body = L10n.contacts_empty_body(ctx),
                )
            }
        }

        if (canCreate) {
            FloatingActionButton(
                onClick = onCreate,
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .padding(16.dp)
                    .testTag("contacts-new"),
            ) {
                Icon(
                    painter = painterResource(R.drawable.ic_add),
                    contentDescription = L10n.contacts_new(ctx),
                )
            }
        }
    }

    openContact?.let { detail ->
        ContactDetailSheet(
            detail = detail,
            accountLabels = accountLabels,
            onDismiss = { openContact = null },
            onEdit = {
                openContact = null
                onEdit(detail)
            },
        )
    }
}

/** The A–Z list, with a section header wherever the initial letter changes. */
@Composable
private fun ContactList(rows: List<ContactRow>, onOpen: (ContactRow) -> Unit) {
    LazyColumn(modifier = Modifier.fillMaxSize().testTag("contacts-list")) {
        itemsIndexed(rows, key = { _, row -> row.id }) { index, row ->
            // The header is decided by comparing with the previous row rather than by grouping the
            // list into buckets: the core hands back a flat ordered list, and re-bucketing it here
            // would be a second ordering that could disagree with the first.
            //
            // `itemsIndexed` for the index, not `rows.indexOf(row)`, that scanned the whole list
            // again for every composed item.
            val previous = rows.getOrNull(index - 1)
            if (previous == null || previous.section != row.section) {
                SectionHeader(row.section)
            }
            ContactListRow(row = row, onOpen = { onOpen(row) })
            HorizontalDivider()
        }
    }
}

@Composable
private fun SectionHeader(section: String) {
    Text(
        text = section,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        fontWeight = FontWeight.Bold,
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .padding(horizontal = 16.dp, vertical = 4.dp),
    )
}

@Composable
private fun ContactListRow(row: ContactRow, onOpen: () -> Unit) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpen)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        AvatarView(row.avatar, modifier = Modifier.testTag("contact-avatar"))
        Spacer(modifier = Modifier.size(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                // A card may legitimately carry an address and no name. The core leaves the name
                // EMPTY rather than filling in English text a Dutch reader would be stuck with:
                // supplying the placeholder is the client's job (docs/contacts.md §2).
                text = row.displayName.ifEmpty { L10n.contacts_no_name(ctx) },
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (row.primaryEmail.isNotEmpty()) {
                Text(
                    text = row.primaryEmail,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            // The disclosure a merged row owes the user. Only above one, every ordinary contact
            // would otherwise carry a meaningless "In 1 accounts".
            if (row.accountCount > 1u) {
                Text(
                    text = L10n.contacts_in_accounts(ctx, row.accountCount.toInt()),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary,
                )
            }
        }
    }
}

@Composable
private fun EmptyState(title: String, body: String?) {
    Box(
        modifier = Modifier.fillMaxSize().padding(32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Text(text = title, style = MaterialTheme.typography.titleMedium)
            if (body != null) {
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = body,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
