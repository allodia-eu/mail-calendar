// Seals the account vault under an AES-256-GCM key generated inside the Android Keystore. The key
// cannot be exported, so every encryption and decryption runs in the device's secure hardware and
// the key is never in this process's memory. No user authentication is attached to it: the
// background sync worker reads the vault while the device is locked.
package eu.allodia.mailcal

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyStore
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

internal object KeystoreSealer : Sealer {
    private const val PROVIDER = "AndroidKeyStore"
    private const val ALIAS = "mailcal-account-vault"
    private const val TRANSFORMATION = "AES/GCM/NoPadding"
    private const val IV_BYTES = 12
    private const val TAG_BITS = 128

    // The output is the IV followed by the ciphertext and its tag. The Keystore chooses the IV
    // itself and refuses one supplied by the caller, so a nonce is never reused.
    override fun seal(plaintext: ByteArray, associatedData: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key() ?: generateKey())
        cipher.updateAAD(associatedData)
        return cipher.iv + cipher.doFinal(plaintext)
    }

    override fun open(sealed: ByteArray, associatedData: ByteArray): ByteArray {
        val key = key() ?: throw VaultUnreadable("the vault key is missing")
        if (sealed.size <= IV_BYTES) throw VaultUnreadable("the vault is truncated")
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(TAG_BITS, sealed, 0, IV_BYTES))
        cipher.updateAAD(associatedData)
        return try {
            cipher.doFinal(sealed, IV_BYTES, sealed.size - IV_BYTES)
        } catch (_: AEADBadTagException) {
            throw VaultUnreadable("the vault does not match its key")
        }
    }

    private fun key(): SecretKey? =
        KeyStore.getInstance(PROVIDER).apply { load(null) }.getKey(ALIAS, null) as SecretKey?

    private fun generateKey(): SecretKey {
        val spec = KeyGenParameterSpec.Builder(
            ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
            .apply { init(spec) }
            .generateKey()
    }
}
