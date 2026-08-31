// Every vendored Material Symbols drawable must actually inflate, and the auto-mirrored ones must
// say so.
//
// These files are fetched from google/material-design-icons and rewritten by a script (the tint is
// stripped, a placeholder fillColor substituted, autoMirrored added). That is exactly the kind of
// mechanical edit that can produce a file which parses as XML but is not a usable VectorDrawable:
// a duplicated attribute, a truncated pathData, and nothing else in the suite opens them. The
// screens that draw them do so through `painterResource`, which on a broken file fails at runtime,
// on a device, in whichever screen happens to use that one icon.
//
// So: inflate all of them here, where it costs nothing and fails loudly.
package eu.allodia.mailcal

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import androidx.core.content.res.ResourcesCompat
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.GraphicsMode

private fun ctx(): Context = RuntimeEnvironment.getApplication()

/** Every `R.drawable.ic_*` the app declares, discovered reflectively so a new icon is covered. */
private fun iconResources(): List<Pair<String, Int>> =
    R.drawable::class.java.fields
        .filter { it.name.startsWith("ic_") }
        .map { it.name to it.getInt(null) }
        .sortedBy { it.first }

// The icons Compose draws inside a mirrored layout: an arrow that keeps pointing left in Arabic is
// a bug you only notice in an RTL locale, so the flag is pinned here rather than eyeballed.
private val MUST_AUTO_MIRROR = setOf(
    "ic_arrow_back",
    "ic_forward",
    "ic_keyboard_arrow_right",
    "ic_reply",
    "ic_reply_all",
    "ic_send",
)

// NATIVE graphics, deliberately: under Robolectric's default LEGACY canvas every draw call is a
// no-op, so a rasterising test would pass on a blank bitmap and prove nothing.
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@RunWith(RobolectricTestRunner::class)
class IconResourcesTest {

    @Test
    fun every_icon_drawable_inflates_and_draws_something() {
        val icons = iconResources()
        // Guards the reflection itself: if this ever came back empty the rest would vacuously pass.
        assertTrue("expected the app to declare icon drawables, found none", icons.size >= 30)
        icons.forEach { (name, id) ->
            val drawable = ResourcesCompat.getDrawable(ctx().resources, id, null)
            assertNotNull("$name failed to inflate", drawable)

            // Inflating is not enough. `pathData` is an opaque string to the resource compiler, so a
            // truncated or malformed path inflates perfectly happily and then draws nothing, which
            // is invisible until someone opens the one screen that uses that icon. Rasterise it and
            // require actual ink.
            val size = 48
            val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
            drawable!!.setBounds(0, 0, size, size)
            drawable.draw(Canvas(bitmap))
            val pixels = IntArray(size * size)
            bitmap.getPixels(pixels, 0, size, 0, 0, size, size)
            assertTrue("$name inflated but drew nothing", pixels.any { it != 0 })
        }
    }

    @Test
    fun the_directional_icons_are_auto_mirrored() {
        val declared = iconResources().map { it.first }.toSet()
        MUST_AUTO_MIRROR.forEach { name ->
            assertTrue("$name is listed as auto-mirrored but no such drawable exists", name in declared)
        }
        iconResources().forEach { (name, id) ->
            val drawable = ResourcesCompat.getDrawable(ctx().resources, id, null)!!
            assertTrue(
                "$name: autoMirrored should be ${name in MUST_AUTO_MIRROR}",
                drawable.isAutoMirrored == (name in MUST_AUTO_MIRROR),
            )
        }
    }
}
