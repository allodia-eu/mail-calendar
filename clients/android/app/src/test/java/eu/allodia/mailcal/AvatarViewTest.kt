package eu.allodia.mailcal

import android.graphics.Bitmap
import android.graphics.Color as AndroidColor
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import java.io.File
import java.io.FileOutputStream
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.Avatar
import uniffi.mailcal_bindings.Swatch

@RunWith(RobolectricTestRunner::class)
class AvatarViewTest {
    @get:Rule val compose = createComposeRule()

    private val avatar = Avatar(
        initials = "AL",
        light = Swatch(background = "#112233", text = "#fefefe", border = "#010203"),
        dark = Swatch(background = "#445566", text = "#010101", border = "#aabbcc"),
        imagePath = null,
    )

    @Test
    fun the_active_theme_picks_the_cores_whole_swatch() {
        assertEquals(avatar.light, avatar.swatch(dark = false))
        assertEquals(avatar.dark, avatar.swatch(dark = true))
    }

    @Test
    fun a_monogram_is_drawn_but_hidden_from_talkback() {
        compose.setContent {
            AppTheme(Appearance.LIGHT) {
                AvatarView(avatar, modifier = Modifier.testTag("avatar-under-test"))
            }
        }

        compose.onNodeWithTag("avatar-monogram", useUnmergedTree = true).assertExists()
        val semantics = compose.onNodeWithTag(
            "avatar-under-test",
            useUnmergedTree = true,
        ).fetchSemanticsNode().config
        assertTrue(SemanticsProperties.HideFromAccessibility in semantics)
    }

    @Test
    fun a_photo_wins_over_the_monogram() {
        val file = pngFile()
        try {
            compose.setContent {
                AppTheme {
                    AvatarView(
                        avatar.copy(imagePath = file.absolutePath),
                        modifier = Modifier.testTag("avatar-under-test"),
                    )
                }
            }

            compose.waitUntil(5_000) {
                compose.onAllNodesWithTag(
                    "avatar-photo",
                    useUnmergedTree = true,
                ).fetchSemanticsNodes().isNotEmpty()
            }
            compose.onNodeWithTag("avatar-photo", useUnmergedTree = true).assertExists()
            assertEquals(
                0,
                compose.onAllNodesWithTag(
                    "avatar-monogram",
                    useUnmergedTree = true,
                ).fetchSemanticsNodes().size,
            )
        } finally {
            file.delete()
        }
    }

    @Test
    fun an_unreadable_photo_falls_back_to_the_monogram() {
        compose.setContent {
            AppTheme {
                AvatarView(
                    avatar.copy(imagePath = "/path/that/does/not/exist.png"),
                    modifier = Modifier.testTag("avatar-under-test"),
                )
            }
        }

        compose.onNodeWithTag("avatar-monogram", useUnmergedTree = true).assertExists()
    }

    @Test
    fun an_avatar_with_no_letters_draws_the_platform_person_glyph() {
        compose.setContent {
            AppTheme {
                AvatarView(
                    avatar.copy(initials = ""),
                    modifier = Modifier.testTag("avatar-under-test"),
                )
            }
        }

        compose.onNodeWithTag("avatar-placeholder", useUnmergedTree = true).assertExists()
    }

    private fun pngFile(): File {
        val file = File.createTempFile("mailcal-avatar-", ".png")
        val bitmap = Bitmap.createBitmap(8, 8, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(AndroidColor.MAGENTA)
        FileOutputStream(file).use { output ->
            check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output))
        }
        bitmap.recycle()
        return file
    }
}
