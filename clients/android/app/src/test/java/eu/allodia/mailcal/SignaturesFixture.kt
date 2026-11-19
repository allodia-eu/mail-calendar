// Settings → Signatures as the core would draw it, for the suites that render it. The two rules are
// stand-ins that answer as the core's do (`crates/mailcal-app`, where the rules themselves are
// tested): this suite never loads the cdylib, so `signatureNameRefusal` and `signatureImageRefusal`
// cannot be called here.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountSignatures
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.SignatureEntry
import uniffi.mailcal_bindings.SignatureImageRefusal
import uniffi.mailcal_bindings.SignatureIntent
import uniffi.mailcal_bindings.SignatureOutcome
import uniffi.mailcal_bindings.SignaturePicker
import uniffi.mailcal_bindings.SignatureRefusal
import uniffi.mailcal_bindings.SignaturesPage

// `FREE` and `HELD` are AccountSettingsFixture.kt's.
internal val SIGNATURE_RUNNING = ControlState(enabled = false, running = true)

internal const val FAKE_SIGNATURE_IMAGE_LIMIT = 512L * 1024

internal fun fakeNameRefusal(name: String): SignatureRefusal? =
    if (name.isBlank()) SignatureRefusal.BlankName else null

internal fun fakeImageRefusal(mediaType: String, size: Long): SignatureImageRefusal? = when {
    mediaType.lowercase() !in setOf("image/png", "image/jpeg", "image/gif") -> SignatureImageRefusal.Format
    size > FAKE_SIGNATURE_IMAGE_LIMIT -> SignatureImageRefusal.TooLarge(FAKE_SIGNATURE_IMAGE_LIMIT.toULong())
    else -> null
}

internal fun signatureEntry(id: String, name: String, edit: ControlState = FREE, delete: ControlState = FREE) =
    SignatureEntry(id = id, name = name, edit = edit, delete = delete)

internal fun accountSignatures(
    account: String,
    newMessage: String? = null,
    replyForward: String? = null,
    control: ControlState = FREE,
) = AccountSignatures(
    accountId = account,
    email = "$account@test.local",
    newMessage = SignaturePicker(selected = newMessage, control = control),
    replyForward = SignaturePicker(selected = replyForward, control = control),
)

internal fun signaturesPage(
    signatures: List<SignatureEntry> = emptyList(),
    accounts: List<AccountSignatures> = emptyList(),
    add: ControlState = FREE,
    newId: String = "sig-new",
    save: ControlState = FREE,
    outcome: SignatureOutcome? = null,
) = SignaturesPage(
    signatures = signatures,
    accounts = accounts,
    add = add,
    newId = newId,
    save = save,
    outcome = outcome,
)

internal fun testSignaturesUi(
    page: SignaturesPage? = signaturesPage(),
    bodyHtml: (String) -> String? = { null },
    onChange: (SignatureIntent) -> Unit = {},
) = SignaturesUi(
    page = page,
    bodyHtml = bodyHtml,
    onChange = onChange,
    nameRefusal = ::fakeNameRefusal,
    imageRefusal = ::fakeImageRefusal,
)
