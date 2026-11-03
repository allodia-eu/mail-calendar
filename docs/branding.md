# Branding: what the app is called, and what the OS knows it by

Two values decide the app's identity, and neither is written in a client:

| Variable | What it is |
|---|---|
| `MAILCAL_APP_NAME` | The name a person reads: the launcher, the window, the installer, the About screen. Not localised: one string in all seven languages. |
| `MAILCAL_APP_ID` | The reverse-DNS identifier the operating system knows the app by, and the directory it gives the app for its data. Also a URI scheme, so it is **all lowercase**: Android matches an intent filter's scheme case-sensitively and never matches an uppercase one. |

They are resolved, in this order:

1. **The real environment**, so a one-off build can be re-branded without editing anything.
2. **`branding/brand.env`**, the publisher's identity, when the checkout has it.
3. **`branding/default.env`**, which every checkout has.

The presence of the second file is the whole switch. Removing it un-brands every build in one
step: no flag, nothing to turn off, which is how the open repository gets an unbranded default:
[`pledge.md`](pledge.md), "a fork is rebranded by omission".

`branding/allodia.env` is the same slot under its older name and is still read, after `brand.env`,
so a release pipeline that copies that name in goes on building branded. Every file slot below
resolves `allodia-*` after `brand-*` the same way.

There is no absent case to handle. `branding/default.env` is committed and names every key, so a
build always has an identity; a checkout that has lost it is incomplete, and the readers say so.

## Reading it

| Language | How |
|---|---|
| Shell | `. scripts/dev/brand.sh` then `brand_load` (exports) or `brand_value KEY`. |
| Python | `from brand import resolve, value` (or `python3 scripts/dev/brand.py --json`, which is how PowerShell reaches it without a fourth copy of the parser). `icon_source()` and `listing_source()` answer the two *file* slots below. |
| Gradle | `brandValue("…")` in `clients/android/app/build.gradle.kts`. |
| Rust | `mailcal-l10n` resolves it at codegen time; nothing else in the workspace needs it. |

The parsers are deliberately not shells. The product name contains an ampersand, and a `source`
of these files is a thing that can be made to run what is written in them.

## How each client gets it

**The name** reaches all four clients one way: the catalog writes `{app_name}` wherever the product
names itself, and `mailcal-l10n` substitutes it at codegen time. Every call site is unchanged and
takes no argument: the name is a build-time constant, not something a caller passes, and a
per-locale copy of a constant could only drift.

**The `User-Agent`** every HTTP request carries, through the engine's adapters and the core's own
clients alike, is the name as one lowercase token with the version from [`VERSION`](../VERSION):
`allodia-mail-calendar/0.13.0`, or `mailcal/0.13.0` unbranded. `mailcal-user-agent` derives it at
build time, so it follows the name with nothing else to keep in step.

**The id** is taken from the most authoritative source each platform offers at the point of use.

| | Name | Application id | Store / package identity |
|---|---|---|---|
| **Android** | `android:label` from a manifest placeholder; catalog elsewhere | `applicationId` from `brandValue`; code reads `BuildConfig.APPLICATION_ID` | n/a |
| **Apple** | `CFBundleDisplayName` / `CFBundleName` from `${MAILCAL_APP_NAME}` in `project.yml` | `PRODUCT_BUNDLE_IDENTIFIER` from `${MAILCAL_APP_ID}`; entitlements derive the keychain group and app group from it; code reads the generated `Brand.appID` | n/a |
| **Windows** | `Package.appxmanifest`, rewritten by `package.ps1` | code reads the generated `Brand.AppId` | `MAILCAL_MSIX_IDENTITY_NAME` / `_PUBLISHER` / `_PUBLISHER_DISPLAY_NAME`: a Partner Center reservation, which does **not** follow the app id. The download we host ourselves is a second package with its own `MAILCAL_MSIX_DIRECT_IDENTITY_NAME` / `_DIRECT_PUBLISHER` ([`windows-channels.md`](windows-channels.md)) |
| **Linux** | catalog only | the generated `l10n::APP_ID`: the GTK application id, the libsecret schema, `~/.var/app/<id>` | Flatpak manifest, rewritten by `package.sh`; the snap recipe, filled by `package-snap.sh`, under `MAILCAL_SNAP_NAME`: a Snap Store reservation, which does **not** follow the app id |

Three notes on the asymmetry, each deliberate:

- **Android and Apple could ask their platform** (`BuildConfig`, `Bundle.main`) and Android does.
  Apple does not, because `MailcalKit` is compiled and tested by SwiftPM outside any app bundle,
  where `Bundle.main` is the test runner: the app group and the keychain service would then differ
  between `swift test` and the shipped app.
- **Windows and Linux cannot ask.** A Linux binary is not a bundle, and the Windows dev loop runs
  unpackaged, where `Package.Current` throws. Both take a constant emitted by `mailcal-l10n`, which
  is already the one build step every client runs.
- **The MSIX and Flatpak manifests are committed unbranded and rewritten by their packaging
  scripts**, which restore the committed bytes afterwards. Both are files a store or a desktop
  shell reads directly, and neither format can reference a variable.

## The publisher's addresses

Where the app sends someone, it sends them to the publisher, so each address is a brand value
beside the name and resolves by the same rule. `branding/default.env` points each at this
repository, the one thing an unbranded build can honestly send anyone to.

| Variable | Where a person meets it | How it gets there |
|---|---|---|
| `MAILCAL_PRIVACY_URL` | The privacy policy link on the welcome screen | The catalog writes `{privacy_url}`, substituted by `mailcal-l10n` like `{app_name}` |
| `MAILCAL_SUPPORT_URL` | Settings → About | `mailcal-bindings`' build script injects it into `about_info()` |
| `MAILCAL_WEBSITE_URL` | An MCP client's server list, as the server's `websiteUrl`; the Linux software centre's home page link | `mailcal-mcp`'s build script; `flatpak_metadata.py` |
| `MAILCAL_HELP_URL` · `MAILCAL_CONTACT_URL` | The Linux software centre's help and contact links | `flatpak_metadata.py` |

The Linux software centre also names the publisher, `MAILCAL_PUBLISHER_ID` and
`MAILCAL_PUBLISHER_NAME` in the AppStream `<developer>` block, and tints the area behind the icon
with `MAILCAL_PRIMARY_COLOR_LIGHT` and `MAILCAL_PRIMARY_COLOR_DARK`. The bug tracker it links stays
this repository's in every build: it is where the code is fixed.

The local MCP server introduces itself by the brand too ([`mcp.md`](mcp.md)): its `title` is
`MAILCAL_APP_NAME`, and its machine name is `MAILCAL_MCP_SERVER_NAME`, which `mailcal-mcp` compiles
in and `mailcal-l10n` emits beside the app id (`Brand.mcpServerName`, `Brand.McpServerName`,
`l10n::MCP_SERVER_NAME`) for the config key each desktop client writes. A user types that name
into their own configuration, so like the app id it does not change once shipped. The client name a
provider's consent screen shows when the app registers itself for OAuth is `MAILCAL_APP_NAME` as
well.

The relay that configuration spawns is installed as `MAILCAL_MCP_RELAY_NAME`, which is therefore
also a name users have typed, and frozen the same way. Cargo cannot take a binary's name from the
brand, so `crates/mailcal-mcp-shim` always builds `mailcal-mcp` and each client installs it under
the brand's name:

| | Where the brand's name goes | Set by |
|---|---|---|
| **Apple** | The nested `Library/Helpers/<name>.app` and the executable inside it | `MCP_RELAY_NAME` in `project.yml`; code reads `Brand.mcpRelayName` |
| **Windows** | The App Execution Alias `<name>.exe`; the file it launches stays `mailcal-mcp.exe` | `msix_manifest.py`; code reads `Brand.McpRelayName` |
| **Linux** | The Flatpak command and the snap app, so `/snap/bin/<snap>.<name>` | the Flatpak manifest's install step; `package-snap.sh`; code reads `l10n::MCP_RELAY_NAME` |

A development build that runs from the build directory reaches the relay as cargo left it, under
`mailcal-mcp`.

On macOS and iOS, `MAILCAL_APPLE_PRODUCT_NAME` names the `.app` and its executable: what Finder
lists in Applications, what Activity Monitor shows, and the file names `package.sh` gives what it
builds. It takes letters and digits only, because every script that signs, launches or stops the
app names that path. The Xcode target stays `AllodiaMail` (rule 5).

## Where the app keeps its data

The mail store, the diagnostic log, the credential index and the MCP endpoint live in a directory
of the app's own, and on two platforms that directory is not given to the app by the OS:

| | The directory | Brand value |
|---|---|---|
| **macOS** (outside the Mac App Store) | `~/.local/share/<name>`, a debug build `<name>-dev-*`, the share box `<name>-share` | `MAILCAL_DATA_DIR_NAME` |
| **Linux** (unpackaged) | `$XDG_DATA_HOME/<name>` | `MAILCAL_DATA_DIR_NAME` |
| **Windows** (packaged too: the app writes there unvirtualized) | `%LOCALAPPDATA%\<path>` | `MAILCAL_WINDOWS_DATA_DIR`, `/` between segments |

Two brands with the same value on one machine open one store, and each then finds the other's
accounts with no credentials to go with them. So a publisher states its own, and like the app id it
is a reservation of sorts: it is where every installed copy already keeps its mail. Allodia's are
`mailcal` and `Allodia/MailCalendar`. Everywhere else the directory is the app's already (the Mac
App Store and iOS containers, Android's `filesDir`, a Flatpak's or a snap's own data directory), and
the same name is used inside it. `mailcal-l10n` emits both (`Brand.dataDirName`,
`Brand.WindowsDataDir`, `l10n::DATA_DIR_NAME`); the macOS Share Extension, which links none of that,
reads it from its Info.plist and its sandbox grant names it through a build setting.

## The store copy

A third slot, and the same resolution rule: `branding/brand-listing.md` when it is present,
`branding/default-listing.md` when it is not, `MAILCAL_LISTING_SOURCE` over both. It is a whole file
rather than a set of values because copy is paragraphs, and `brand.listing_source()` is what every
reader goes through.

**It is not the contract.** [`store-listing.md`](store-listing.md) holds the rules both files obey:
what may be claimed, which locales move together, the stores' field limits, and how the Linux
metainfo is generated, and stays public whatever brand is on the build. What resolves here is only
the answer.

**The neutral default is deliberately thin.** English only, no per-store fields, and no capability
claim: the anti-hype rule measures copy against the capability matrix per platform, and an unbranded
build's reach is nobody's to certify. It carries the two fields the Linux metainfo cannot be
generated without, and the checker measures a per-store field only if the listing promises one.

⚠️ **The product name is not in either file.** It is `MAILCAL_APP_NAME` above, read by the store
publishers and the metainfo generator alike, because a software centre draws the name and the
description side by side and a name written twice is a name that can disagree with the launcher.

## The art

Art cannot be injected the way a name can: no client draws its art at build time, so what ships is
whatever was committed. The switch is therefore one step longer: **swap the source, re-run the
generators, commit what they wrote**, but it resolves by the same rule, so it takes the same one
file to flip.

There are two slots, each with its own source file and its own generator:

| Slot | Source | Cut by | Drawn by |
|---|---|---|---|
| **Launcher icon** | `branding/{brand,default}-icon.png`, `MAILCAL_ICON_SOURCE` over both | `scripts/dev/brand-icons.sh` | all four clients |
| **Welcome illustration** | `branding/{brand,default}-welcome.png`, `MAILCAL_WELCOME_SOURCE` over both | `scripts/dev/brand-welcome.sh` | Android, Apple, Windows; Linux draws none |

They are separate slots on purpose. The neutral welcome art is the neutral icon with a launcher's
corner radius applied, because a full-bleed alpha-free square is right for a thing every platform
masks and wrong in the middle of a screen that masks nothing, and keeping it a *file* rather than
an alias is what lets art drawn for the screen replace it without the launcher icon following.

The client-side resource is named for the slot (`welcome_art`, `WelcomeArt`, `welcome-art.png`) and
so is its accessibility label, which describes the slot rather than the picture: an unbranded build
must not announce a mascot that is not there.

### The launcher icon

| | What decides it |
|---|---|
| The source | `branding/brand-icon.png` if the checkout has it, `branding/default-icon.png` otherwise. `MAILCAL_ICON_SOURCE` overrides both. Read it with `brand_icon_source` (shell) or `brand.icon_source()` (Python). |
| Re-cutting every client | `scripts/dev/brand-icons.sh`, which runs each client's generator and **exits non-zero naming any it could not run on this host**: a rebrand that quietly skipped a platform ships that platform's old art. |
| The ones a given host cannot run | Two are host-bound: `clients/windows/Mailcal/Images/generate-assets.ps1` draws with `System.Drawing` (Windows-only) and `clients/apple/Scripts/generate-appicon.sh` cuts with `sips` (macOS-only). Android and Linux run anywhere ImageMagick does. |

A source icon is a **square, full-bleed, alpha-free PNG**, ≥1024px. Each of those is load-bearing:
every platform applies its own mask, so baked-in rounded corners show up as pale slivers underneath
one; and Apple rejects an iOS app icon that has an alpha channel at all.

Android is the one that is not a downscale. An adaptive icon is three layers with three rules, and
`clients/android/generate-icons.sh` writes what it can and deliberately omits what it cannot:

- **background**: a flat colour, sampled from the source's four corners rather than written down
  beside it, so new art cannot leave a stale colour behind. Only ever seen in the sliver a
  launcher's parallax uncovers.
- **foreground**: the art at 84dp on the 108dp canvas. The mask shows the inner 72dp, so art
  scaled to exactly that covers the viewport only up to its own antialiased edge, and a launcher
  walking the layers apart draws a seam across the icon.
- **monochrome**: *not written*. See "Known gaps".

`LauncherIconTest` asserts all three from the built resources.

It writes **no raster mipmap**: not `mipmap-<density>/<icon>.webp` and not the circle-cropped
`_round` twin. Those are the API 25 and below fallback, `minSdk` is 31, and `aapt2 dump badging`
resolves the manifest's icon to the adaptive XML at every density bucket with them gone. The round
one was dead twice over: a round icon is read only through `android:roundIcon`, which the manifest
does not declare.

## Rules

1. **A client never writes the name or the id.** Both are injected; `cargo xtask check-branding`
   asserts each build config still derives rather than states, and is in `cargo xtask gate` and CI.
2. **Anything named after the id follows the id.** The OAuth redirect schemes, the keychain service
   and access group, the app group, the credential-target prefix, the MCP pipe name, the
   background-refresh task id, the notification-portal attribution. A literal is a value that stops
   matching the moment the app is re-branded, and every one of those failures is silent. The data
   directory is the one exception: it is a brand value of its own (above), because Allodia's
   predates the id it would otherwise follow.
3. **`branding/brand.env` is the only file that carries a publisher's identity.** It is not secret;
   it is the public identity of a shipped app, so it is committed, and it is the one file a
   publisher swaps to make a build their own.
4. **⚠️ Its values are reservations held by third parties.** `MAILCAL_APP_ID` is the App ID Apple
   has on file, the redirect URIs registered with Azure and Google, and the directory every
   existing installation keeps its mail in; the MSIX trio is what Partner Center matches an upload
   against, and `MAILCAL_MSIX_DIRECT_PUBLISHER` is the subject of the certificate that signs a
   package we host, which Windows compares character for character before it will install one.
   `MAILCAL_SNAP_NAME` is the name registered in the Snap Store, which can never be renamed.
   Nothing there is edited without the corresponding console change.
5. **Source identifiers do not follow the brand.** The Kotlin package `eu.allodia.mailcal`, the C#
   namespace `Allodia.Mailcal`, the Apple target and scheme `AllodiaMail` and its entitlement
   files, the cargo binary `mailcal-mcp`: no OS and no user ever sees them, and moving them would
   rewrite every client for a string nothing reads. A name a person does see, the `.app` in Finder
   or the command an assistant spawns, is a brand value instead.

## Known gaps

- **The neutral welcome art is the neutral icon, not an illustration.** It stands in until one is
  drawn for the screen; replacing it is `branding/default-welcome.png` and a re-run.
- **No themed icon on Android.** Android 13+ tints the `<monochrome>` layer's alpha with one colour
  from the wallpaper palette, which needs a silhouette: a mark, drawn as a mark. There is no way
  to derive one from full-bleed art: flood-filling the background out of a gradient keeps whichever
  side of it the fill could not reach, and what the launcher then paints is a slab. Until a
  silhouette is drawn, the generator writes no layer and the launcher falls back to the icon in
  colour, which is the better of the two failures.
- **No single host re-cuts all four.** `generate-assets.ps1` needs `System.Drawing` and
  `generate-appicon.sh` needs `sips`, so `brand-icons.sh` reports one of them as skipped wherever
  it runs: a rebrand is a Mac pass **and** a Windows pass, and the tree is inconsistent between
  them. It exits non-zero naming what it left behind, so the second pass cannot be forgotten
  quietly, but nothing checks that it happened.
- **Prose is untouched.** Documentation, comments and store copy still name the product. That is
  the split's content pass, not this contract, and a check here that grepped for the word would
  fail on every file that correctly explains what Allodia's build is.
