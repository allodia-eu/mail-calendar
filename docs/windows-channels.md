# Windows: two channels, two packages

Windows is the one platform this app ships to twice. The Microsoft Store carries it, and so does a
download we host ourselves, for the very large number of Windows machines that have no Store: a
managed desktop where it was removed, an N edition, an LTSC install, an account that cannot sign in
to it.

Both channels ship the **same build of the app**. Nothing in the product asks which one it came
from, and nothing may start to: a capability that exists in one and not the other would be a second
product to certify, and [`capabilities.md`](capabilities.md) has one Windows column.

What differs is the package around it, and that difference is forced rather than chosen.

## Why they cannot be one package

An MSIX carries an identity: a name, and a **publisher**, which is an X.500 subject.

- **In the Store**, that publisher is a GUID Partner Center issued, and Microsoft re-signs the
  package on ingestion with a certificate carrying exactly that subject. Nobody else holds it.
- **Outside the Store**, Windows compares the publisher in the manifest against the subject of the
  certificate that actually signed the file, and refuses the install if they differ by a character.
  Our certificate's subject is our own legal identity, which is not a Partner Center GUID.

So the two packages have two publishers. A package family name is the identity name plus a hash of
the publisher, which makes them **two packages as far as Windows is concerned**: both can be
installed at once, each with its own Start menu entry and its own entry in Installed apps.

They are given different identity **names** as well, though only the publisher forces the split.
That is so `Get-AppxPackage` answers a support question without anyone having to decode a hash.

| | Store | Direct |
|---|---|---|
| Identity name | `MAILCAL_MSIX_IDENTITY_NAME` | `MAILCAL_MSIX_DIRECT_IDENTITY_NAME` |
| Publisher | `MAILCAL_MSIX_PUBLISHER`, a Partner Center reservation | `MAILCAL_MSIX_DIRECT_PUBLISHER`, the signing certificate's subject |
| Publisher display name | `MAILCAL_MSIX_PUBLISHER_DISPLAY_NAME`, the same in both | ← |
| Signed by | Microsoft, on ingestion | us, before publishing |
| Artifact | `.msixupload` | `.msixbundle` plus an `.appinstaller` |
| Updates | the Store | App Installer, from the `.appinstaller` URL |

`branding/default.env` holds the neutral values and `branding/allodia.env` overrides them
([`branding.md`](branding.md)). `scripts/dev/msix_manifest.py --channel store|direct` is what puts
one set or the other into the committed manifest, and `clients/windows/package.ps1 -Channel Direct`
is what builds the second.

## ⚠️ Both installed at once is one mailbox with two writers

The app keeps its mail in `%LOCALAPPDATA%\Allodia\MailCalendar`, an ordinary path outside the
package container, because a packaged full-trust app gets no redirection there. Two installed
packages therefore read and write **one** store, one log and one credential namespace.

That is the right answer when somebody *moves* between channels: their mail, accounts and settings
are where they left them, and no migration step exists to go wrong. It is the wrong answer when
both are installed and both are open, which is two processes on one SQLite database.

Windows does not prevent it, and neither does anything here. What holds the line is that nobody is
offered both: the Store listing is the Store's, the download page offers the direct build, and each
says to remove the other first. Two smaller consequences follow from the same root and are worth
knowing before somebody reports them as bugs:

- **Both packages claim `mailto` and the app's own URI scheme.** With both installed, Windows
  cannot know which should receive a browser sign-in redirect, so it asks. The dev loop already
  avoids this by registering a different scheme
  (`Program.RegisterProtocolForUnpackaged`); two shipped packages cannot, because the scheme is
  registered with the identity providers.
- **Two Start menu entries under one name.** They are distinguishable only in Installed apps, by
  publisher.

## What the direct channel has to carry that the Store does not

**The Windows App Runtime.** The app is framework-dependent against it, which on the Store is free:
the Store resolves a package dependency. Nothing resolves it on a machine with no Store, so the
runtime's own MSIX is published beside the bundle and named in the `.appinstaller`'s
`<Dependencies>`, for **both** architectures. `package.ps1 -Channel Direct` stages the framework
package out of the restored NuGet graph, and `scripts/dev/appinstaller.py` refuses to write a file
whose dependency set does not cover every architecture the bundle carries. It is the framework
package alone: the Main, Singleton and DDLM packages beside it serve push notifications and
unpackaged apps, and the Store build is given neither.

**A signature.** `appinstaller.py` refuses an unsigned bundle, because an unsigned bundle installs
on no machine and the file pointing at it says nothing is wrong.

**Its own update mechanism.** The `.appinstaller` is the update channel: App Installer remembers
the URL an installation came from and looks there again. So that URL is permanent, and the file at
it is rewritten by every release while the bundles beside it are never touched once published.
`Version` on the file is the package version, and an installation only updates when it moves.

It is asked twice, and the pair is deliberate. `OnLaunch` catches a machine that was asleep, and
`AutomaticBackgroundTask` checks every eight hours whether or not anybody opened the app. A mail
client is the case that makes the second one necessary rather than nice: this one is left running
for days, so launches can be a fortnight apart and, with `OnLaunch` alone, so would the updates be.
`AutomaticBackgroundTask` belongs to the **2021** schema, which wants Windows 10 2004, and the
package's own floor is the same `10.0.19041`: a machine that cannot read the file cannot install
what it points at, so naming that schema gives up nothing.

None of that is visible while it works, which is why Settings → About names the mechanism and can be
asked on the spot. [`updates.md`](updates.md) is that contract, and it covers the Store channel
too, where the answer is the Store's own updates page rather than a check this app could make.

## Known gaps

- **The `ms-appinstaller:` protocol is disabled on consumer machines** (Microsoft, December 2023),
  so the one-click install from a web page does not work and must not be offered. A download page
  links the `.appinstaller` file directly; the browser saves it and opening it runs App Installer.
- **Nothing detects the other channel.** The collision above is handled by copy, not by code.
- **No Store-to-direct handover.** Moving between channels is uninstall, install; the mail survives
  because the data directory is shared, but nobody is told that by the app.
