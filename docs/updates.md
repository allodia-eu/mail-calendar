# Telling a person whether they are running the old one

Every client updates itself, and on no platform do we write the updater: a store or a package
manager owns that. What this contract covers is the part that is ours, which is **answering the
question the user actually has**. Not "please update me", which nobody should have to ask, but "am I
on the old one, and if so, when does that stop?"

It matters because the failure is silent by construction. An app that has quietly stopped updating
looks exactly like an app that is up to date, and the user finds out months later, from a bug that
was fixed in a release they never received.

## The rule

**Settings → About carries an Updates group**, under the version, saying two things:

1. **What keeps this copy current**, named as the thing the user would go to. Not a general claim
   that the app updates itself: the mechanism, because a user sent to the wrong one finds nothing
   there and concludes the app is broken.
2. **A way to ask now**, whose answer is one of exactly three: this is the latest version, an update
   is waiting, or the check did not get through.

Three rules bind every platform:

- **The mechanism is read from the platform, never from the brand or the build.** A client works out
  how it was installed by asking the OS, because the same build can arrive by more than one route
  and only the OS knows which.
- **A check that failed is never reported as up to date.** "Could not tell" and "you are current"
  are different answers, and collapsing them is the one mistake that turns a broken update channel
  into silence. A platform whose API has an "unknown" state maps it to the failure, not the success.
- **A client never claims to have installed anything it did not.** Where the platform downloads on
  its own schedule and applies at the next launch, the copy says that, because "updating now" that
  visibly does nothing is worse than an honest wait.

Nothing here belongs in the core. It is not a product decision dressed as a platform detail: there
is no shared state, no snapshot and no intent, only a different OS API per platform answering a
question only that OS can. The core supplies the running version through `AboutInfo` and stops there.

## Per platform

| | What updates it | Where a person asks | Ships |
|---|---|---|:---:|
| **Windows**, Microsoft Store | the Store | Settings → About → Updates, in place | ✅ |
| **Windows**, our own download | App Installer, from the `.appinstaller` ([`windows-channels.md`](windows-channels.md)) | Settings → About → Updates, in place | ✅ |
| **macOS**, Mac App Store | the App Store | ⬜ | ⬜ |
| **iOS / iPadOS** | the App Store | ⬜ | ⬜ |
| **Android**, Google Play | Play | ⬜ | ⬜ |
| **Linux**, Flatpak | the user's Flatpak remote, through their software centre | ⬜ | ⬜ |

Windows is first because it is the platform where the question is real today: it is the only one
that ships through two mechanisms, and its hosted channel is the only one where nobody else is
watching. The rest are a store the user already knows how to open, which is why they are a gap
rather than a defect.

### Windows

`Package.Current.GetAppInstallerInfo()` is the discriminator, and it is the platform's own answer
rather than an inference: a package installed from an `.appinstaller` has one, and every other
packaged build has none. Reading the identity name instead would be a guess, wrong for a
hand-sideloaded build, which is exactly the build a support question comes from.

- **Hosted**: `Package.Current.CheckUpdateAvailabilityAsync()`. It asks; it does not download. The
  copy therefore says an update installs at the next start, because that is what happens. A link to
  the `.appinstaller` is offered alongside for anyone who wants it sooner, since the
  `ms-appinstaller:` protocol that used to make that one click is disabled on consumer machines.
- **Store**: `StoreContext.GetAppAndOptionalStorePackageUpdatesAsync()`, and an empty list is the
  "up to date" answer. `RequestDownloadAndInstallStorePackageUpdatesAsync` then does the work, with
  the Store's own progress and consent, so this is the one channel where the app can finish the job
  rather than point at it. Two constraints come with it: the call must be awaited on the UI thread
  with an owner window set on the `StoreContext`, or it fails as `ERROR_INVALID_WINDOW_HANDLE`; and
  it performs at most **one real check every 30 minutes**, and ten a day, returning the last known
  status past either. So the copy says what is true of a check rather than claiming a fresh one.

  ⚠️ **It is not interchangeable with the hosted check.** `Package.CheckUpdateAvailabilityAsync`
  asks App Installer about a URL the Store never gave us, and `StoreContext` asks the Store's
  licensing service about a product it has no record of for a hosted copy. Either one pointed at
  the other's channel answers "nothing is waiting", forever and without erring.
- **Unpackaged** (the dev loop): the group is absent. There is no package to replace, and a button
  that could only ever fail is worse than its absence.

`Required` folds into "available" and `Unknown` folds into "could not check"
(`AppIdentity.CheckForUpdateAsync`), and `UpdatesTests` pins the channel resolution, which
`Package.Current` makes untestable at its call site.

## Known gaps

- **Only Windows ships it.** The four rows above are the work, not a decision that those platforms
  do not need it.
- **No notification.** A waiting update is visible in Settings and nowhere else. A mail client that
  interrupts to talk about itself is a trade worth refusing until asked for.
- **Nothing reports a channel that has gone quiet.** An installation whose `.appinstaller` URL stops
  resolving reports "could not check" when someone looks, and nothing when nobody does.
