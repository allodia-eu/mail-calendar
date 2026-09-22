# Apple client (macOS · iPhone · iPad)

One XcodeGen project (`project.yml` is the source of truth; the `.xcodeproj` is generated and
git-ignored) with a single multiplatform `AllodiaMail` target over the shared `MailcalKit` Swift
package. The Rust core is packaged as `Mailcal.xcframework` by `Scripts/build-core.sh`.

## Run (dev loop)

`Scripts/build-and-run.sh` rebuilds the core, regenerates the project, and builds + launches the
**debug** app, ad-hoc signed (`CODE_SIGN_IDENTITY "-"`), no hardened runtime. Great for iterating;
**not** something you can hand to another machine.

```sh
Scripts/build-and-run.sh                       # macOS
Scripts/build-and-run.sh --iphone              # a connected iPhone, else a booted iPhone simulator
Scripts/build-and-run.sh --ipad                # the same, falling back to an iPad simulator
Scripts/build-and-run.sh --iphone --simulator  # a simulator even with a device plugged in
Scripts/build-and-run.sh --device <udid>       # a named one; --list-devices prints what is attached
```

A **physical** iPhone/iPad is the one target that cannot be ad-hoc signed: iOS launches an app only
if a provisioning profile stands behind its signature. So `sdk=iphoneos*` signs automatically
(`project.yml`), against the team id in the git-ignored `signing.local.xcconfig`, written by a
device build, and optionally included by the committed `signing.xcconfig`. That file is also what
lets **Xcode's own Run button** target a device; without it Xcode stops at *"AllodiaMail requires a
provisioning profile"*, since it cannot run the script's team detection itself. Developer Mode has to
be on (device ▸ Settings ▸ Privacy & Security), and the Stalwart harness is loopback-only, so a
device session runs against a real account added in the app.

**The Share Extension has to be switched on before it appears anywhere**, in the dev loop and on a
user's Mac alike: macOS ships every third-party sharing extension disabled
(`docs/os-integration.md`). System Settings ▸ General ▸ Login Items & Extensions ▸ Sharing, or, for
a dev build, `pluginkit -e use -i eu.allodia.mailcal.share`. `pluginkit -mvv -p
com.apple.share-services` lists what the system has registered and which bundle each one came from,
which is the first thing to check when a build of yours is not the one being offered: two checkouts
share a bundle id.

Debugging against the local Stalwart harness (accounts, seed data, logs) is covered by the repo
skills, see [`docs/debugging.md`](../../docs/debugging.md); macOS notifications are raised off the
live runtime and so are reachable from that loop, while **iOS** background sync and its
notifications need real hardware and a loop of their own
([`scripts/dev/device.sh`](../../scripts/dev/device.sh)).

## Packaging (production)

`Scripts/package.sh` is the release/Store twin of the dev loop, and the mirror of
`clients/windows/package.ps1`. Three flows, native Xcode tooling throughout (`xcodebuild archive` /
`-exportArchive`, `notarytool`, `stapler`, `hdiutil`):

```sh
Scripts/package.sh                  # Flow A: notarized Developer-ID .dmg (install on any Mac)
Scripts/package.sh --no-notarize    # Flow A, skip the notary round-trip (fast pipeline check)
Scripts/package.sh --app-store      # Flow B: Apple-Distribution .pkg for the macOS App Store
Scripts/package.sh --ios-app-store  # Flow C: App Store .ipa for iOS/iPadOS
Scripts/package.sh --ios-device     # Flow D: installable Release .ipa for your own iPhone/iPad
Scripts/package.sh --sandboxed      # Flow E: Flow B's app, development-signed to run on this Mac
Scripts/package.sh --version 1.0.1  # stamp the marketing version
```

Every flow builds the **release** core (`build-core.sh --release`), regenerates the project, then
archives the **Release** configuration. The two macOS flows (A, B) turn on Hardened Runtime and the
macOS entitlements (`project.yml`); Flow C archives for the device (`generic/platform=iOS`), which
ignores both and uses the iOS keychain entitlements instead, and it additionally builds the
`aarch64-apple-ios` device slice (the macOS flows pass `--no-device`). The build is **Apple-silicon /
arm64 only** (Apple-silicon Mac, arm64 iPhone + iPad); a universal Mac binary would need adding the
`x86_64-apple-darwin` slice to `build-core.sh` and dropping the macOS `EXCLUDED_ARCHS`.

### One-time setup

1. **Tools:** Xcode 27+ (the first with Swift 6.4, the tools version
   `Packages/MailcalKit/Package.swift` states), `xcodegen` (`brew install xcodegen`) and
   **bash 5** (`brew install bash`). macOS's own `/bin/bash` is 3.2 and every script in this tree
   refuses it.
2. **Paid Apple Developer Program.** The App ID the build signs under is registered with **no capabilities** (the app needs none, its networking, local
   notifications, background fetch, keychain, and App-Sandbox grants are all entitlement/Info.plist
   settings, not App ID capabilities). The App ID + provisioning profiles matter for the **Store**
   flow and for iOS; **Developer ID (Flow A) uses neither**, it's an account-wide certificate.
3. **Certificates:**
   - **Flow A, Developer ID Application** (the direct-distribution cert; needs no App ID or
     provisioning profile). Easiest path, which also creates the private key + CSR for you:
     **Xcode ▸ Settings ▸ Accounts ▸** select the Apple ID **▸** select the team
     **▸ Manage Certificates… ▸** the **+** at the bottom-left **▸ Developer ID Application**. It
     installs straight into your login keychain. (Only a team **Account Holder/Admin** can create
     these. There's a hard limit of a few per account, so don't make
     duplicates.) The portal alternative: developer.apple.com ▸ Certificates ▸ + ▸ *Developer ID
     Application*, upload a CSR made in **Keychain Access ▸ Certificate Assistant ▸ Request a
     Certificate from a Certificate Authority** (save to disk), then double-click the downloaded
     `.cer` to install it.
   - **Flow B, two persistent certs.** Flow B signs the whole app tree itself (see "Packaging for
     the Mac App Store" for why it can't use automatic signing), so both certs must be real,
     persistent identities in your keychain, *not* the cloud-managed ones Xcode fetches transiently:
     - **Apple Distribution** (signs the `.app` and every nested bundle): **Xcode ▸ Settings ▸
       Accounts ▸** the team **▸ Manage Certificates… ▸ + ▸ Apple Distribution**. Installs into your
       login keychain. Copy its exact name into `signing.local.sh` as `APPLE_DISTRIBUTION_IDENTITY`.
     - **Mac Installer Distribution** (signs the Store `.pkg`): developer.apple.com ▸ Certificates ▸
       + ▸ **Mac Installer Distribution**, upload a CSR (Keychain Access ▸ Certificate Assistant ▸
       Request a Certificate…), then double-click the downloaded `.cer`. It prints as
       `3rd Party Mac Developer Installer: … (<TEAM_ID>)`, copy that into `signing.local.sh` as
       `MAC_INSTALLER_IDENTITY`.

   - **A Mac App Store provisioning profile** that lists the Apple Distribution cert. Because the
     archive is dev-signed, nothing in the flow refreshes an auto-managed Store profile to include a
     newly-created cert, so create one **explicitly** (once): developer.apple.com ▸ Profiles ▸ + ▸
     **Mac App Store** ▸ App ID `eu.allodia.mailcal` ▸ pick the Apple Distribution cert ▸ Generate ▸
     Download, then double-click the `.provisionprofile` to install it. `package.sh` auto-finds it in
     either standard profile folder (or set `MAS_PROVISIONING_PROFILE=<path>` in `signing.local.sh`).
     A distribution profile has no device list, so, unlike a *development* profile, it needs no
     registered Mac.

     ⚠️ **The Share Extension is a second App ID, and it needs the App Group too.** Flow B ships
     `eu.allodia.mailcal.share` inside the app, and the App Group is the entire hand-off a share
     travels through (`docs/os-integration.md`): the profile must grant it to **both** ids, or the
     build signs cleanly, installs, appears in the share sheet and then quietly attaches nothing.
     In the portal: Identifiers ▸ **+** ▸ App ID `eu.allodia.mailcal.share` ▸ enable *App Groups* ▸
     assign `group.eu.allodia.mailcal`, then a **Mac App Store** profile for that id as well.
     `package.sh` checks the app's half only, so the extension's is on you.

     (The **development** profile the archive itself uses stays auto-managed via
     `-allowProvisioningUpdates`, and *that* one embeds a device list, so the team still needs **at
     least one registered Mac** (developer.apple.com ▸ Devices; UDID from
     `system_profiler SPHardwareDataType`), or the archive fails "no devices from which to generate a
     provisioning profile".)

   - **A macOS App Development profile carrying the App Group**, kept alongside the Store one and
     not a packaging prerequisite: it is what `build-and-run.sh --macos --sandboxed` signs with, and
     the only way to exercise App Groups on a local build, since the auto-managed profile above
     grants none and ad-hoc signing has no team to anchor one. One per machine, because it lists
     that Mac. [`docs/debugging.md`](../../docs/debugging.md) section 8 has the steps and the two
     ways it goes wrong quietly.

   - **Flow C, iOS/iPadOS: nothing extra to create.** The iOS App Store flow uses **automatic**
     signing end-to-end, so there are **no persistent certs to add to `signing.local.sh`**, just an
     Apple account signed into Xcode and `DEVELOPMENT_TEAM` set. `-allowProvisioningUpdates` fetches or
     creates the **Apple Distribution** cert and the **iOS App Store** provisioning profile at export
     time (iOS `-exportArchive` re-signs nested code correctly, so, unlike Flow B, no hand-signing is
     needed; see "Flow C" below). As with Flow B the *archive* is development-signed, so the team needs
     **at least one registered iPhone/iPad** (developer.apple.com ▸ Devices; UDID from Finder or
     `xcrun devicectl list devices`), or the archive fails "no devices from which to generate a
     provisioning profile".

   Confirm the certs are ready, you should see a `Developer ID Application: … (<TEAM_ID>)` line
   (Flow A) and, for Flow B, `Apple Distribution: … (<TEAM_ID>)` plus
   `3rd Party Mac Developer Installer: … (<TEAM_ID>)`:
   ```sh
   security find-identity -v -p codesigning   # Developer ID + Apple Distribution
   security find-identity -v                  # also lists the installer cert
   ```
   Copy each exact line into the matching `signing.local.sh` variable. (A common slip: pointing
   `DEVELOPER_ID_IDENTITY` at an *Apple Development* cert, `package.sh` rejects that up front, and
   likewise checks both Flow B certs before archiving.)
4. **Notary credentials (Flow A)**, once per machine:
   ```sh
   xcrun notarytool store-credentials AllodiaNotary \
     --apple-id you@example.com --team-id <TEAM_ID> --password <app-specific-password>
   ```
   (Create the app-specific password at appleid.apple.com → Sign-In and Security.)
5. **Local config:** `cp signing.local.sh.example signing.local.sh` and fill it in (Team ID, the
   Developer ID cert's exact name, the notary profile name). `signing.local.sh` is git-ignored;
   nothing secret is committed.

### Flow A, direct distribution (notarized `.dmg`)

The "install on any Mac, including yours, as a real app" path. `package.sh`:

1. Archives the Release app (Developer ID signing, Hardened Runtime, the minimal
   `App/AllodiaMail.macOS.entitlements`).
2. Exports a Developer-ID-signed `AllodiaMail.app`.
3. Notarizes it (`notarytool submit --wait`) and **staples** the ticket into the app, so it launches
   with no Gatekeeper prompt even offline. `--no-notarize` stops before this for a quick pipeline
   check (the resulting app runs on the build machine but is blocked elsewhere).
4. Wraps the app in a compressed `.dmg` (with an `/Applications` drop target) at
   `build/AllodiaMail.dmg`.

Verify the result:

```sh
spctl -a -vvv -t exec build/package/export/AllodiaMail.app   # accepted, source=Notarized Developer ID
stapler validate build/package/export/AllodiaMail.app
```

### Flow B, Mac App Store (`.pkg`), **manual distribution signing; build uploaded + accepted, in review**

`--app-store` archives with the sandbox entitlements (`App/AllodiaMail.appstore.entitlements`,
development-signed), then **signs the whole app tree by hand** with the persistent **Apple
Distribution** cert and builds the installer with the persistent **Mac Installer Distribution** cert.
Upload the result with **Transporter** (Apple's app) or `xcrun altool --upload-app -f <pkg> -t macos
--apiKey <KEY_ID> --apiIssuer <ISSUER_UUID>` (set the two ids **once** in `signing.local.sh` and the
script prints that line filled in, see "Uploading a build" below). What remains before public release is the App Store
**listing** (metadata, screenshots, submitting the build for review), product steps in App Store
Connect, not tooling.

**Why not `xcodebuild -exportArchive` (ITMS-90284).** The first upload (build 1) was **rejected**:
App Store Connect flagged `Contents/Resources/MailcalKit_MailcalUI.bundle` as *"must be signed with
the certificate that is contained in the provisioning profile."* Xcode's automatic (cloud-managed)
export re-signs the app and frameworks with the Apple Distribution cert but **skips nested SPM
resource bundles**, that bundle kept the Apple Development signature from the archive. And the fix
can't be applied after the fact, because the cloud-managed distribution cert is fetched transiently
and left nothing in the keychain to re-sign with. So `package.sh --app-store` now signs everything
itself, inner bundles first, with an explicit persistent cert, and runs a **signature-consistency
gate** (every nested item's cert must match the app's) before building the `.pkg`, the check that
would have caught the rejection before upload.

**Flow B's `.pkg` cannot be installed locally, and its app cannot be launched.** The installer is
signed with the **Mac Installer Distribution** cert, which only the Store's submission pipeline
consumes. Extracting the `.app` and running it does not get around that either: it is killed on
launch with **SIGKILL (137)**, before `main()`, because its **Mac App Store** provisioning profile
is a *production* profile and macOS honours those only for a copy the App Store delivered. The
kernel says `load code signature error 4`, under `amfid: "No matching profile found"` (AMFI -413)
and `taskgated-helper: "Only Development Provisioning Profiles can be installed in System
Settings"` (CPProfileManager -215). This is the macOS counterpart of Flow C's device-list rule
below, and like that one it is not a flag.

**To run the Store's app on this Mac, use Flow E: `Scripts/package.sh --sandboxed`.** It archives
exactly as this flow does and signs through this flow's own pass, swapping only the certificate and
the profile: an Apple Development certificate, and a **macOS App Development** profile that grants
the App Group and lists this Mac, in place of the Store's pair. Entitlements, signing order and every
gate are the Store's, so an entitlement the sandbox needs and the Store build lacks fails here
first, on a Release build with the release core. The profile is the one the dev loop's
`--sandboxed` signs with, and [`docs/debugging.md`](../../docs/debugging.md) section 8 says how to
make one; `MACOS_DEV_PROVISIONING_PROFILE` in `signing.local.sh` points at one kept elsewhere. The
result is `build/release-<VERSION>/AllodiaMail-<VERSION>-Sandboxed.zip`, which runs only on the
Macs the profile lists and keeps its data in the sandbox's container.

⚠️ **The Share Extension keeps the profile the archive embedded**, Xcode's wildcard *Mac Team
Provisioning Profile*, which grants no group, while it is signed claiming
`group.eu.allodia.mailcal`. This flow's pass leaves it the same way, so Flow E is faithful to the
Store build here too. Whether a share still reaches the app under it has not been measured.

For the dev loop, `Scripts/build-and-run.sh --macos --sandboxed` gives the app the same
entitlements in seconds rather than an archive's minutes, with a debug core.

The archive's app (`build/package/AllodiaMail.xcarchive/Products/Applications/AllodiaMail.app`) is
sandboxed too, because `--app-store` archives with
`MACOS_ENTITLEMENTS=App/AllodiaMail.appstore.entitlements` under Apple Development signing. But
⚠️ **as archived its MCP server cannot start, and only MCP**: automatic provisioning resolves the
generic *Mac Team Provisioning Profile*, which grants **no**
`com.apple.security.application-groups`, while the Mac App Store profile grants
`group.eu.allodia.mailcal`. The app still *claims* the group, so the group container exists and is
refused: `deny(1) file-write-create …/mcp.sock`, surfacing as `mcp: could not bind the socket
(permission denied)`. Everything else in the sandbox is faithful, so read an MCP failure in **that**
copy as the profile rather than as the code, and reach for Flow E instead of hand-signing it.

Steering the archive itself onto an App-Groups profile is not a flag we have, deliberately. An
`xcodebuild` setting override is global, and `PROVISIONING_PROFILE_SPECIFIER` is one no override can
scope: it reaches the MailcalKit package targets ("does not support provisioning profiles") and the
Share Extension, whose bundle id is a different App ID. `--sandboxed` gets the app target alone
through `MACOS_PROVISIONING_PROFILE` in `project.yml`, which is the same indirection
`MACOS_ENTITLEMENTS` uses and for the same reason.

For a build that is Distribution-signed as well, install it through **TestFlight for macOS** after
uploading the `.pkg`: that copy is Store-provisioned and carries a receipt, and is what a reviewer
and a user run.

**App Sandbox runtime, verified (2026-07-18).** The Store **mandates the App Sandbox**, whose one
behaviour change is that `homeDirectoryForCurrentUser` (`MailcalModel.swift`, `FileLog.swift`)
resolves to the app **container** instead of `~/.local/share/mailcal`. Confirmed with a sandboxed
build of this app driven against the local harness: the engine store + diagnostic log land in
`~/Library/Containers/eu.allodia.mailcal/Data/.local/share/mailcal` (writable, **no** sandbox denial
the `getpwuid`-escapes-the-container trap does not bite here), the SQLite store opens, JMAP sync
connects/authenticates/downloads under `network.client`, and Keychain items the app owns round-trip
prompt-free across launches. The container is a **different path** from the direct-distribution
build, so data does not carry over between the two.

**Keychain, the Store build uses the data-protection keychain (2026-07-19).** `KeychainHelper.swift`
enables `kSecUseDataProtectionKeychain` whenever the running build carries a `keychain-access-groups`
entitlement, the Store build (via its Mac App Store profile) and iOS, so the Store build keeps
credentials in the iOS-style, access-group-scoped **data-protection keychain**, matching iOS
(verified prompt-free across launches). The Developer ID `.dmg` and the ad-hoc dev build have no such
profile, so they stay on the **file keychain**, and *must*: macOS **SIGKILLs** a Developer-ID app
that declares `keychain-access-groups` with no provisioning profile (exit 137 at launch), and ad-hoc
signing has no team to anchor a group. A single macOS keychain across all three is therefore not
reachable; the runtime gate picks the right one per signing, and the choice is invisible, the three
are separate distributions that never share data.

**Keychain, dev builds have their own namespace, and must be cert-signed (2026-07-22).** The Store
build is isolated by the paragraph above, but the Developer ID `.dmg` and a dev build share one
login keychain, and until this change they shared one service name too. Two independent faults came
out of that, and **each needed its own fix**:

1. *Shared namespace.* A keychain grant is per **(item, code signature)**, so every switch between
   the installed `.dmg` and a dev build made each one a stranger to the other's items, one prompt
   per stored account, in both directions, and a dev-build form-add reordered the real
   `account-index`. `DevNamespace` (`DevNamespace.swift`) now puts every DEBUG build under
   `eu.allodia.mailcal.dev`, with `.dev.stalwart` / `.dev.stalwart-imap` per harness mode. Unlike
   Windows' dev namespace this is **not** limited to the harness modes: `--account personal` is the
   mode that touches real credentials, so it is exactly the one that must be isolated. The same
   type also splits the **engine store** (`~/.local/share/mailcal-dev-personal`) and the
   **preference domain** (a `UserDefaults` suite, the two builds share a bundle id, so they shared
   `.standard`); see [`docs/debugging.md`](../../docs/debugging.md) for the full table and the
   `@SceneStorage` gap.
2. *Unstable dev signature.* This is the one that produced the endless prompts, and a namespace does
   nothing for it. Since Sierra the file keychain gates access on a **partition id** as well as the
   trusted-application list, and for an ad-hoc binary that partition is its **`cdhash:`**, which
   changes on every rebuild. So "Always Allow" was being granted to a binary that would never exist
   again. `build-and-run.sh` used to keep Xcode's ad-hoc signature whenever stdin was not a TTY, so
   every script- or agent-driven build hit this; it now always re-signs with a persistent identity,
   which makes the partition **`teamid:`** and the ACL entry certificate-based, both stable across
   rebuilds.

Measured, not assumed: a cert-signed binary's item is readable prompt-free by a **different** binary
with the same identifier and cert, while an ad-hoc one is not, and neither is an item written with
an "any application" ACL (`security add-generic-password -A`), because the partition check runs
regardless. That last result is why `writeData` deliberately sets **no** custom `kSecAttrAccess`:
the permissive-ACL trick does not work, and stable signing makes it unnecessary.

Building straight from Xcode's Run button still produces an ad-hoc app (`project.yml` commits
`CODE_SIGN_IDENTITY: "-"` so CI can build without a certificate), so that path keeps re-prompting;
use `Scripts/build-and-run.sh`, or set the identity in the scheme.

### Flow C, iOS/iPadOS App Store (`.ipa`), **build uploaded + accepted by App Store Connect; listing/submission remaining**

`--ios-app-store` builds the release core **with** the `aarch64-apple-ios` device slice, archives the
app for `generic/platform=iOS` (development-signed, automatic provisioning), then runs `xcodebuild
-exportArchive` with `method: app-store-connect` to produce a distribution-signed
`build/package/export/*.ipa`. Upload it with **Transporter** (Apple's app) or `xcrun altool
--upload-app -f <ipa> -t ios --apiKey <KEY_ID> --apiIssuer <ISSUER_UUID>` (set the two ids **once**
in `signing.local.sh` and the script prints that line filled in, see "Uploading a build" below).

The export signs manually, against **two** installed App Store profiles you create in the portal:
one for `eu.allodia.mailcal` and one for the Share Extension's own App ID,
`eu.allodia.mailcal.share`. Both App IDs claim `group.eu.allodia.mailcal`, so the group has to be
assigned to each **before** its profile is generated. `package.sh` resolves both before it builds
anything and names the one that is missing; `IOS_PROVISIONING_PROFILE` and
`IOS_SHARE_PROVISIONING_PROFILE` in `signing.local.sh` point at a download kept elsewhere.

### Flow D, iOS/iPadOS on your own device (`.ipa` you can install)

**Flow C's `.ipa` cannot be installed on a device, and no flag changes that.** An App Store
provisioning profile carries no device list, so iOS refuses to launch it however you copy it across.
`package.sh` asserts exactly that before upload, which is why a build that *could* be sideloaded
would have failed the gate.

`--ios-device` archives **the same Release configuration** and differs only in the export: `method:
development`, whose profile embeds the team's registered devices. The result lands in
`build/release-<VERSION>/` and installs by dragging it onto the device in **Xcode → Window → Devices
and Simulators**, or:

```sh
xcrun devicectl device install app --device <udid> "<the .ipa>"
```

The export is gated on the mirror of Flow C's assertion, that the embedded profile lists **some**
`ProvisionedDevices` rather than none. An `.ipa` that silently came out distribution-signed looks
identical right up to the moment it will not launch, on the device, after you have gone to install
it.

**What it is and is not.** Everything that is a property of the compiled code is what ships:
optimisation, timing, memory, the release core with no dev-harness. What differs is signing, and
with it the entitlements, `aps-environment` is `development`, so push tokens are **sandbox**
tokens. For real APNs delivery, or to check the exact bits that go to users, install the
**TestFlight** build instead; a device this team has registered can do both.

### Uploading a build, the two ids go in `signing.local.sh`, once

`xcrun altool --upload-app` wants `--apiKey` and `--apiIssuer` on every invocation. Rather than
fetching them from App Store Connect each release, set them in the git-ignored `signing.local.sh`:

```sh
ASC_API_KEY_ID="ABCDE12345"
ASC_API_ISSUER_ID="6053b7fe-68a3-47e6-a0d3-000000000000"
```

Both flows then print their upload line **ready to paste**, with the ids substituted; leave them out
and you get the `<KEY_ID>`/`<ISSUER_UUID>` placeholders exactly as before. A **half-filled** config:
one id set, or the template's `REPLACE_WITH_*` text left in place, deliberately falls back to the
placeholders too, so you never get a line that looks complete and fails against Apple talking about
credentials instead of about the line you just pasted.

**Neither id is a secret**, which is why they belong in that file under the same rule as the Team ID
and the certificate names. What authenticates is the private key, `AuthKey_<KEY_ID>.p8`, and altool
finds it **by key id** in the first of `./private_keys`, `~/private_keys`, `~/.private_keys`,
`~/.appstoreconnect/private_keys` that has it, so the `.p8` is never named in the config and never
goes in the repo. Apple lets you download it exactly once; keep the copy you have.

Both ids are on one page: App Store Connect ▸ **Users and Access** ▸ **Integrations** ▸ **App Store
Connect API**. "Issuer ID" sits above the key table (one per account); the Key ID is the key's own
row. These are the same credentials the `asc` CLI uses for the listing push, which is not built
here and belongs to whoever holds the App Store Connect account, but `asc` keeps its copy in the
system keychain and does not expose the issuer, so the two are configured separately.

**Why automatic signing here (and not Flow B's manual pass).** Flow B signs the whole app tree by
hand because macOS `-exportArchive` **skips nested SPM resource bundles** (ITMS-90284). That is a
**macOS-only** bug, iOS `-exportArchive` re-signs nested code correctly, so Flow C lets the export
do the Apple-Distribution signing against an auto-managed **iOS App Store** provisioning profile.
No persistent cert or profile to create by hand; `-allowProvisioningUpdates` fetches them.

**The stamped build number survives.** `ExportOptions-AppStore-iOS.plist` sets
`manageAppVersionAndBuildNumber = false`, so Xcode keeps the dotted-timestamp `CFBundleVersion`
`package.sh` stamped (the same `/VERSION` + dotted-`CFBundleVersion` scheme as macOS; see
[`docs/versioning.md`](../../docs/versioning.md)) instead of overriding it.

**Gates that assert before the remote gate.** A store rejection costs a build number and a slow
round-trip, so `package.sh` asserts what App Store delivery checks, locally:

- **Pre-archive** (right after `xcodegen`, before the multi-minute archive): unless the app opts out
  of iPad multitasking (`UIRequiresFullScreen = true`), `UISupportedInterfaceOrientations~ipad` must
  list **all four** orientations, or delivery rejects the build with **error 90474**. (This one bit a
  first upload, the generated plist listed three.)
- **Pre-upload** (after export): `package.sh` unzips the `.ipa` and checks the app's leaf signing
  authority is **Apple Distribution**, its `embedded.mobileprovision` is a **distribution** profile
  (no `ProvisionedDevices`, catches an accidental development/ad-hoc export), and `codesign --verify
  --deep --strict` passes.

Any failure aborts locally with a clear message.

**App icon.** iOS App Store validation requires an app icon; the shared `AppIcon.appiconset` now
carries a single-size **1024** `universal`/`ios` image (alpha-free, so Apple accepts it) alongside the
macOS images, `Scripts/generate-appicon.sh` emits it and `project.yml` wires it per SDK.

**Verified end-to-end (2026-07-20).** A Flow C `.ipa` was built, exported, passed the local gates,
and **uploaded + accepted by App Store Connect** (`altool --upload-app`, delivery UUID recorded).
(The first attempt surfaced error 90474, see the pre-archive gate above, which is now fixed and
asserted locally.)

**Remaining (not tooling).** Completing the iOS/iPadOS App Store Connect record + listing and
submitting the build for review, product steps in App Store Connect. Apple's app record spans
macOS/iOS/iPadOS, so the drafted copy, the **4+** age rating, and the App Privacy labels already live
in [`docs/store-listing.md`](../../docs/store-listing.md) and apply to iOS unchanged.

### Files

- `project.yml`, the XcodeGen manifest (targets, signing, Info.plist, Release production settings).
- `App/`, the multiplatform entry point (`AllodiaApp.swift`), the asset catalog with the generated
  `AppIcon` (macOS per-size images + a single-size 1024 iOS icon), and the entitlements:
  `AllodiaMail.entitlements` (iOS keychain + App Group), `*.macOS.entitlements` (Flow A),
  `*.appstore.entitlements` (Flow B sandbox), and the Share Extension's pair,
  `AllodiaMailShare.entitlements` (App Group: iOS and Flow B) and
  `AllodiaMailShare.macOS.entitlements` (a home-relative grant: every other macOS build, and it
  carries the measurement that forced the split).
- `ShareExtension/`, the Share Extension: what puts the app in the OS's own share sheet
  ([`docs/os-integration.md`](../../docs/os-integration.md)). It draws nothing and links no core.
- `Packages/MailcalKit/`, the shared Swift package (`MailcalUI` + `MailcalShareBox`, the drop box a
  share is handed over in, + the generated `MailcalBindings`).
- `Scripts/`
  - `build-and-run.sh`, the debug dev loop.
  - `build-core.sh [--release] [--no-device] [--slices device,sim,macos]`, cross-compiles the Rust
    core and assembles `Mailcal.xcframework` + the generated Swift bindings. `--slices` narrows it
    to the platforms a build actually links; CI has a job per platform and passes one name.
  - `package.sh`, the three production flows above (Developer-ID `.dmg`, macOS Store `.pkg`, iOS
    Store `.ipa`).
  - `generate-appicon.sh`, regenerates `App/Assets.xcassets/AppIcon.appiconset` (macOS + the iOS
    1024 icon) from the brand source icon ([`docs/branding.md`](../../docs/branding.md); every
    client's generator is run at once by `scripts/dev/brand-icons.sh`). A straight downscale, the
    macOS "squircle" shape/padding is a design follow-up.
  - `ExportOptions-*.plist`, `-exportArchive` templates: `ExportOptions-DeveloperID.plist` (Flow A)
    and `ExportOptions-AppStore-iOS.plist` (Flow C). `__TEAM_ID__` substituted at run time.
- `signing.local.sh.example`, template for the git-ignored local signing config.
- `signing.xcconfig`, the build's half of the same thing: it optionally includes the git-ignored
  `signing.local.xcconfig`, which carries the team a physical-device build signs with.
