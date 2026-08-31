# Linux client

This is the GTK4/libadwaita three-pane mail client for Allodia Mail & Calendar. It links
`mailcal-bindings` directly, keeps mailbox and reading state in the shared Rust core, and uses
Relm4 to marshal observer notifications onto the GLib main loop and render snapshots. A conversation
in the message list is an `AdwExpanderRow` that opens in place into every message on the thread and
reads its latest in the pane; unread mail is bold, per row, in the list's own stylesheet. The three
app-specific glyphs there; a conversation, read and unread mail; are **ours**, drawn by
[`icons/generate-symbolics.py`](icons/generate-symbolics.py) and compiled from [`icons/`](icons)
into a GResource by [`build.rs`](build.rs), then registered on the icon theme, so they say the same
thing on every distribution; a theme can only replace a name it ships, and none ships `mailcal-*`.
Chrome that is universal, such as the attachment paperclip and flagged star, stays themed. WebKitGTK
hosts the locked-down reading document and a separate, fresh instance of the shared rich editor.
Each message row exposes read/unread, flag, archive, Trash, spam and confirmed permanent-delete
actions. The reading toolbar archives or moves to Trash and advances to the next visible message;
conversation rows archive the whole conversation while leaving Sent copies in Sent.
The calendar adds an agenda, a composed 6×7 month, and a drawn Cairo time grid whose semantic
AT-SPI event buttons use the same unit-free core geometry as its painted blocks. Its header exposes
day, 3-day, work-week, week, month, and agenda modes plus `< Today >` navigation. Event detail,
create, edit, and delete dispatch the shared provider-neutral intents.

It packages as a Flatpak (see "Package as a Flatpak" below) and has showcase mode, so the
store/marketing captures are produced the same way as every other client's.

It is not a shippable client yet. It has email-first setup and autodetection, Google Desktop
OAuth, and standards-discovered JMAP OAuth (RFC 9728 → 8414 → 7591 → PKCE) through the system
browser and bounded `127.0.0.1` loopback callbacks. JMAP sign-in appears only after the core's
pre-flight confirms the server advertises dynamic registration; a failure restores the detected
password/API-token form. Microsoft permission gaps and expired OAuth sign-ins reconnect in place;
an abandoned browser attempt can be superseded immediately. An `oo7` Secret Service store has no
plaintext fallback and persists, replaces and removes account credentials plus refresh-token
rotation. It carries the shared Settings taxonomy; every
category ([`docs/settings.md`](../../docs/settings.md)): Diagnostics, first-run analytics
consent, and local new-mail notifications from the live sync runtime. Calendar management,
default-calendar selection, periodic refresh, device-zone recovery, and packaged-runtime performance
qualification are wired and semantically driven. Horizontal calendar gestures, event move/resize,
and the rows still marked ⬜ in the README matrix remain. Normal/release startup loads stored accounts
from the system keyring and opens required setup when none exist; deterministic data remains
available through debug-only demo and local Stalwart fixtures.

## Prerequisites

The supported development baseline is Ubuntu 26.04 LTS. Install the complete native build and
debugging bundle once:

```sh
sudo apt update
sudo apt install --yes \
  git curl ca-certificates build-essential pkg-config \
  libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev \
  gnome-screenshot imagemagick ffmpeg \
  wmctrl xdotool x11-utils x11-apps \
  at-spi2-core accerciser python3-pyatspi dbus-x11 xvfb xauth
```

Then install the GNOME runtime the Flatpak ships on, once; a ~2 GB download, deliberately not
performed by any script:

```sh
sudo apt install --yes flatpak flatpak-builder
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub \
  org.gnome.Platform//50 org.gnome.Sdk//50 org.freedesktop.Sdk.Extension.rust-stable//25.08
```

**That runtime: not the `-dev` packages above: is what the client is verified against.** The
baseline tracks the same GNOME generation the Flatpak links (26.04 ships GNOME 50), so the two are
close; they are still two separate builds of GTK on two schedules, and only the runtime is pinned.
`clients/linux/build-and-run.sh` and `scripts/dev/test-linux-ui.sh` use it by default;
`build-and-run.sh --host` is the faster inner loop against the distribution's GTK, for work that has
nothing to do with the toolkit. The `-dev` packages are what `--host` needs; the driving tools below
run outside the sandbox either way.

The **compile-time floor is the crate feature gates** (GTK 4.14 / libadwaita 1.5 in
[`Cargo.toml`](Cargo.toml)), not the baseline: it is what the code may call, and it is deliberately
older than either. Raising it is a separate decision from moving the baseline, and it is what would
put `AdwSidebar` and friends within reach.

| Packages | Purpose |
|---|---|
| `git`, `curl`, `ca-certificates` | Clone with submodules and install/update Rust through `rustup` |
| `build-essential`, `pkg-config` | Native linker/build tools and GTK library discovery |
| `libgtk-4-dev`, `libadwaita-1-dev` | GTK 4.14+/libadwaita 1.5+ client build |
| `libwebkitgtk-6.0-dev` | Locked reading/composer content islands and native network content filters |
| `gnome-screenshot`, `imagemagick`, `ffmpeg` | Compositor-aware still capture, image inspection/conversion, and recordings/frame extraction |
| `wmctrl`, `xdotool` | Find, focus, size, and drive X11/XWayland windows deterministically |
| `x11-utils`, `x11-apps` | `xprop`, `xwininfo`, and `xwd` for low-level X11 diagnosis |
| `at-spi2-core`, `accerciser`, `python3-pyatspi`, `dbus-x11` | Inspect and script GTK's accessibility tree on a private D-Bus session |
| `xvfb`, `xauth` | Headless X server and authorisation helper for UI acceptance tests |

Install Docker Engine or Docker Desktop separately for the Stalwart harness, and install Rust with
`rustup`; the repository's `rust-toolchain.toml` then selects the exact compiler. The Linux build
script checks the native packages while compiling; GTK, libadwaita, and WebKitGTK are linked into
the binary. A production account also needs the desktop's normal Secret Service provider
(such as GNOME Keyring); notifications use the desktop portal, including inside a future Flatpak.

## Run against the local harness

```sh
scripts/dev/harness.sh up
scripts/dev/boot.sh linux                    # JMAP fixture
scripts/dev/boot.sh linux --account stalwart-imap
scripts/dev/boot.sh linux --account demo     # in-memory sample mailbox
```

To open the calendar directly in a debug build, choose one of `day`, `three-day`, `work-week`,
`week`, `month`, or `agenda`:

```sh
MAILCAL_CALENDAR=1 MAILCAL_CALENDAR_VIEW=agenda scripts/dev/boot.sh linux
```

These launch hooks are compiled out of release builds. The calendar always treats
`is_materialized: false` as a loading period, never as an empty one.

The harness stores are isolated under `$XDG_DATA_HOME/mailcal/dev` and `dev-imap` (falling back to
`~/.local/share`). `MAILCAL_DEV_ACCOUNT` and the harness TLS override are compiled out of release
builds. `--account personal` uses the production Secret Service-backed account store and opens the
first-run consent and setup flow when it is empty.

## Read diagnostics

```sh
scripts/dev/logs.sh linux
scripts/dev/logs.sh linux --dump
```

The log is `$XDG_DATA_HOME/mailcal/mailcal.log` (standard fallback: `~/.local/share`) with three
1 MB backups. The sink meets the rotation, best-effort, thread-safety, and privacy contract;
Settings → Diagnostics provides the current-log viewer, privacy-confirmed export, total size,
copyable path, and persisted DEBUG opt-in.

## Capture and control the window

The shared adapters find the live GTK window and its AT-SPI tree:

```sh
scripts/dev/screenshot.sh linux /tmp/mailcal-linux.png
scripts/dev/control.sh linux ui-dump
scripts/dev/control.sh linux find "Reply"
scripts/dev/control.sh linux activate "Reply"
scripts/dev/control.sh linux set-text "Title" "Team planning"
```

Controls use exact accessible names and GTK's semantic `click` action, never stored coordinates.
List rows do not expose such an action consistently, so use a debug launch hook such as
`MAILCAL_OPEN_SUBJECT="HTML message with a remote image" scripts/dev/boot.sh linux` to establish
that state rather than claiming a synthetic row click worked. A **conversation** row exposes none
at all: `AdwExpanderRow` publishes `expandable` + `focusable` and leaves opening it to Enter, so
`activate` cannot reach it: and GTK4 offers no way round it either, reporting zero-sized extents
for desktop coordinates and refusing `grabFocus`. The count badge does speak: it carries "N
messages" as its accessible label, so what a screen reader hears is not the bare digit on screen.

Check the session with `printf '%s\n' "$XDG_SESSION_TYPE"`. The control path is AT-SPI and works on
X11 or Wayland. The screenshot adapter uses X11/XWayland window discovery; on a regular GNOME
session it delegates the final capture to `gnome-screenshot`. A raw `xwd` of a composited GTK
window can contain backing pixels rather than what the user sees, so it is used only inside the
private, compositor-free Xvfb test session. Use only the demo or Stalwart harness for captures.

## Package as a Flatpak

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.gnome.Platform//50 org.gnome.Sdk//50 \
                                org.freedesktop.Sdk.Extension.rust-stable//25.08
clients/linux/package.sh --install --run
```

The runtime is a ~2 GB one-time download and is deliberately **not** installed by the script. The
GNOME runtime is what decouples the app from the host distribution: whatever the user runs, the
bundle links GTK 4.22, libadwaita 1.9 and WebKitGTK 2.52 from the runtime.

The build compiles `--release`, so the debug-only fixtures (`MAILCAL_DEV_ACCOUNT`, showcase mode,
the harness trust path) are absent by construction. It then **generates** the desktop entry and the
AppStream metainfo from the resolved `branding/<brand>-listing.md`, [`VERSION`](../../VERSION)
and the assembled release notes ([`flatpak_metadata.py`](../../scripts/dev/flatpak_metadata.py)), and
validates both against the installed tree with `desktop-file-validate` and `appstreamcli`: a store
rejection caught during the build rather than after an upload. The icons under `flatpak/icons/` are
committed, derived from the one brand source by `flatpak/generate-icons.sh`.

### Keep one installed as your everyday app

`--install` adds a `mailcal-local` remote over `target/flatpak/repo` and installs from it, so the
same command builds and refreshes:

```sh
clients/linux/package.sh --install          # build, then install or update in place
clients/linux/package.sh --install --run    # and launch it
```

It installs `--user`, so it needs no root and touches nothing outside `~/.local/share/flatpak`.
This is a **real** install: it reads the real keyring and the real accounts, which is the point of
having one; but it means a build from a feature branch is the app you will be reading your mail
in until you install another.

**The remote is re-pointed at this checkout on every install.** It has to be: a worktree builds into
its own `target/`, so a second checkout finds `mailcal-local` already pointing at the first one's
repository, and without this the install quietly serves *that* build and reports success. Check
which one you are actually running when something looks wrong:

```sh
flatpak remotes --user -d | grep mailcal-local          # which repository it installs from
flatpak info --user <app-id> | grep -E 'Commit|Date'    # and which build is deployed
```

If the deployed commit is older than the build you just made, the remote was aimed elsewhere.

To go back to the released app, or to stop tracking a branch build:

```sh
flatpak uninstall --user <app-id>
flatpak remote-delete --user mailcal-local
```

**Sandbox permissions are minimal on purpose**, and the manifest records why each omission is safe:
credentials go through the Secret *portal* (oo7 picks its backend from the sandbox state) rather
than the Secret Service bus name, and attachments go through the FileChooser and OpenURI portals
rather than a `--filesystem` grant. Verified: the app comes up with an isolated, empty keyring, and
its store, log and preferences land under `~/.var/app/<app-id>/`: `eu.allodia.mailcal`
for an Allodia build, `org.mailcal.client` for an unbranded one (docs/branding.md).

**GTK single-instances on the application id.** An installed Flatpak and a `target/debug` build own
the same name, so whichever starts second hands off and exits without reading its own environment,
which looks exactly like a launch flag being ignored. `flatpak kill <app-id>` before
running a dev build, and vice versa.

**The local MCP relay ships inside the Flatpak.** Settings → Advanced generates a client snippet
that runs `flatpak run --command=allodia-mcp`, entering the installed app before connecting to its
private data-directory socket. Development builds use the relay beside `mailcal-linux`, or
`allodia-mcp` on `PATH` when the binaries are installed separately.

### Known gaps

- **Not submitted to Flathub yet.** The listing is generated but not pushed, so the screenshots
  the store would show are captured rather than published
  ([`store-listing.md`](../../docs/store-listing.md)): which is also why the generated metainfo
  carries the sovereignty framing and no feature bullets.
- **Flathub builds with no network and requires every source declared in the manifest.** This
  manifest instead passes `--share=network` to the build and lets cargo fetch crates.io and the
  pinned engine commit. A submission additionally needs a generated `cargo-sources.json`
  (flatpak-builder-tools) regenerated whenever `Cargo.lock` moves.
- **The gallery has no published images.** `showcase.sh linux` captures them; nothing uploads them
  yet (see `store-listing.md` → Known gaps).

## Capture the showcase set

```sh
scripts/dev/showcase.sh linux                      # 6 screens x 7 languages
scripts/dev/showcase.sh linux --locale de --screen calendar
```

`signatures` is captured on request rather than by `--screen all`. Asking for a screen this
client cannot reach makes it **exit 2** rather than fall back to the mailbox list, because a clean
capture of the inbox filed under another screen's name is the one failure nothing downstream can
detect. `scripts/ci/check-showcase-flag.sh` holds the offered list and the client's accepted list
together.

## Verify

```sh
cargo clippy -p mailcal-linux --all-targets --all-features -- -D warnings
xvfb-run --auto-servernum cargo test -p mailcal-linux --all-features
cargo doc -p mailcal-linux --no-deps
cargo build --release -p mailcal-linux
/usr/bin/python3 -m unittest scripts/dev/tests/test_linux_ui_atspi.py
scripts/dev/test-linux-ui.sh --start-harness
scripts/dev/test-linux-calendar-perf.sh
clients/linux/package.sh
```

The headless wrapper builds with `dev-harness`, creates private Xvfb, D-Bus, AT-SPI, portal, and XDG
fixtures, and opens the seeded HTML message through an exact debug-only subject hook. It drives the
first-run analytics preview, consent and withdrawal; the time-zone prompt; foreground progress;
Settings navigation; Diagnostics view/export; periodic calendar refresh; and notifications off then
on, proving the portal sees only the enabled message. It goes on through reading, reply/send, search,
calendar visibility/colour/default selection, calendar create → detail → edit → delete, meeting
invitations, contacts, recipient autosuggest, signatures, and mail actions. It ends on
`stalwart-multi`, the only shape that can show one person filed in two accounts. All of it runs
against the Stalwart harness through semantic AT-SPI actions; no stored coordinates or synthetic key
events are involved. Screenshots, the final tree, and
privacy-safe stdout/stderr land under `target/ui-test-artifacts/linux/<timestamp>`; a failure captures
the tree and window before teardown. `--no-build` reuses the current debug binary.

When the wrapper itself runs inside Codex's bubblewrap, WebKit cannot create a second unprivileged
user namespace. The wrapper detects that case and disables only WebKit's nested process sandbox;
the outer Codex sandbox still contains the deterministic harness run. A normal developer or CI run
keeps WebKit's own sandbox enabled.

The calendar performance script needs a real display and GPU. It builds an optimized
`dev-harness` binary inside the pinned GNOME SDK, scrolls a week holding at least 125 events for 600
frames, and reads completed GDK presentation timestamps. It fails if p90 exceeds 1.5 refresh
intervals or more than 5% of the motion misses that boundary. The sighted measurement runs without
semantic overlay nodes; `test-linux-ui.sh` separately proves the same grid with AT-SPI enabled.

Also exercise `scripts/dev/boot.sh linux --account demo` when changing row rendering. The demo
deliberately covers localised/plain-text punctuation such as `&`; it must launch without a
GTK/Pango markup warning.
