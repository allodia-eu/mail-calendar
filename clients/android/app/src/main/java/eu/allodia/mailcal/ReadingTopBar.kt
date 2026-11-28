// The reading screen's top app bar (docs/reading-actions.md): back, then archive and delete, then
// the overflow for what acts on the message as a document. The answers are at the foot of the
// screen, within a thumb's reach (ReadingAnswers.kt).
package eu.allodia.mailcal

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource

/** The bar over an open message. [overflow] is the menu drawn last of all. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ReadingTopBar(
    onBack: () -> Unit,
    onArchive: () -> Unit,
    onDelete: () -> Unit,
    overflow: @Composable () -> Unit,
) {
    val ctx = LocalContext.current
    TopAppBar(
        title = {},
        navigationIcon = {
            IconButton(onClick = onBack) {
                Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = L10n.a11y_back(ctx))
            }
        },
        actions = {
            IconButton(onClick = onArchive) {
                Icon(painterResource(R.drawable.ic_archive), contentDescription = L10n.action_archive(ctx))
            }
            IconButton(onClick = onDelete) {
                Icon(
                    painter = painterResource(R.drawable.ic_delete),
                    contentDescription = L10n.action_delete(ctx),
                    tint = MaterialTheme.colorScheme.error,
                )
            }
            overflow()
        },
    )
}
