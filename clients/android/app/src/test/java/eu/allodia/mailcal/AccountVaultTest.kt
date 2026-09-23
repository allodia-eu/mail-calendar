// The account vault's layout and its one-time move out of the legacy store. The Keystore is not
// available on the JVM, so a software AES-GCM key stands in for it: what is pinned here is the
// order, the replacement, the migration's sequencing and what an unreadable vault costs, all of
// which decide whether a user keeps their accounts across this release.
package eu.allodia.mailcal

import android.content.Context
import android.content.SharedPreferences
import java.io.IOException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.spec.GCMParameterSpec
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class AccountVaultTest {
    private lateinit var prefs: SharedPreferences
    private val sealer = SoftwareSealer()
    private val legacy = FakeLegacy()
    private val discarded = mutableListOf<String>()

    @Before
    fun setUp() {
        prefs = RuntimeEnvironment.getApplication()
            .getSharedPreferences("account-vault-test", Context.MODE_PRIVATE)
        prefs.edit().clear().commit()
    }

    private fun vault(sealer: Sealer = this.sealer) = AccountVault(prefs, sealer, legacy) { discarded += it }

    @Test
    fun anEmptyVaultHoldsNoAccounts() {
        assertEquals(emptyList<StoredAccount>(), vault().accounts())
    }

    @Test
    fun accountsKeepTheOrderTheyWereAddedIn() {
        vault().save("b", "config-b")
        vault().save("a", "config-a")
        vault().save("c", "config-c")

        assertEquals(listOf("b", "a", "c"), vault().accounts().map { it.id })
    }

    @Test
    fun savingAStoredAccountReplacesItInPlace() {
        vault().save("a", "old")
        vault().save("b", "config-b")
        vault().save("a", "new")

        assertEquals(
            listOf(StoredAccount("a", "new"), StoredAccount("b", "config-b")),
            vault().accounts(),
        )
    }

    @Test
    fun removingAnAccountLeavesTheOthers() {
        vault().save("a", "config-a")
        vault().save("b", "config-b")
        vault().remove("a")

        assertEquals(listOf(StoredAccount("b", "config-b")), vault().accounts())
    }

    @Test
    fun neitherTheIdsNorTheConfigsAreStoredReadably() {
        vault().save("someone@example.eu", "password = \"hunter2\"")

        val onDisk = prefs.all.toString()
        assertFalse(onDisk, onDisk.contains("someone@example.eu"))
        assertFalse(onDisk, onDisk.contains("hunter2"))
    }

    @Test
    fun theLegacyStoreIsMovedInOnceAndThenDeleted() {
        legacy.accounts = listOf(StoredAccount("a", "config-a"), StoredAccount("b", "config-b"))

        assertEquals(legacy.accounts, vault().accounts())
        assertFalse(legacy.exists())

        // Read from the vault from now on, not from the store the migration emptied.
        assertEquals(listOf("a", "b"), vault().accounts().map { it.id })
        assertEquals(1, legacy.reads)
    }

    @Test
    fun aLegacyStoreThatCannotBeReadIsGivenUpOnOnce() {
        legacy.accounts = listOf(StoredAccount("a", "config-a"))
        legacy.failRead = true

        assertEquals(emptyList<StoredAccount>(), vault().accounts())
        assertFalse(legacy.exists())
        assertEquals(1, discarded.size)

        // The app starts clean and a new account is stored as on a first run.
        vault().save("b", "config-b")
        assertEquals(listOf("b"), vault().accounts().map { it.id })
        assertEquals(1, discarded.size)
    }

    @Test
    fun theLegacyStoreIsNotDeletedUntilTheVaultIsWritten() {
        legacy.accounts = listOf(StoredAccount("a", "config-a"))

        runCatching { vault(FailingSealer).accounts() }

        assertTrue(legacy.exists())
    }

    @Test
    fun aLegacyStoreLeftBesideAWrittenVaultIsDeletedWithoutOverwritingIt() {
        vault().save("new", "config-new")
        legacy.accounts = listOf(StoredAccount("old", "config-old"))

        assertEquals(listOf("new"), vault().accounts().map { it.id })
        assertFalse(legacy.exists())
        assertEquals(0, legacy.reads)
    }

    @Test
    fun aVaultNothingCanOpenIsDiscardedOnce() {
        vault().save("a", "config-a")

        assertEquals(emptyList<StoredAccount>(), vault(UnreadableSealer).accounts())
        assertEquals(1, discarded.size)

        // Gone, so the next launch starts clean rather than failing the same way again.
        assertEquals(emptyList<StoredAccount>(), vault(UnreadableSealer).accounts())
        assertEquals(1, discarded.size)
    }

    @Test
    fun aVaultThatMightOpenLaterIsKept() {
        vault().save("a", "config-a")

        runCatching { vault(FailingSealer).accounts() }

        assertEquals(listOf("a"), vault().accounts().map { it.id })
        assertTrue(discarded.isEmpty())
    }

    private class SoftwareSealer : Sealer {
        private val key = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()

        override fun seal(plaintext: ByteArray, associatedData: ByteArray): ByteArray {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key)
            cipher.updateAAD(associatedData)
            return cipher.iv + cipher.doFinal(plaintext)
        }

        override fun open(sealed: ByteArray, associatedData: ByteArray): ByteArray {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, sealed, 0, 12))
            cipher.updateAAD(associatedData)
            return cipher.doFinal(sealed, 12, sealed.size - 12)
        }
    }

    private object UnreadableSealer : Sealer {
        override fun seal(plaintext: ByteArray, associatedData: ByteArray) = plaintext

        override fun open(sealed: ByteArray, associatedData: ByteArray): ByteArray =
            throw VaultUnreadable("the vault key is missing")
    }

    private object FailingSealer : Sealer {
        override fun seal(plaintext: ByteArray, associatedData: ByteArray): ByteArray =
            throw IOException("keystore busy")

        override fun open(sealed: ByteArray, associatedData: ByteArray): ByteArray =
            throw IOException("keystore busy")
    }

    private class FakeLegacy : LegacyAccounts {
        var accounts: List<StoredAccount>? = null
        var failRead = false
        var reads = 0

        override fun exists() = accounts != null

        override fun read(): List<StoredAccount> {
            if (failRead) throw IOException("keyset unreadable")
            reads++
            return accounts.orEmpty()
        }

        override fun delete() {
            accounts = null
        }
    }
}
