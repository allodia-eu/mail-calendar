// The contact editor, as a modal bottom sheet over the contacts list, and the small question
// that precedes it when a person is filed in more than one account.
//
// The email and phone fields are LISTS the user adds to and removes from, and their order is the
// card's order: the first address is the person's primary one, which is what the avatar and the
// list row are keyed on. Everything that decides anything lives in ContactEditorState next door,
// so it can be tested without composing this.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.Intent

/**
 * The create/edit form.
 *
 * [onSave] receives the intent the state built; the sheet never assembles one itself, so the
 * validation and the create-versus-edit split have exactly one home.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ContactEditorSheet(
    state: ContactEditorState,
    onSave: (Intent) -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    // Shown only after a Save that could not go through: an error under a field the user has not
    // finished filling in is an accusation, not help.
    var showError by remember { mutableStateOf(false) }
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
                .testTag("contact-editor"),
        ) {
            Text(
                text = if (state.isEditing) {
                    L10n.contacts_edit(ctx)
                } else {
                    L10n.contacts_new(ctx)
                },
                style = MaterialTheme.typography.headlineSmall,
            )
            Spacer(modifier = Modifier.height(16.dp))

            Field(L10n.contacts_first_name(ctx), state.givenName, "contact-given") {
                state.givenName = it
            }
            Field(L10n.contacts_last_name(ctx), state.surname, "contact-surname") {
                state.surname = it
            }
            Field(
                L10n.contacts_section_organizations(ctx),
                state.organization,
                "contact-organization",
            ) { state.organization = it }
            Field(L10n.contacts_section_titles(ctx), state.title, "contact-title") {
                state.title = it
            }

            ValueList(
                heading = L10n.contacts_section_emails(ctx),
                addLabel = L10n.contacts_add_email(ctx),
                removeLabel = L10n.contacts_remove_email(ctx),
                tag = "contact-email",
                keyboard = KeyboardType.Email,
                values = state.emails,
            )
            ValueList(
                heading = L10n.contacts_section_phones(ctx),
                addLabel = L10n.contacts_add_phone(ctx),
                removeLabel = L10n.contacts_remove_phone(ctx),
                tag = "contact-phone",
                keyboard = KeyboardType.Phone,
                values = state.phones,
            )

            // Only a create files a contact somewhere new, and only when there is a choice to
            // make: one address book is a fact, not a decision.
            if (state.picksTarget) {
                AddressBookPicker(state)
            }

            val error = state.error
            if (showError && error != null) {
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = when (error) {
                        ContactFormError.EMPTY -> L10n.contacts_editor_invalid(ctx)
                        ContactFormError.EMAIL -> L10n.contacts_editor_invalid_email(ctx)
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.testTag("contact-editor-error"),
                )
            }

            Spacer(modifier = Modifier.height(16.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                TextButton(onClick = onDismiss) { Text(L10n.action_cancel(ctx)) }
                TextButton(
                    onClick = {
                        val intent = state.intent()
                        if (intent == null) {
                            showError = true
                        } else {
                            onSave(intent)
                        }
                    },
                    modifier = Modifier.testTag("contact-save"),
                ) { Text(L10n.action_save(ctx)) }
            }
        }
    }
}

/**
 * Asks which account's card to edit, when the person is filed in more than one.
 *
 * Its own step rather than a picker inside the editor, because the answer decides what the form
 * is *seeded with*: a merged person's values belong to different cards, and letting the user
 * change accounts mid-edit would have to throw away what they had typed.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ContactCardChoiceSheet(
    cards: List<ContactCardChoice>,
    onPick: (ContactCardChoice) -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp)
                .padding(bottom = 32.dp)
                .testTag("contact-card-choice"),
        ) {
            Text(
                text = L10n.contacts_pick_card(ctx),
                style = MaterialTheme.typography.titleMedium,
            )
            Spacer(modifier = Modifier.height(12.dp))
            cards.forEach { card ->
                TextButton(onClick = { onPick(card) }, modifier = Modifier.fillMaxWidth()) {
                    Text(card.label, modifier = Modifier.fillMaxWidth())
                }
            }
        }
    }
}

/** One scalar field. */
@Composable
private fun Field(label: String, value: String, tag: String, onChange: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
            .testTag(tag),
    )
}

/** One repeating field: a row per value, plus the button that adds another. */
@Composable
private fun ValueList(
    heading: String,
    addLabel: String,
    removeLabel: String,
    tag: String,
    keyboard: KeyboardType,
    values: MutableList<String>,
) {
    Spacer(modifier = Modifier.height(8.dp))
    Text(
        text = heading,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
    )
    // An index-keyed loop over a snapshot list: the row writes back by position, which is what
    // keeps the order on screen the order that is saved.
    values.forEachIndexed { index, value ->
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = value,
                onValueChange = { values[index] = it },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = keyboard),
                modifier = Modifier
                    .weight(1f)
                    .padding(vertical = 4.dp)
                    .testTag("$tag-$index"),
            )
            IconButton(
                onClick = { values.removeAt(index) },
                modifier = Modifier.testTag("$tag-remove-$index"),
            ) {
                Icon(
                    painter = painterResource(R.drawable.ic_close),
                    contentDescription = removeLabel,
                )
            }
        }
    }
    TextButton(onClick = { values.add("") }, modifier = Modifier.testTag("$tag-add")) {
        Text(addLabel)
    }
}

/** Which address book a create files into. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddressBookPicker(state: ContactEditorState) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    Spacer(modifier = Modifier.height(8.dp))
    ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { expanded = it }) {
        OutlinedTextField(
            value = state.target?.label.orEmpty(),
            onValueChange = {},
            readOnly = true,
            label = { Text(L10n.contacts_address_book(ctx)) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            modifier = Modifier
                .menuAnchor(androidx.compose.material3.MenuAnchorType.PrimaryNotEditable)
                .fillMaxWidth()
                .testTag("contact-address-book"),
        )
        ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            state.targets.forEach { target ->
                DropdownMenuItem(
                    text = { Text(target.label) },
                    onClick = {
                        state.target = target
                        expanded = false
                    },
                )
            }
        }
    }
}
