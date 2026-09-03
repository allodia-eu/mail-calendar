# OS integration: cross-platform contract

**Scope.** The two doors the operating system knocks on: a **mail link** (`mailto:`), and a
**share** ("send this file by email"). Plus the question that decides whether the first door is
ever used, becoming the OS's **default mail app**.

**Principle.** What arrives is opaque and hostile, so the shared core decides what it means and a
client only carries it: `mailcal_composer::mailto` decodes a link and
`mailcal_composer::share` decodes a share, both pure and account-free, so a wrapper's whole job is
to convert what its OS handed it and open a composer with the answer. The header allowlist, the
filename and media-type normalisation, the cap, and the decision to offer to become the default
are all one implementation, not five.

**Nothing on either path ever sends.** A link and a share both pre-fill an **editable** composer
that the user must still send themselves.

## A share may not address a message

A `mailto:` link names recipients because that is what it is for, and RFC 6068 bounds what else it
may name. A share does not: an app that hands us a PDF gets to suggest a subject and some body
text, and nothing else. The one route from a share to `To`/`Cc`/`Bcc` is shared **text** that is
itself a mail link, which is then decoded by the same allowlist a tapped link goes through, so
there is no second, laxer parser to get wrong.

## ⚠️ Attachments never come from a URI

`mailto:?attach=` is not in RFC 6068, and it must never be honoured. A handler cannot tell a URI
that came from a local `xdg-email` from one that came from a web page, so honouring `attach` would
let a page attach any local file it can name and send it off the machine. There is no
configuration under which this becomes safe.

Files therefore reach the composer only through a channel that is **itself a user action**: a
share sheet, an "Open With", or an explicit `--attach` argument. Adding a fifth is a decision, not
a refactor.

## What the core decides about a shared file

Both the composer's own file picker and a share resolve their metadata through
`mailcal_composer::{safe_file_name, safe_media_type}`, so a seeded attachment and a picked one are
indistinguishable by the time either is sent.

- **The name is the final path component**, never a path: a shared `../../etc/passwd` attaches as
  `passwd`, and both separators count, since a name shared from Windows reaches the core with its
  backslashes intact.
- **Control characters are dropped.** A CR or LF would end the `Content-Disposition` line and start
  a header of the sharing app's choosing.
- **Bidirectional formatting characters are dropped.** `holiday<U+202E>gpj.exe` renders as
  `holidayexe.jpg` in the recipient's list, which is the oldest attachment trick there is.
- **The name is capped at 200 bytes, through the stem**, so the extension survives: it is what
  decides which application opens the file.
- **A declared media type is used only when well formed**, parameters dropped. `*/*`, which is what
  Android hands a share target that accepts anything, is not a media type; it falls back to the
  extension, then to `application/octet-stream`.
- **At most `MAX_SHARED_ITEMS` (20) files**, duplicates by path collapsed.

**A refusal is reported, never silent.** Every item the core will not attach comes back in
`SharePrefill::rejected` with its reason, because a file the user watched go into a share sheet and
never saw again is one they will assume was attached. A client shows what it could not take.

## Offering to become the default mail app

The core owns *whether to ask*; each platform owns only its own call. `should_offer_default_mail_app`
answers **no** when:

- there is no account yet (a first launch cannot send mail, so the offer asks for a commitment to
  something the person has not seen working);
- the app is already the default;
- the offer has already been put, whatever came of it. **Once.** A prompt closed without an answer
  counts as answered: an unanswered question is not permission to ask again.

Where the host cannot tell whether it is already the default it reports `None`, which is treated as
"not default": offering where we need not is recoverable, staying silent where we are not is the
state this feature exists to change.

The support level is a property of the **build**, not the platform: the same macOS source may set
the handler when signed for Developer ID and may not when sandboxed for the Mac App Store. So a
host reports `SetDirectly` / `OpenSettings` / `Unsupported`, and a build that can do nothing never
shows the offer, because a prompt that cannot lead anywhere is worse than silence.

The permanent way back is **Settings → General → Default mail app** ([`settings.md`](settings.md)),
present on the same condition: only where the build can act.

## Per-platform

Registration, and what each platform can do about the default. The security gates these surfaces
must meet are Gate 12 and Gate 13 in [`composer-security.md`](composer-security.md).

| Platform | `mailto:` registration | Share ingress | Can it ask to be the default? |
|---|---|---|---|
| **macOS** | ⬜ `CFBundleURLTypes` + `LSHandlerRank` | ⬜ Share Extension (`com.apple.share-services`) | **Developer ID only.** `NSWorkspace.setDefaultApplication(at:toOpenURLsWithScheme:)`, which shows a system consent alert. The **App Store build cannot**: the sandbox refuses it, and there is no replacement for `LSSetDefaultHandlerForURLScheme`. |
| **iOS / iPadOS** | ⬜ `CFBundleURLTypes` | ⬜ Share Extension | **Only with Apple's grant.** The `com.apple.developer.mail-client` entitlement is requested by email and excludes the browser entitlement. There is no prompt API; the app deep-links to Settings → Apps → Default Apps. |
| **Windows** | ✅ MSIX `windows.protocol` `mailto` | ⬜ `windows.shareTarget` extension | **Deep link only**, by design since Windows 10: register under `HKCU\Software\RegisteredApplications` and open `ms-settings:defaultapps?registeredAppUser=…`. |
| **Android** | ✅ `ACTION_VIEW` + `ACTION_SENDTO` on scheme `mailto` | ⬜ `ACTION_SEND` / `ACTION_SEND_MULTIPLE` | **No, and nothing to add.** There is no `ROLE_EMAIL` in `RoleManager`; the chooser is the mechanism, and it already works. |
| **Linux** | ✅ desktop `MimeType=x-scheme-handler/mailto` | ⬜ curated `MimeType=` ("Open With") + a local `--attach` | **No, and it cannot even tell.** No default-apps portal was ever shipped, and inside a Flatpak `GAppInfo` has no host application database to ask, which is why [`check-desktop-handoff.sh`](../scripts/ci/check-desktop-handoff.sh) already bans those calls. The desktop entry declares the handler; the user chooses it in their desktop's settings. |

## Known gaps

- **Only the shared core exists so far.** `prefill_from_share` and the offer policy are
  implemented and tested; every ⬜ above is a client that has not been wired to them yet, which is
  why [`capabilities.md`](capabilities.md) claims neither row on any platform.
- **Apple registers for nothing.** No `CFBundleURLTypes`, no URL handling anywhere in
  `clients/apple/`, so a mail link cannot reach it at all. It is the one platform where the
  ordinary `mailto:` case does not work, and it blocks the default-app offer behind it.
- **iOS cannot be a default mail app without Apple's approval**, and that is a request by email,
  not a change to this tree. Until it is granted, iOS can register for `mailto:` and be chosen from
  a share sheet, but will not appear in Default Apps. Nothing in a PR closes this.
- **Linux has no share portal to use.** "Open With" plus a local `--attach` is the closest
  equivalent a desktop offers, and it is reached by the user naming this app, so it satisfies the
  user-action rule above. A file manager's "Send to → Email" reaches us only where it is configured
  to call the default handler.
- **The Mac App Store build can never set the default itself.** The offer must be *absent* there,
  not failing: the sandbox returns an error, and an offer that cannot work is worse than none.

## Enforcement

When you add or change an OS entry point:

1. **Never widen what a URI may carry.** The allowlist is
   `mailcal_composer::parse_mailto`'s, and `attach` is not on it. A client that pulls a field out
   of a URI itself has already left this contract.
2. **Stage bytes, then hand over a path.** A host holding an OS handle (an Android `content://`
   URI, a Windows `StorageFile`, an `NSItemProvider`) copies it into its own private storage first.
   The core reads bytes at submit; they never cross FFI.
3. **Resolve names and types through the core**, never with the platform's own helper: one
   normalisation, or five that disagree about the same file.
4. **Ask the core whether to offer**, and record what came of it. A client that keeps its own
   "have we asked?" flag has just made a second answer that can disagree with Settings.
5. Update this doc's rule **and** its matrix, [`capabilities.md`](capabilities.md) if a
   capability's reach shifted, and write a changelog fragment if a user could notice.
