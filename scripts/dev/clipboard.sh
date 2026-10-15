#!/usr/bin/env bash
# Put something on the Mac's clipboard, or read back what is there, for testing a paste.
#
#   scripts/dev/clipboard.sh set-html <file.html> [plain text]   # public.html (+ plain text)
#   scripts/dev/clipboard.sh set-png <file.png>
#   scripts/dev/clipboard.sh set-text "<text>"
#   scripts/dev/clipboard.sh html [out.html]   # public.html as it is, to stdout or a file
#   scripts/dev/clipboard.sh types             # every type on the clipboard, one per line
#
# The read is the oracle for what an editor holds: paste, Select All, Copy, then `html`. WebKit
# copies the editor's DOM as `public.html`, so the answer is the document's own markup, `data-*`
# attachment ids and `data:` pictures included, with no inspector to attach.
#
# An iOS simulator follows this clipboard both ways (.agents/skills/debug-app, "Clipboard"), so the
# same two calls set what an iPhone pastes and read what it copied.
#
# `set-html` writes the file's bytes unchanged. Keep a captured clipboard's `<meta charset>`: an
# HTML flavour that declares none is read as Latin-1 on iOS, and a "·" arrives as "Â·".
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

is_macos || die "clipboard.sh reads and writes the macOS pasteboard"
[[ $# -ge 1 ]] || die "usage: clipboard.sh <set-html|set-png|set-text|html|types> [args...]"
action="$1"; shift

# One JXA program, run with the action and its arguments: osascript passes them to `run`.
jxa() {
  osascript -l JavaScript - "$@" <<'JS'
ObjC.import('AppKit');
function run(argv) {
  const pb = $.NSPasteboard.generalPasteboard;
  const [action, a, b] = argv;
  const file = (path) => {
    const data = $.NSData.dataWithContentsOfFile(path);
    if (data.isNil()) throw new Error('cannot read ' + path);
    return data;
  };
  switch (action) {
    case 'set-html':
      pb.clearContents;
      pb.setDataForType(file(a), 'public.html');
      if (b) pb.setStringForType(b, 'public.utf8-plain-text');
      return '';
    case 'set-png':
      pb.clearContents;
      pb.setDataForType(file(a), 'public.png');
      return '';
    case 'set-text':
      pb.clearContents;
      pb.setStringForType(a, 'public.utf8-plain-text');
      return '';
    case 'html': {
      const data = pb.dataForType('public.html');
      if (data.isNil()) throw new Error('no HTML on the clipboard');
      data.writeToFileAtomically(a, true);
      return '';
    }
    case 'types':
      return ObjC.unwrap(pb.types).map((type) => ObjC.unwrap(type)).join('\n');
  }
}
JS
}

case "$action" in
  set-html) [[ $# -ge 1 ]] || die "set-html <file.html> [plain text]"; jxa set-html "$1" "${2:-}" >/dev/null ;;
  set-png)  [[ $# -ge 1 ]] || die "set-png <file.png>"; jxa set-png "$1" >/dev/null ;;
  set-text) [[ $# -ge 1 ]] || die "set-text <text>"; jxa set-text "$1" >/dev/null ;;
  html)
    if [[ $# -ge 1 ]]; then
      jxa html "$1" >/dev/null
    else
      tmp="$(mktemp)"; trap 'rm -f "$tmp"' EXIT
      jxa html "$tmp" >/dev/null
      cat "$tmp"; echo
    fi
    ;;
  types) jxa types ;;
  *) die "unknown action '$action' (set-html|set-png|set-text|html|types)" ;;
esac
