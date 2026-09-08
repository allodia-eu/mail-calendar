// What the rich composer is for, split out of RichComposeScreen.kt so the JVM suite can hold these
// pure decisions without composing anything.
package eu.allodia.mailcal

// What the rich composer is for. Every mode exposes editable From/To/Cc/Bcc/Subject fields; a reply
// and reply-all open with To/Cc pre-filled from the core (`replyRecipients`) and the Subject with
// the core's derived `Re:`/`Fwd:`, a forward and new message open with empty addresses, so the user
// can adjust anything before sending. Every mode shares the one hardened editor host below.
internal enum class RichComposeMode {
    New,
    Reply,
    ReplyAll,
    Forward,
}

// Whether the composer opens with its Cc/Bcc row already revealed. It must, whenever either
// arrives pre-filled, a reply-all's Cc, or a mail link that named a Cc or Bcc.
//
// Bcc is the one that makes this a rule rather than a nicety: a `mailto:` link is allowed to set
// one (RFC 6068 lists it among the safe fields), so a link can add a silent recipient. Behind the
// collapsed chevron the user would neither see it before sending nor know to look. A recipient
// you cannot see is one you cannot remove. Extracted so the JVM suite can hold it (a check that
// lives only inside a composable is one nothing can fail).
internal fun revealsCcBcc(cc: String, bcc: String): Boolean = cc.isNotBlank() || bcc.isNotBlank()

// Where the caret opens. A reply/forward is already addressed, so writing is the only thing left to
// do and the body takes it; a new message's To is empty and is where the user has to begin. A mail
// link is the exception among new messages, it supplied the recipient, so the body is the place
// there too. One predicate rather than two flags, because exactly one of the two may be focused
// and two flags can disagree. Extracted so the JVM suite can hold it.
internal fun composerOpensInBody(mode: RichComposeMode, to: String): Boolean =
    mode != RichComposeMode.New || to.isNotBlank()

