# Branding: what the app is called, and what the OS knows it by

Two values decide the app's identity, and neither is written in a client:

| Variable | What it is |
|---|---|
| `MAILCAL_APP_NAME` | The name a person reads: the launcher, the window, the installer, the About screen. Not localised: one string in all seven languages. |
| `MAILCAL_APP_ID` | The reverse-DNS identifier the operating system knows the app by, and the directory it gives the app for its data. Also a URI scheme, so it is **all lowercase**: Android matches an intent filter's scheme case-sensitively and never matches an uppercase one. |

They are resolved, in this order:

1. **The real environment**, so a one-off build can be re-branded without editing anything.
2. **`branding/allodia.env`**, when the checkout has it.
3. **`branding/default.env`**, which every checkout has.

The presence of the second file is the whole switch. Removing it un-brands every build in one
step: no flag, nothing to turn off, which is how the open repository gets an unbranded default:
[`pledge.md`](pledge.md), "a fork is rebranded by omission".

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

**The id** is taken from the most authoritative source each platform offers at the point of use.

| | Name | Application id | Store / package identity |
|---|---|---|---|
| **Android** | `android:label` from a manifest placeholder; catalog elsewhere | `applicationId` from `brandValue`; code reads `BuildConfig.APPLICATION_ID` | n/a |
| **Apple** | `CFBundleDisplayName` / `CFBundleName` from `${MAILCAL_APP_NAME}` in `project.yml` | `PRODUCT_BUNDLE_IDENTIFIER` from `${MAILCAL_APP_ID}`; entitlements derive the keychain group and app group from it; code reads the generated `Brand.appID` | n/a |
| **Windows** | `Package.appxmanifest`, rewritten by `package.ps1` | code reads the generated `Brand.AppId` | `MAILCAL_MSIX_IDENTITY_NAME` / `_PUBLISHER` / `_PUBLISHER_DISPLAY_NAME`: a Partner Center reservation, which does **not** follow the app id |
| **Linux** | catalog only | the generated `l10n::APP_ID`: the GTK application id, the libsecret schema, `~/.var/app/<id>` | Flatpak manifest, rewritten by `package.sh` |

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

## The store copy

A third slot, and the same resolution rule: `branding/allodia-listing.md` when it is present,
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
| **Launcher icon** | `branding/{allodia,default}-icon.png`, `MAILCAL_ICON_SOURCE` over both | `scripts/dev/brand-icons.sh` | all four clients |
| **Welcome illustration** | `branding/{allodia,default}-welcome.png`, `MAILCAL_WELCOME_SOURCE` over both | `scripts/dev/brand-welcome.sh` | Android, Apple, Windows; Linux draws none |

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
| The source | `branding/allodia-icon.png` if the checkout has it, `branding/default-icon.png` otherwise. `MAILCAL_ICON_SOURCE` overrides both. Read it with `brand_icon_source` (shell) or `brand.icon_source()` (Python). |
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

1. **A client never writes the name or the id.** Both are injected; `scripts/ci/check-branding.sh`
   asserts each build config still derives rather than states, and is in `gate.sh` and CI.
2. **Anything named after the id follows the id.** The OAuth redirect schemes, the keychain service
   and access group, the app group, the credential-target prefix, the MCP pipe name, the
   background-refresh task id, the notification-portal attribution. A literal is a value that stops
   matching the moment the app is re-branded, and every one of those failures is silent.
3. **`branding/allodia.env` is the only file that carries Allodia's identity.** It is not secret;
   it is the public identity of a shipped app, so it is committed, and it is the one file a
   publisher swaps to make a build their own.
4. **⚠️ Its values are reservations held by third parties.** `MAILCAL_APP_ID` is the App ID Apple
   has on file, the redirect URIs registered with Azure and Google, and the directory every
   existing installation keeps its mail in; the MSIX trio is what Partner Center matches an upload
   against. Nothing there is edited without the corresponding console change.
5. **Source identifiers do not follow the brand.** The Kotlin package `eu.allodia.mailcal`, the C#
   namespace `Allodia.Mailcal`, the Apple target and executable `AllodiaMail`: no OS and no user
   ever sees them, and moving them would rewrite every client for a string nothing reads.

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
- **The MCP relay is called `allodia-mcp` in every build.** It is a cargo `[[bin]]` name, which
  cannot be injected, but each client installs it under a name its own build config decides
  (`Link=` on Windows, the nested bundle on macOS), so that is where a neutral default belongs.
  Renaming it changes the command users have already configured, so it is not a silent change.
- **Publisher metadata still names Allodia in an unbranded build**: the AppStream `<developer>`
  block and the four `allodia.eu` URLs in `clients/linux/flatpak/metainfo.xml.in`, and the
  Credential Manager's own labels. These come from `docs/store-listing.md`'s family of documents,
  which travel with the brand rather than with this tree.
- **One catalog string still names the company**, so an unbranded build shows it: the MCP
  description's "nothing is sent to Allodia". It is not the *product's* name, which is what
  `{app_name}` covers: it is a claim about who runs the service, and it is rewritten with the rest
  of the prose.
- **Prose is untouched.** Documentation, comments and store copy still name the product. That is
  the split's content pass, not this contract, and a check here that grepped for the word would
  fail on every file that correctly explains what Allodia's build is.
