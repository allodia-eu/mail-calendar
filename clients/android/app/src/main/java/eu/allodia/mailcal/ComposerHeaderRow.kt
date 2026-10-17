// The composer header's rows: a short label at the start, the field filling the rest of the line,
// a hairline under each. Material's text fields are built for forms, and an outline round every
// line of an address block spends a third of a phone's height on boxes, which the message then
// loses as soon as the keyboard is up. Google's own mail app draws the header this way.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.hideFromAccessibility
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp

/** One header line: Material's minimum touch target. */
internal val HEADER_ROW_HEIGHT = 48.dp

/** The label column, wide enough for every catalog locale's From/To/Cc/Bcc, so the fields align. */
private val HEADER_LABEL_WIDTH = 56.dp

/**
 * One row of the header: [label] at the start, [content] filling the line, [trailing] at the end.
 *
 * [labelReadAloud] is false for a row whose input carries the label as its own description (a
 * recipient field): reading both would announce the field's name twice.
 */
@Composable
internal fun ComposerHeaderRow(
    label: String?,
    modifier: Modifier = Modifier,
    labelReadAloud: Boolean = true,
    trailing: (@Composable () -> Unit)? = null,
    content: @Composable RowScope.() -> Unit,
) {
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = HEADER_ROW_HEIGHT)
                // An icon button carries its own 12dp of padding, so the row keeps less for it.
                .padding(start = 16.dp, end = if (trailing == null) 16.dp else 4.dp),
            verticalAlignment = Alignment.Top,
        ) {
            if (label != null) {
                Box(
                    modifier = Modifier.width(HEADER_LABEL_WIDTH).heightIn(min = HEADER_ROW_HEIGHT),
                    contentAlignment = Alignment.CenterStart,
                ) {
                    Text(
                        text = label,
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = if (labelReadAloud) {
                            Modifier
                        } else {
                            Modifier.semantics { hideFromAccessibility() }
                        },
                    )
                }
            }
            content()
            if (trailing != null) {
                Box(modifier = Modifier.heightIn(min = HEADER_ROW_HEIGHT), contentAlignment = Alignment.Center) {
                    trailing()
                }
            }
        }
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
    }
}

/** The text every header input is drawn in, so the rows read as one block. */
@Composable
internal fun headerTextStyle(): TextStyle =
    MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onSurface)

/**
 * A borderless one-line input, its text centred on the row's line, showing [placeholder] while
 * empty. The placeholder is the field's only label, so it is also the field's name to TalkBack,
 * which keeps it once the field holds text and the placeholder is no longer drawn.
 */
@Composable
internal fun HeaderTextField(
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
) {
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = modifier.heightIn(min = HEADER_ROW_HEIGHT).namedBy(placeholder),
        singleLine = true,
        textStyle = headerTextStyle(),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        decorationBox = { inner -> HeaderFieldDecoration(placeholder, value.text.isEmpty(), inner) },
    )
}

/** [HeaderTextField] over a plain string, for a field nothing rewrites from outside (Subject). */
@Composable
internal fun HeaderTextField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
) {
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = modifier.heightIn(min = HEADER_ROW_HEIGHT).namedBy(placeholder),
        singleLine = true,
        textStyle = headerTextStyle(),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        decorationBox = { inner -> HeaderFieldDecoration(placeholder, value.isEmpty(), inner) },
    )
}

private fun Modifier.namedBy(placeholder: String?): Modifier =
    if (placeholder == null) this else semantics { contentDescription = placeholder }

@Composable
private fun HeaderFieldDecoration(placeholder: String?, empty: Boolean, inner: @Composable () -> Unit) {
    Box(modifier = Modifier.heightIn(min = HEADER_ROW_HEIGHT), contentAlignment = Alignment.CenterStart) {
        if (placeholder != null && empty) {
            Text(
                text = placeholder,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        inner()
    }
}
