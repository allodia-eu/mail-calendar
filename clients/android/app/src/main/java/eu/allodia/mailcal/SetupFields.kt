// The setup form's two text fields, shared by the account panes and by the manual server row.
// Split from AccountSetupScreen.kt to keep that file under the 500-line limit.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.foundation.text.KeyboardOptions

// A single-line text field for the setup form; [placeholder] is shown when empty and
// [keyboardType] tailors the soft keyboard (email/password vs. plain text). [missing] marks a field
// Connect found empty, and takes the focus to it.
@androidx.compose.runtime.Composable
internal fun SetupField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    placeholder: String? = null,
    keyboardType: KeyboardType = KeyboardType.Text,
    missing: Boolean = false,
) {
    val focus = androidx.compose.runtime.remember { androidx.compose.ui.focus.FocusRequester() }
    androidx.compose.runtime.LaunchedEffect(missing) {
        if (missing) runCatching { focus.requestFocus() }
    }
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = Modifier.fillMaxWidth().focusRequester(focus),
        isError = missing,
        singleLine = true,
        label = { Text(label) },
        placeholder = placeholder?.let { text -> { Text(text) } },
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
    )
}

// A single-line, masked field for a password or API token in the setup form.
@androidx.compose.runtime.Composable
internal fun PasswordField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
        label = { Text(label) },
        visualTransformation = PasswordVisualTransformation(),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
    )
}
