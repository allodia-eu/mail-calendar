// The rich composer's header: From / To / Cc / Bcc / Subject, plus the per-message quote-style
// toggle. Split out of RichComposeScreen so the editor below it stays the readable part of that
// file. Cc and Bcc stay hidden behind a chevron on the To row (as Gmail and Thunderbird do), so
// the default header is short, From, To, Subject, and the editor keeps room even with the
// keyboard up. The caller owns the state, including whether Cc/Bcc are revealed.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.QuoteStyleKind
import uniffi.mailcal_bindings.RecipientMatch

@Composable
internal fun ComposerHeaderFields(
    accounts: List<AccountRow>,
    from: AccountRow?,
    onFrom: (AccountRow) -> Unit,
    to: String,
    onTo: (String) -> Unit,
    cc: String,
    onCc: (String) -> Unit,
    bcc: String,
    onBcc: (String) -> Unit,
    subject: String,
    onSubject: (String) -> Unit,
    // Whether Cc/Bcc are revealed, and the toggle for the chevron on the To row.
    showCcBcc: Boolean,
    onToggleCcBcc: () -> Unit,
    // Non-null only for a reply/forward carrying a quoted original, which is the only case that
    // shows the style toggle.
    style: QuoteStyleKind?,
    onStyle: (QuoteStyleKind) -> Unit,
    // Ranked address suggestions for a partially-typed recipient. Answered by the core from synced
    // contacts AND from people the user has written to before, so it is useful on an account with
    // no address book at all. A `null` disables autosuggest entirely (screenshot runs, tests).
    suggestionsFor: ((String) -> List<RecipientMatch>)? = null,
    // Whether the composer opens with the caret in To, a new message the caller did not address.
    focusesTo: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    Column(modifier = modifier) {
        // From is always present, even with a single account, so the sending identity is never a
        // guess the user has to infer from which mailbox they were looking at.
        FromAccountField(accounts = accounts, selected = from, onSelect = onFrom)
        Spacer(modifier = Modifier.height(8.dp))
        // To carries the Cc/Bcc reveal: a chevron that rotates to point up when they're open.
        RecipientField(
            label = L10n.compose_to(ctx),
            value = to,
            onValue = onTo,
            suggestionsFor = suggestionsFor,
            focusesOnAppear = focusesTo,
            trailing = {
                IconButton(onClick = onToggleCcBcc) {
                    Icon(
                        painter = painterResource(R.drawable.ic_keyboard_arrow_down),
                        contentDescription = L10n.compose_show_cc_bcc(ctx),
                        modifier = Modifier.rotate(if (showCcBcc) 180f else 0f),
                    )
                }
            },
        )
        Spacer(modifier = Modifier.height(8.dp))
        if (showCcBcc) {
            RecipientField(
                label = L10n.compose_cc(ctx),
                value = cc,
                onValue = onCc,
                suggestionsFor = suggestionsFor,
            )
            Spacer(modifier = Modifier.height(8.dp))
            RecipientField(
                label = L10n.compose_bcc(ctx),
                value = bcc,
                onValue = onBcc,
                suggestionsFor = suggestionsFor,
            )
            Spacer(modifier = Modifier.height(8.dp))
        }
        // Editable whatever the composer is for. A reply and a forward open with the core's
        // derived `Re:`/`Fwd:` already in it, and renaming a thread here is what the user means by
        // editing it: the field's value is what gets sent.
        OutlinedTextField(
            value = subject,
            onValueChange = onSubject,
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text(L10n.compose_subject(ctx)) },
        )
        Spacer(modifier = Modifier.height(8.dp))
        // The per-message quote-style override. Only shown when the user has opted into it in
        // Settings (the caller passes null otherwise), and only on a reply/forward that actually
        // carries a quote. Flipping it re-styles the quoted original in place; it does not change
        // the persisted app default.
        //
        // Each segment takes an equal share of the row via `weight(1f)`. Without it a segment sizes
        // to its own content, so the longer label overflowed the row at phone width and painted on
        // top of its neighbour; the labels also ellipsize rather than wrap, so a long
        // translation shortens instead of growing the row to two lines.
        if (style != null) {
            SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                val styles = listOf(
                    QuoteStyleKind.INDENTED to L10n.quote_style_indented(ctx),
                    QuoteStyleKind.LINE_AND_HEADER to L10n.quote_style_line_header(ctx),
                )
                styles.forEachIndexed { index, (value, label) ->
                    SegmentedButton(
                        selected = style == value,
                        onClick = { onStyle(value) },
                        shape = SegmentedButtonDefaults.itemShape(index, styles.size),
                        modifier = Modifier.weight(1f),
                    ) {
                        Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
            Spacer(modifier = Modifier.height(8.dp))
        }
    }
}
