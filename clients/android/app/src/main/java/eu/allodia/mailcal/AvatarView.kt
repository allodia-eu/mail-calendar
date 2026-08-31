// The circle beside a person: their contact photo when the core resolved one, otherwise the
// core-derived monogram and colour. Shape and size are Android's; identity stays in the core.
package eu.allodia.mailcal

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.hideFromAccessibility
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.Avatar
import uniffi.mailcal_bindings.Swatch

private const val PHOTO_CACHE_KIB = 8 * 1024

/** The core's complete swatch for the active app theme. */
internal fun Avatar.swatch(dark: Boolean): Swatch = if (dark) this.dark else this.light

/**
 * A sender/contact avatar. The core decides its letters, colour and photo; Android draws a circle.
 *
 * The path names its own content, so it is also a stable decoded-image cache key. Decoding stays
 * off the main thread: a list can reveal dozens of photos in one frame.
 */
@Composable
internal fun AvatarView(
    avatar: Avatar,
    modifier: Modifier = Modifier,
    diameter: Dp = 40.dp,
) {
    val swatch = avatar.swatch(LocalAppDark.current)
    val path = avatar.imagePath
    var photo by remember(path) { mutableStateOf(path?.let(AvatarPhotoCache::get)) }
    LaunchedEffect(path) {
        if (path != null && photo == null) {
            photo = withContext(Dispatchers.IO) { AvatarPhotoCache.load(path) }
        }
    }

    Box(
        modifier = modifier
            .size(diameter)
            .clip(CircleShape)
            .background(parseHexColor(swatch.background))
            // Decoration. The row already announces the person's name; TalkBack reading a letter
            // before every sender would only repeat it, and the fallback glyph says nothing.
            .semantics { hideFromAccessibility() },
        contentAlignment = Alignment.Center,
    ) {
        when {
            photo != null -> Image(
                bitmap = requireNotNull(photo),
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize().testTag("avatar-photo"),
            )
            avatar.initials.isEmpty() -> Icon(
                painter = painterResource(R.drawable.ic_account_circle),
                contentDescription = null,
                tint = parseHexColor(swatch.text),
                modifier = Modifier
                    .fillMaxSize()
                    .padding(diameter * 0.17f)
                    .testTag("avatar-placeholder"),
            )
            else -> Text(
                text = avatar.initials,
                color = parseHexColor(swatch.text),
                fontSize = (diameter.value * 0.4f).sp,
                fontWeight = FontWeight.Medium,
                modifier = Modifier.testTag("avatar-monogram"),
            )
        }
    }
}

/** The no-identity state used only while a deep-linked message has no row or body snapshot. */
internal fun placeholderAvatar(): Avatar = Avatar(
    initials = "",
    light = Swatch(background = "#59636e", text = "#ffffff", border = "#474f58"),
    dark = Swatch(background = "#3b4249", text = "#ffffff", border = "#626d78"),
    imagePath = null,
)

/** Decoded photos, bounded by their approximate ARGB memory rather than by entry count. */
private object AvatarPhotoCache {
    private val images = object : LruCache<String, ImageBitmap>(PHOTO_CACHE_KIB) {
        override fun sizeOf(key: String, value: ImageBitmap): Int =
            ((value.width.toLong() * value.height.toLong() * 4L) / 1024L)
                .coerceAtLeast(1L)
                .coerceAtMost(Int.MAX_VALUE.toLong())
                .toInt()
    }

    @Synchronized
    fun get(path: String): ImageBitmap? = images.get(path)

    @Synchronized
    fun load(path: String): ImageBitmap? {
        images.get(path)?.let { return it }
        val decoded = BitmapFactory.decodeFile(path)?.asImageBitmap() ?: return null
        images.put(path, decoded)
        return decoded
    }
}
