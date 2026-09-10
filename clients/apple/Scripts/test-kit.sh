#!/usr/bin/env bash
# MailcalKit's own suite: plain logic, no UI, no simulator. The page<->date mapping, the zoom
# clamps, the all-day overflow rule, the localised copy.
#
# All this adds to `swift test` is where SwiftPM builds. Its default `.build` sits inside the
# package and nothing caches it, so the suite recompiled MailcalBindings and all 115 files of
# MailcalUI from scratch on every run, beside a warm Xcode cache. Building under
# clients/apple/build, the directory CI caches, makes an unchanged tree about a second.
#
# A scratch path of its own, and NOT the app's derived data. Pointed at that, SwiftPM's build of
# MailcalUI and Xcode's overwrite each other in Build/Products/Debug, and from then on neither the
# suite nor the macOS app is ever up to date: measured at 26-32s apiece on every run, against
# 1-3s when they are kept apart.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)" # clients/apple
cd "$HERE/Packages/MailcalKit"
exec swift test --scratch-path "$HERE/build/KitSPM"
