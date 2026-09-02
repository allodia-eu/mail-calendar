# Store screenshots: the submission set, and where it lives

One PNG per (platform, locale, screen), for whoever publishes this app to a store.

**They are no longer committed.** They live in the gitignored `showcase-screenshots/<platform>/` at
the repo root, exactly where `scripts/dev/showcase.sh` writes them, and each store's publisher
uploads them from there. The set became seven locales × seven captures in August 2026 (49 PNGs and
~25 MB for macOS alone, ~100 MB across five platforms), and every one of them is **reproducible from
a script and a seeded dataset**, so git was storing a large binary artefact it could regenerate on
demand and that the store itself holds a copy of. What a reviewer actually needs is the *recipe*
(`showcase.sh`, the showcase seeds, the gallery order in `store_payload.SCREEN_ORDER`), and that
is all in git. This page is that recipe.

The cost of the change is honest: a capture is now only as reproducible as the client that took it,
so a screenshot from an older release cannot be diffed against a new one: re-capture instead
(`scripts/dev/showcase.sh <platform>`), which takes minutes and is scripted for exactly that reason.

Every PNG is captured from the seeded, in-memory **showcase** dataset (`MAILCAL_SHOWCASE`):
two fictional accounts, sample mail, and a full work-and-private-life calendar, with **no real
mail and no network**, so a store asset can never leak a personal mailbox. The one asset that isn't
a capture, Google Play's **feature graphic**, is composed *from* those captures, so it inherits the
same guarantee.

## Layout

`showcase-screenshots/<platform>/<locale>-<screen>.png`: e.g. `macos/en-calendar.png`,
`iphone/nl-reply.png`. The layout is unchanged by the move out of git, and both publishers read it:
`--screenshots DIR` takes any directory shaped this way.

- **Platforms:** `macos`, `iphone`, `ipad`, `android`, `windows`.
- **Locales:** `en`, `nl`, `de`, `fr`, `es`, `it`, `pt`: exactly the app's localisation catalog
  ([`project.inlang/settings.json`](../../project.inlang/settings.json)); each store listing
  language needs its own screenshots.
- **Screens (7):** `list`, `list-dark`, `reply`, `settings`, `add-account`, `calendar`,
  `invitation`. The order they appear in a store gallery is a separate decision, and one that is
  written down: `SCREEN_ORDER` in
  [`scripts/dev/store_payload.py`](../../scripts/dev/store_payload.py), which every publisher
  applies (alphabetical would open every listing on `add-account`).

**`invitation`** is the one screen that shows mail and calendar working as one product: a meeting
request open in the reading view, with Accept / Maybe / Decline over a preview of the day it would
land on ([`docs/invitations.md`](../../docs/invitations.md)).

**`list-dark` is not a seventh screen: it is the sixth one again, in the other appearance.** Every
store capture is pinned to a light/dark appearance (`MAILCAL_APPEARANCE`, `docs/debugging.md`), and
the mailbox list is captured twice: `<locale>-list.png` light and `<locale>-list-dark.png` dark. No
client learns a new `MAILCAL_SHOWCASE_SCREEN` word for it (the theme is a launch environment, not a
destination), which is why `showcase.sh --screen list` writes two files.

Pinning the *light* ones matters as much as adding the dark one. Unstated, the appearance of a
capture is whatever the capturing machine happens to be set to, and nothing downstream can tell: a
dark screenshot of the right screen in the right language passes the byte floor, the pixel-size
assertion and the showcase-launch proof alike, and reaches the store looking like a different
product from the one the other six locales show.

One dark capture, not a dark set, and it is the mailbox list because that is the screen a listing
opens on: the pair reads as the same inbox in either theme rather than as two unrelated screens.
Google Play takes at most **8** images per slot (`MAX_PER_SLOT` in
[`scripts/dev/play_listing.py`](../../scripts/dev/play_listing.py)), so a second dark capture would
put the set one under the ceiling for every locale at once.

⚠️ **On the three-pane clients the dark capture carries a white panel, and that is the product, not
the capture.** macOS, Windows, Linux and iPad show the reading pane on this screen, and a message's
own body is HTML in a web view with no dark palette: `docs/settings.md` → Known gaps. So about a
third of a desktop `list-dark` frame is white. Phones are unaffected: no
reading pane, so the frame is dark throughout. Do not "fix" it by re-shooting, switching the dark
screen to `calendar`, or editing the PNG: the first two hide a shipped gap and the third puts a
hand-made asset in a set whose whole point is that a script produces it.

Windows is the one platform still at the old set (`en`/`nl`, five screens): it builds only on a
Windows host, so it is re-captured there rather than in the same pass as the other four.

**Android is one flat directory, and the form factor is in the file name:**
`android/<form-factor>-<locale>-<screen>.png`: e.g. `android/tablet-10-nl-list.png`. Google Play
keeps a **separate screenshot slot per form factor** (phone, 7-inch tablet, 10-inch tablet), and an
empty tablet slot is what makes Play treat an app as phone-only on large screens, so the set really
is three device sizes of one client. They live in one directory rather than three because **the Play
Console's picker is a flat file list**: uploading from three identically-named sets is how a tablet
shot ends up in the phone slot. The prefix (`phone`, `tablet-7`, `tablet-10`) is what makes each
file self-describing at the point it is picked.

## Store-valid sizes

| Platform | Pixels | Store slot |
|---|---|---|
| `macos` | 2880×1800 | App Store Connect: Mac |
| `iphone` | 1320×2868 | App Store Connect: iPhone 6.9" |
| `ipad` | 2064×2752 | App Store Connect: iPad 13" |
| `android/phone-*` | 1080×2400 | Google Play: phone (emulator-native) |
| `android/tablet-7-*` | 1200×1920 | Google Play: 7-inch tablet |
| `android/tablet-10-*` | 1600×2560 | Google Play: 10-inch tablet |
| `android/feature-graphic-*` | 1024×500 | Google Play: feature graphic (see below) |
| `windows` | 2880×1800 | Microsoft Store |

The Windows client pins **1440×900 logical** rather than a pixel size, so its physical frame follows
the capturing monitor's scale: 2880×1800 at 200% (what this set is shot at), 1440×900 at 100%. Both
sit inside the Store's 1366×768…3840×2160 bounds, so the Store would take either and the mismatch
would show up only as one language looking smaller than the rest. `showcase.sh` **asserts** the size
above instead: shoot on a 200% display, or the run fails rather than files the odd one out.

That assertion also holds a screen's two appearances to one shape. The window's top border row is a
row the app does not paint and `PrintWindow` does not render: it comes back pure white under a light
theme and pure black under a dark one, never the grey it is on screen. It is cropped in both themes
(`screenshot-frame.ps1`) and the window carries one extra row to pay for it: recognise that row as
*black* alone and the crop turns theme-dependent, taking it off the dark capture only and filing a
frame a pixel shorter than its light twin, inside every bound the Store checks.

The two tablet sizes are Play's recommended portrait exports, and `showcase.sh` **asserts** each
capture's pixel size against them: the stock tablet AVDs boot landscape-native, so a rotation that
silently didn't take would otherwise file a 1920×1200 landscape frame under a portrait slot.
Portrait is also the shape the rest of this set is in, and the one a single-pane layout reads best
in: the 10-inch list shows 10 rows portrait against 6 landscape.

## The Google Play feature graphic

`android/feature-graphic-<locale>.png`, one per catalog locale: **1024×500, 24-bit PNG, no alpha**.
Play will not publish a listing without one. It is not a screenshot, since no device can be asked
for it, so it is composed from the captured phone screens plus a palette and a wordmark. The
generator draws a wordmark, so it belongs to whoever owns the brand and lives with them rather than
here.

## Regenerating

`scripts/dev/showcase.sh` relaunches the app once per (locale, screen) with the showcase flags and
shoots each screen, into `showcase-screenshots/<platform>/`. That directory **is** the set now: no
copy step:

```sh
scripts/dev/showcase.sh macos                       # 49 PNGs: 7 captures × 7 locales
scripts/dev/showcase.sh iphone
scripts/dev/showcase.sh ipad
scripts/dev/showcase.sh android --serial <emulator> # emulator only (demo-mode status bar)
scripts/dev/showcase.sh android-tablet-7            # boots the Small_Tablet AVD itself
scripts/dev/showcase.sh android-tablet-10           # boots the Pixel_Tablet AVD itself
scripts/dev/showcase.sh windows                     # on a Windows host
```

Then push, from the same directory:

```sh
scripts/dev/appstore_listing.py --apply  --screenshots showcase-screenshots/macos
scripts/dev/publish_play.py     --commit --screenshots showcase-screenshots/android
```

The Microsoft Store push is not in this repository; it belongs to whoever holds the Partner Center
account, and it is pointed at `showcase-screenshots/windows/` in a checkout of this one.

The Play push reads all four slots out of the one Android directory (`phone-`, `tablet-7-`,
`tablet-10-` and the feature graphic) and **replaces each slot**, because Play appends to a gallery
rather than overwriting it. Run it without `--commit` first: that uploads everything, asks Play to
validate it, and deletes the edit.

The three Android targets write into one `showcase-screenshots/android/` with the form-factor prefix
already applied (`phone-en-list.png`, `tablet-10-nl-calendar.png`), which is the layout Play's flat
picker needs: no rename step.

macOS / iPhone / iPad run on a Mac; **Android needs an emulator** (SystemUI demo mode, which pins
the status bar to a clean 09:41, only works on emulators, not a physical phone); **Windows** runs
on a Windows host. The tablet targets boot their AVD if it isn't already running and shut it down
again afterwards, so each is a single command. See [`docs/debugging.md`](../../docs/debugging.md) →
the showcase section.

If the phone screenshots change, **regenerate the feature graphic too**: it is composed from them.

## The two ways a run photographs the wrong thing

Neither is visible in the result. A capture of the wrong build, or of the wrong device, is a clean,
correctly-sized, showcase-mode PNG of the right screen in the right language: it clears the byte
floor, the pixel-size assertion and the showcase-launch proof, and it looks right. So `showcase.sh`
now says which device and which build it is about to shoot, before the first shutter:

```
==> shooting iphone on iPhone 17 Pro Max (85F2F484-…)
==> app built 2026-08-20 19:04:16
```

- **The device.** `boot.sh <platform>` and `showcase.sh <platform>` resolve **different** defaults:
  boot.sh takes whichever simulator is booted, this one takes the store-sized `default_simulator`.
  Drive one, then run the other, and you are on a device you never looked at. Pass `--simulator` to
  say which, or read the line above.
- **The build.** `--no-build` shoots whatever is installed, which can predate the change being
  photographed by whole branches. It is now **refused** when the installed build is older than the
  sources behind it, or when there is none on the target device (macOS, iPhone, iPad and Linux,
  where the binary is a path this script can stat; Android's APK lives on the device under its own
  clock, and Windows' exe is found by `showcase.ps1`'s own search).

A **generated** file named in that refusal (an `Info.plist`, a bindings or L10n source) is a real
answer rather than a false alarm: it was rewritten because its inputs moved, and the build predates
that. Re-running without `--no-build` is always the fix.
