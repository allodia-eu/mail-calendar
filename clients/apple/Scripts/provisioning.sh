#!/usr/bin/env bash
# Which installed provisioning profile signs a macOS or iOS build of this app, and with which of
# our certificates.
#
# Sourced, never run:
#
#     . "$HERE/Scripts/provisioning.sh"
#     resolve_profile <app-identifier> <cert-fingerprints> <explicit-path> <app-group-or-empty> \
#                     <platform> <kind>
#
# `kind` is `distribution`, a Store profile, which lists no devices, or `development`, one that
# lists THIS Mac. Prints `path<tab>fingerprint<tab>uuid`, or nothing when none qualifies.
#
# One implementation for every caller: Scripts/package.sh signs the two Store submissions with it,
# Scripts/build-and-run.sh --sandboxed signs a local build carrying the same sandbox. The predicate
# below is subtle enough that a second copy would be the one nobody tested, and its comments record
# what each clause is load-bearing for.
#
# The bash 5 floor is the sourcing script's to check (AGENTS.md, "Building & verifying"); both
# already do, package.sh itself and build-and-run.sh through scripts/dev/lib.sh. A guard here
# would be unreachable code, which is why this file is named in SOURCED_ONLY in
# scripts/dev/tests/test_bash_version_floor.py rather than carrying one.

resolve_profile() {
  python3 - "$1" "$2" "$3" "$4" "$5" "$6" <<'PY'
import glob, hashlib, os, plistlib, subprocess, sys
appid, explicit, group, platform, kind = (
    sys.argv[1], sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6])
candidates = {s.strip().upper() for s in sys.argv[2].split() if s.strip()}

def decode(p):
    try:
        return plistlib.loads(subprocess.run(['security', 'cms', '-D', '-i', p],
                                              capture_output=True).stdout)
    except Exception:
        return None

# This Mac, as a provisioning profile names it. NOT IOPlatformUUID, which is what every snippet
# online reaches for and is a different identifier on Apple silicon; the portal registers the
# "Provisioning UDID", and a profile built around anything else lists a device that does not exist.
# Read only for a development profile, the one kind whose device list decides anything.
def provisioning_udid():
    out = subprocess.run(['system_profiler', 'SPHardwareDataType'],
                         capture_output=True, text=True).stdout
    for line in out.splitlines():
        if 'Provisioning UDID' in line:
            return line.split(':', 1)[1].strip()
    return ''

this_mac = provisioning_udid() if kind == 'development' else ''

# Which of our certs this profile authorises, if any. Returned so the caller signs with THAT one
# rather than by name, the fingerprint is the only unique key when two certs share a name.
def signing_cert(d):
    listed = {hashlib.sha1(c).hexdigest().upper() for c in d.get('DeveloperCertificates', [])}
    match = candidates & listed
    return sorted(match)[0] if match else None

# Every capability the signed app claims has to be in here, not just the app id and the cert.
# Regenerating a profile mints a NEW file beside the old one, both match the app id, both list
# the same cert, so a predicate that stops there is choosing between an outdated profile and a
# current one on a coin toss it does not know it is flipping. (It was worse than a coin toss: the
# scan was alphabetical, so `6186a9b6…` beat `a91000f9…` and the STALE profile won every time,
# deterministically, no matter how many times you regenerated.)
# The same entitlement under two names: macOS profiles carry
# `com.apple.application-identifier`, iOS profiles carry a bare `application-identifier`. Reading
# only one of them finds every profile on one platform and none on the other.
def application_identifier(d):
    ent = d.get('Entitlements', {})
    return ent.get('com.apple.application-identifier') or ent.get('application-identifier')

def authorizes(d):
    if not d or application_identifier(d) != appid:
        return False
    # Both platforms' profiles carry the same application-identifier, so the app id alone chooses
    # between a macOS profile and an iOS one by whichever the scan reached first. `Platform` is a
    # list because one profile can serve several: an iOS profile reads ['iOS', 'xrOS', 'visionOS'].
    if platform not in (d.get('Platform') or []):
        return False
    # A profile that lists devices is a development or ad-hoc one. For a Store flow that is never
    # the right answer and Xcode rejects it by name. For a local sandboxed build it is the only
    # right answer, and it has to list THIS Mac: one that does not gets past every check here and
    # then the app is killed before `main()`, with nothing on stdout to say why (SIGKILL, AMFI
    # -413 "No matching profile found"), which reads as a broken build rather than a missing grant.
    devices = d.get('ProvisionedDevices') or []
    if kind == 'development':
        if this_mac not in devices:
            return False
    elif devices:
        return False
    # The Store flows sign manually, and `-exportArchive` refuses an Xcode-managed profile outright:
    # "is Xcode managed, but signing settings require a manually managed profile". Xcode mints these
    # for itself whenever it resolves signing, so a machine that has ever opened the project has
    # several, and they sit beside the portal-created one matching everything it matches. A local
    # build exports nothing, so there a managed profile granting the group is as good as any other.
    if kind != 'development' and d.get('IsXcodeManaged'):
        return False
    # The group is a macOS Store requirement: that app declares one and the profile has to grant it.
    # An iOS App Store profile has no group to grant, so an empty argument means "do not ask", which
    # is different from asking for '' and is why it is a separate branch rather than a default.
    if group and group not in d.get('Entitlements', {}).get(
            'com.apple.security.application-groups', []):
        return False
    return signing_cert(d) is not None

if explicit:
    ex = os.path.expanduser(explicit)
    d = decode(ex)
    if authorizes(d):
        print(f"{ex}\t{signing_cert(d)}\t{d.get('UUID', '')}")
    sys.exit(0)

# Newest first among the qualifying ones: after a regeneration the freshest profile is the one
# that reflects the App ID as it stands today, and the older ones are debris nobody prunes.
found = []
for base in ('~/Library/MobileDevice/Provisioning Profiles',
             '~/Library/Developer/Xcode/UserData/Provisioning Profiles'):
    for ext in ('*.provisionprofile', '*.mobileprovision'):
        for p in glob.glob(os.path.join(os.path.expanduser(base), ext)):
            d = decode(p)
            if authorizes(d):
                found.append((d.get('CreationDate'), p, signing_cert(d), d.get('UUID', '')))
if found:
    found.sort(key=lambda c: (c[0] is not None, c[0]), reverse=True)
    print(f"{found[0][1]}\t{found[0][2]}\t{found[0][3]}")
PY
}
