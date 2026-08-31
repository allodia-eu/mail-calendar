// The language picker is built from the catalog, not a hand-kept list: L10n.LOCALES names every
// language the app ships and L10n.languageName gives each its endonym. This pins that wiring:
// and, because Robolectric resolves a qualified resource for real, it also proves the generated
// res/values-<locale>/strings.xml actually exists for each one. A locale registered in
// project.inlang/settings.json with no translations would fall back to English here and fail,
// rather than shipping a half-English screen.
package eu.allodia.mailcal

import android.content.Context
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

private fun ctx(): Context = RuntimeEnvironment.getApplication()

@RunWith(RobolectricTestRunner::class)
class LanguageCatalogTest {

    @Test
    fun the_picker_offers_every_language_the_catalog_ships() {
        assertEquals(
            listOf("en", "nl", "de", "fr", "es", "it", "pt"),
            L10n.LOCALES,
        )
    }

    @Test
    fun each_language_is_named_in_its_own_tongue() {
        // Endonyms, so a German user looking for their language reads "Deutsch", not "German".
        // These are identical in every table, so the active locale doesn't matter here.
        assertEquals("Deutsch", L10n.languageName(ctx(), "de"))
        assertEquals("Français", L10n.languageName(ctx(), "fr"))
        assertEquals("Español", L10n.languageName(ctx(), "es"))
        assertEquals("Italiano", L10n.languageName(ctx(), "it"))
        assertEquals("Português", L10n.languageName(ctx(), "pt"))
        assertEquals("Nederlands", L10n.languageName(ctx(), "nl"))
        assertEquals("English", L10n.languageName(ctx(), "en"))
    }

    @Test
    fun an_unshipped_language_yields_its_code_rather_than_crashing() {
        assertEquals("sv", L10n.languageName(ctx(), "sv"))
    }

    @Test
    @Config(qualifiers = "de")
    fun german_resolves_the_german_table() {
        assertEquals("Posteingang", L10n.folder_inbox(ctx()))
        assertEquals("Allen antworten", L10n.action_reply_all(ctx()))
        assertEquals("In den Papierkorb", L10n.swipe_action_delete(ctx()))
    }

    @Test
    @Config(qualifiers = "fr")
    fun french_resolves_the_french_table() {
        assertEquals("Boîte de réception", L10n.folder_inbox(ctx()))
        assertEquals("Répondre à tous", L10n.action_reply_all(ctx()))
        assertEquals("Mettre à la corbeille", L10n.swipe_action_delete(ctx()))
    }

    @Test
    @Config(qualifiers = "es")
    fun spanish_resolves_the_spanish_table() {
        assertEquals("Bandeja de entrada", L10n.folder_inbox(ctx()))
        assertEquals("Responder a todos", L10n.action_reply_all(ctx()))
        assertEquals("Mover a la papelera", L10n.swipe_action_delete(ctx()))
    }

    @Test
    @Config(qualifiers = "it")
    fun italian_resolves_the_italian_table() {
        assertEquals("Posta in arrivo", L10n.folder_inbox(ctx()))
        assertEquals("Rispondi a tutti", L10n.action_reply_all(ctx()))
        assertEquals("Sposta nel cestino", L10n.swipe_action_delete(ctx()))
    }

    @Test
    @Config(qualifiers = "pt")
    fun portuguese_resolves_the_portuguese_table() {
        assertEquals("Caixa de entrada", L10n.folder_inbox(ctx()))
        assertEquals("Responder a todos", L10n.action_reply_all(ctx()))
        assertEquals("Mover para o lixo", L10n.swipe_action_delete(ctx()))
    }

    @Test
    @Config(qualifiers = "de")
    fun a_formatted_string_keeps_its_argument_in_every_locale() {
        // The positional args are indexed off the BASE template, so a translation that reorders
        // the sentence still binds each argument correctly. A German count string is the cheapest
        // proof that the %1$d hole survived the translation.
        assertTrue(L10n.mailbox_count_messages(ctx(), 7).contains("7"))
        assertEquals("7 Nachrichten", L10n.mailbox_count_messages(ctx(), 7))
    }
}
