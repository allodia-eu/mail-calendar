#!/usr/bin/env bash
# Assert that every executable bundle in a built app carries a privacy manifest declaring each
# required-reason API its executable imports, with a reason Apple lists for that category.
#
#     check-privacy-manifests.sh <app-dir>
#
# App Store Connect runs this check on upload and rejects a build that fails it (ITMS-91053,
# "Missing API declaration"). It reads the binary, not the source: the Rust core is linked
# statically, so a dependency bump that starts calling `mach_absolute_time` or `statfs` changes
# what the app has to declare without a line of Swift moving. So this reads the binary too.
#
# The scan matches import names as NUL-delimited strings in the executable: a Mach-O keeps every
# imported symbol and Objective-C selector name that way, whether the imports sit in the symbol
# table or in chained fixups, and a plain byte search needs neither Xcode nor macOS, so
# scripts/dev/tests/test_check_privacy_manifests.py exercises it on fixture files anywhere.
#
# The symbol and reason lists are Apple's "Describing use of required reason API". A reason
# reserved for third-party SDKs (0A2A.1, C56D.1) is refused: this app is not one.

# Bash 5 or newer, like every script in this tree (AGENTS.md, "Building & verifying").
if [[ ${BASH_VERSION%%.*} -lt 5 ]]; then
  echo "error: ${0##*/} needs bash 5 or newer, and got ${BASH_VERSION:-no bash at all}" >&2
  echo "       macOS ships bash 3.2 as /bin/bash: \`brew install bash\` puts 5 ahead of it" >&2
  exit 1
fi

set -euo pipefail

[[ $# -eq 1 && -d "$1" ]] || { echo "usage: $(basename "$0") <app-dir>" >&2; exit 1; }

python3 - "$1" <<'PY'
import plistlib
import sys
from pathlib import Path

# Category -> (import names that trigger it, reasons this app may declare for it).
CATEGORIES = {
    "NSPrivacyAccessedAPICategoryFileTimestamp": (
        ["_stat", "_stat64", "_fstat", "_fstat64", "_fstatat", "_fstatat64", "_lstat", "_lstat64",
         "_getattrlist", "_getattrlistbulk", "_fgetattrlist", "_getattrlistat",
         "_NSFileCreationDate", "_NSFileModificationDate",
         "_NSURLContentModificationDateKey", "_NSURLCreationDateKey", "fileModificationDate"],
        {"DDA9.1", "C617.1", "3B52.1"},
    ),
    "NSPrivacyAccessedAPICategorySystemBootTime": (
        ["_mach_absolute_time", "systemUptime"],
        {"35F9.1", "8FFB.1", "3D61.1"},
    ),
    "NSPrivacyAccessedAPICategoryDiskSpace": (
        ["_statfs", "_statfs64", "_statvfs", "_fstatfs", "_fstatfs64", "_fstatvfs",
         "_getattrlist", "_fgetattrlist", "_getattrlistat",
         "_NSURLVolumeAvailableCapacityKey", "_NSURLVolumeAvailableCapacityForImportantUsageKey",
         "_NSURLVolumeAvailableCapacityForOpportunisticUsageKey", "_NSURLVolumeTotalCapacityKey",
         "_NSFileSystemFreeSize", "_NSFileSystemSize"],
        {"85F4.1", "E174.1", "7D9E.1", "B728.1"},
    ),
    "NSPrivacyAccessedAPICategoryActiveKeyboards": (
        ["activeInputModes"],
        {"3EC4.1", "54BD.1"},
    ),
    "NSPrivacyAccessedAPICategoryUserDefaults": (
        ["_OBJC_CLASS_$_NSUserDefaults"],
        {"CA92.1", "1C8F.1", "AC6B.1"},
    ),
}

app = Path(sys.argv[1])
bundles = [app] + sorted(
    p for pattern in ("*.app", "*.appex", "*.xpc") for p in app.rglob(pattern) if p.is_dir()
)

problems = []
for bundle in bundles:
    rel = "." if bundle == app else bundle.relative_to(app).as_posix()
    if (bundle / "Contents" / "Info.plist").is_file():
        info, exedir = bundle / "Contents" / "Info.plist", bundle / "Contents" / "MacOS"
        manifest = bundle / "Contents" / "Resources" / "PrivacyInfo.xcprivacy"
    else:
        info, exedir = bundle / "Info.plist", bundle
        manifest = bundle / "PrivacyInfo.xcprivacy"
    try:
        executable = exedir / plistlib.loads(info.read_bytes())["CFBundleExecutable"]
        binary = executable.read_bytes()
    except Exception:
        continue  # Not an executable bundle: check-nested-bundles.sh owns that verdict.

    used = {}
    for category, (names, _) in CATEGORIES.items():
        hits = [n for n in names if b"\0" + n.encode() + b"\0" in binary]
        if hits:
            used[category] = hits

    print(f"    {rel}: {', '.join(c.removeprefix('NSPrivacyAccessedAPICategory') for c in used) or 'no required-reason API'}")

    if not manifest.is_file():
        problems.append(f"{rel}: no {manifest.relative_to(bundle).as_posix()}")
        continue
    try:
        declared = {
            entry["NSPrivacyAccessedAPIType"]: list(entry.get("NSPrivacyAccessedAPITypeReasons", []))
            for entry in plistlib.loads(manifest.read_bytes()).get("NSPrivacyAccessedAPITypes", [])
        }
    except Exception as error:
        problems.append(f"{rel}: {manifest.name} does not parse ({error})")
        continue

    for category, reasons in declared.items():
        allowed = CATEGORIES.get(category, (None, set()))[1]
        if not reasons:
            problems.append(f"{rel}: {category} is declared with no reason")
        for reason in reasons:
            if reason not in allowed:
                problems.append(f"{rel}: {reason} is not a reason this app may give for {category}")
    for category, hits in used.items():
        if category not in declared:
            problems.append(f"{rel}: imports {', '.join(hits)} but does not declare {category}")

if problems:
    print("\n".join("    " + p for p in problems), file=sys.stderr)
    print("error: a bundle's privacy manifest does not cover the required-reason APIs it uses.\n"
          "       App Store Connect rejects this with ITMS-91053. Declare the category with a reason\n"
          "       from Apple's list that is true of the code: clients/apple/README.md, \"Privacy manifests\".",
          file=sys.stderr)
    sys.exit(1)
print("    privacy manifests cover every required-reason API")
PY
