package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.mailcal_bindings.ManualSetupKind

// Which account types the setup picker shows, and in which order: the core's
// (`manual_setup_kinds`), which already leaves out a browser sign-in this build carries no
// registration for. The mapping is a pure function of the core's list so it can be asserted here:
// the JVM suite never loads the cdylib, so a form that asked the core itself could not be rendered
// in a test at all.
class AccountKindOfferedTest {
    @Test
    fun the_picker_offers_the_core_s_kinds_in_the_core_s_order() {
        assertEquals(
            listOf(
                AccountKind.PASSWORD,
                AccountKind.JMAP,
                AccountKind.MICROSOFT,
                AccountKind.GOOGLE,
                AccountKind.NEXTCLOUD,
                AccountKind.DAV,
            ),
            AccountKind.of(
                listOf(
                    ManualSetupKind.PASSWORD,
                    ManualSetupKind.JMAP,
                    ManualSetupKind.MICROSOFT,
                    ManualSetupKind.GOOGLE,
                    ManualSetupKind.NEXTCLOUD,
                    ManualSetupKind.DAV,
                ),
            ),
        )
    }

    @Test
    fun a_kind_the_core_leaves_out_is_not_offered() {
        assertEquals(
            listOf(AccountKind.PASSWORD, AccountKind.JMAP, AccountKind.NEXTCLOUD, AccountKind.DAV),
            AccountKind.of(
                listOf(ManualSetupKind.PASSWORD, ManualSetupKind.JMAP, ManualSetupKind.NEXTCLOUD, ManualSetupKind.DAV),
            ),
        )
    }

    @Test
    fun every_kind_the_core_can_name_is_one_the_form_draws() {
        assertEquals(AccountKind.entries, AccountKind.of(ManualSetupKind.entries))
    }
}
