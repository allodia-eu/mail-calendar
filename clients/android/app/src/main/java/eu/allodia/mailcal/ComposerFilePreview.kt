// The square at the start of a composer's file tile: the file itself shrunk when the core names it
// a picture (`ComposerFileTile.preview`), the attachment glyph otherwise (docs/composer-layout.md,
// rule 4; docs/icons.md).
package eu.allodia.mailcal

import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.util.Log
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import java.io.File
import kotlin.math.ceil
import kotlin.math.min
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

private const val TAG = "Mailcal"

private val PREVIEW_SIDE = 28.dp

/**
 * The picture at [path], decoded no larger than needed to fill a square of [sidePx], or null when it
 * cannot be decoded. Never decoded at its own size: a phone's photo is tens of megabytes as pixels.
 */
internal fun decodeComposerPreview(path: String, sidePx: Int): Bitmap? = try {
    ImageDecoder.decodeBitmap(ImageDecoder.createSource(File(path))) { decoder, info, _ ->
        val width = info.size.width
        val height = info.size.height
        val scale = sidePx.toDouble() / min(width, height)
        if (scale < 1.0) {
            decoder.setTargetSize(
                ceil(width * scale).toInt().coerceAtLeast(1),
                ceil(height * scale).toInt().coerceAtLeast(1),
            )
        }
    }
} catch (e: Exception) {
    Log.w(TAG, "composer preview: ${e.javaClass.simpleName}")
    null
}

/** The tile's square: the picture once it has been decoded, the glyph until then and without one. */
@Composable
internal fun ComposerFilePreview(path: String?) {
    val sidePx = with(LocalDensity.current) { PREVIEW_SIDE.roundToPx() }
    val picture by produceState<ImageBitmap?>(initialValue = null, path, sidePx) {
        value = path?.let { withContext(Dispatchers.IO) { decodeComposerPreview(it, sidePx) } }
            ?.asImageBitmap()
    }
    val shape = RoundedCornerShape(6.dp)
    val shown = picture
    if (shown != null) {
        Image(
            bitmap = shown,
            contentDescription = null,
            contentScale = ContentScale.Crop,
            modifier = Modifier.size(PREVIEW_SIDE).clip(shape).testTag("composer-file-picture"),
        )
    } else {
        Box(
            modifier = Modifier
                .size(PREVIEW_SIDE)
                .clip(shape)
                .background(MaterialTheme.colorScheme.surface)
                .testTag("composer-file-glyph"),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_attachment),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}
