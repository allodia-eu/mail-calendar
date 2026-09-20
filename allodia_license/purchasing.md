# Purchasing: the cross-platform contract

How somebody pays for the services Allodia runs, and what every client does with the receipt.
[`entitlement.md`](entitlement.md) decides what a paid account may *draw*; this decides how an
account becomes paid. Everything here is under the [Allodia Licence](LICENSE.md), and the free
application never reaches any of it.

## What is sold, and what it may be called

**One service, two periods, three shops.** The service is what [`docs/pledge.md`](../docs/pledge.md)
already permits charging for: capabilities that exist because Allodia runs a server behind them.
Nothing on the pledge's free list may ever appear here, and a shipped free feature may not move
into it.

⚠️ **It is named as a service, never as an edition.** No "Pro", no "Premium", no "paid version":
the pledge's naming rule is that an edition name makes the free application read as the lesser one,
which promise 1 says it is not. Copy that calls this an upgrade of the app rather than a
subscription to a service is a breach, not a wording preference.

| | Billed | Sold by |
|---|---|---|
| Yearly | once a year | Allodia's own checkout, the App Store, Google Play |
| Monthly | once a month | the same three |

Yearly is drawn first, because it is the cheaper rate per month, and Allodia's own checkout before
a store's, because it is the cheaper shop. `allodia_license::ordered` decides both, so five clients
cannot each sort the list their own way.

## Prices are the shop's, never ours

**No price is written in this repository, and none is computed in it.** A store returns its own
localised price string and that is what a client draws.

Three reasons, and each alone would be enough. Apple requires the store's formatted price be shown
rather than one the app assembled. Currency and number formatting is the platform's job, because
the core carries no locale data at all
([`../AGENTS.md`](../AGENTS.md), "Localisation is client-side"). And the stores charge commission,
which Allodia adds on top rather than absorbs, so the same plan costs more inside a phone app than
on the website: a number kept in both places is wrong the first time either is repriced in a
console.

The markup therefore lives in App Store Connect and the Play Console, not in code. What lives in
code is the product identifier, in `allodia_license::Plan::store_product`, because Apple never
releases a product id for reuse and Play refuses to change one after publication.

⚠️ **The two stores do not shape a subscription the same way.** Apple sells a product per period
inside one subscription group; Play sells one subscription carrying a base plan per period, so the
id alone does not name a price there. `StoreProduct` says which is which. Treating them as one
shape leaves the Play client asking for a product that was never published, which Play reports as
an empty result rather than as an error.

## Signing in comes first

**A store purchase is redeemed against a signed-in Allodia account, so the sign-in happens before
the purchase, not after it.**

This falls out of [`entitlement.md`](entitlement.md) rather than being a separate decision: the
server resolves entitlement from the access token, and every capability a plan turns on needs
Allodia to do something. A purchase with no account attached would buy the use of servers that
have no idea who is asking. It is also what carries a purchase between a person's devices and
across platforms: somebody who subscribes on the website has it on their phone, and somebody who
subscribes on their phone has it on their desktop.

A client therefore offers the sign-in **before** the purchase buttons, from the same card
`entitlement.md` already describes, and never puts a payment sheet in front of somebody who cannot
yet be granted what they are paying for.

⚠️ **The subscription scopes raise no standing prompt.** `mailcal:subscription:read` and
`mailcal:subscription:write` are not in `Feature::ALL`, which is the set the account card offers
"sign in again" for. Opening the account screen and buying something are both deliberate, so the
prompt belongs on that screen; and every grant issued before those scopes existed carries neither,
so listing them there would tell every signed-in person to sign in again about a screen most of
them will never open. What gates the call is `grant_permits`, where the call is made.

## The one rule everything else follows from

**A purchase is never settled with the store until the account service has attached it.**
`POST /subscription/store` is what attaches it, and nothing is settled before it has answered.

Both stores are built around that, from opposite directions, and both are protecting the same
person:

| | What an unsettled purchase does | What settling early costs |
|---|---|---|
| StoreKit | re-delivered at every launch, indefinitely | the only copy of a purchase somebody paid for is gone |
| Play Billing | **refunded and revoked after three days** | the money is taken and nothing is granted |

⚠️ **Who settles it is not the same on the two platforms, and no client may assume it is.** A Play
purchase is acknowledged on the account's side once it has been attached, so an Android client
acknowledges nothing and `AllodiaRedeemReport::finish` is a list it ignores. A StoreKit transaction
can only be finished by the device that holds it, so on Apple that stays the client's job and the
list is exactly what it finishes.

**Read `finish` and do what it says.** It is empty on Play and populated on Apple, and a client
that hard-codes either has written down a split that is not its to know.

`allodia_license::Ledger` tracks a purchase in between, and `Ledger::apply` is the only thing that
returns `Settled::Settled`.

**The ledger stores nothing, and that is the point of it.** The store is the durable copy: it
offers an unfinished purchase again at every launch, and it says when the purchase was made. So the
ledger holds retry pacing for the life of the process and no more. There is no file to corrupt, no
preferences key to migrate, and nothing for five clients to implement.

Two things follow, and both are better than what persistence would have bought:

- **No signed transaction is ever written to the device.** `Proof` derives no `Serialize` at all,
  which makes that structural rather than a rule somebody has to keep remembering. A signed
  transaction is presentable by whoever holds it.
- **How long somebody has been waiting comes from the store's own timestamp**, not from an attempt
  count. A count resets every time the app is killed, which on a phone is the ordinary case, so a
  purchase could have been stuck for a day and never reach a threshold measured that way.

Three more consequences worth stating rather than discovering:

- **The store is the authority on what is outstanding**, which is why `Ledger::sync` takes the
  whole set rather than one purchase at a time. StoreKit re-delivers every unfinished transaction
  at launch and again through its updates stream, and Play returns it from every
  `queryPurchasesAsync`, so a ledger that appended blindly would grow without bound and redeem one
  purchase once per copy; and a purchase the store has stopped offering, refunded or finished from
  somewhere else, has to leave.
- **The redemption carries the purchase's own id as its idempotency key**, because a redemption
  whose response was lost must grant once rather than once per retry.
- **Retrying is capped at an hour.** A backoff left to double would pass Play's three-day refund
  clock, which turns a service having a bad afternoon into a refund nobody asked for.

## Every purchase names the account it is for

A purchase carries an opaque id the app chooses, `appAccountToken` on Apple and
`obfuscatedAccountId` on Play, and it is **the Allodia Account subject**: the identity the account
service issued, which every client already holds from its own sign-in.

**Not the subscription service's own user id**, which is a different value. That one is created
when the service first sees a sign-in and is this service's mirror of the identity; the subject is
the identity. A purchase outlives mirrors, and Apple keeps the token for the life of the
subscription, so the token names the thing that does not move.

⚠️ **It cannot be attached afterwards.** A purchase made without one is never taggable, so every
purchase carries it from the first, not from the first that turned out to need it.

**What it is for is the purchase whose report never arrived.** The ordinary path is the device
naming its own transaction; this is the path for a device that paid and then lost the network or
was killed before it could. The store's own notification about the renewal or the refund is then
all that reaches the account service, and the token is the only thing in it that says whose money
it was. Apple requires a UUID and refuses anything else; Play takes any string.

It identifies an **account, not a person**: an opaque id the service already holds, carrying no
address and no name, and it is never drawn on a screen.

## The device names the purchase; it says nothing about it

**One identifier crosses, and nothing else**: Apple's StoreKit transaction id, or Play's purchase
token. The service then reads every fact about the subscription back from the store itself, with
keys that exist only on Allodia's servers, so nothing a device claims about what it bought is
believed, and there is no store payload in this repository to parse, sign or get wrong.

That is [`../AGENTS.md`](../AGENTS.md)'s "check before you parse" arriving at its conclusion: not
"parse it carefully" but "there is nothing here to parse". It is also why the body names no
account. **The account is the access token's**, exactly as `GET /api/v1/entitlement` takes no
input, because a device that could name the account could attach somebody else's purchase to
itself.

An identifier is still a claim: presenting one attaches the subscription to whoever is signed in,
which is precisely what `owned_by_another_account` reports happening to somebody else. So it never
reaches a log line ([`../docs/logging.md`](../docs/logging.md)), and both `StorePurchase` and
`Pending` redact it from `Debug`.

**Nor does the device name a period.** Apple's transaction names its product, but a Play purchase
names only its subscription: which base plan was bought is resolved by
`purchases.subscriptionsv2.get` and nowhere else. A device that reported a period would be guessing
on one of the two platforms, and a field that is wrong half the time is worse than one that is
absent.

## Two answers, which a client may not collapse

The distinction [`entitlement.md`](entitlement.md) makes about reading an entitlement applies again
to redeeming a purchase, for the same reason.

| | What it means | What happens to the purchase |
|---|---|---|
| Linked | the service attached it | dropped from the ledger, and settled with the store |
| Claimed elsewhere | `owned_by_another_account`: already attached to a **different** Allodia account, which is what restoring purchases onto a second one looks like | dropped and settled, and the person is told which of the two happened |
| Deferred | nothing was learned: an outage, a captive portal, an expired token, a deployment with no billing, or a purchase the store does not recognise **yet** | kept, nothing is settled |

**The refusal's own `data.code` decides, not the status.** The service answers `404` for more than
one thing and a `409` could later carry a code this build has never heard of, so reading the status
alone would conclude from a shape rather than from a reason. Everything that is not a success and
not `owned_by_another_account` defers: discarding somebody's purchase on the strength of a code a
later service invented is the one mistake here that cannot be undone.

⚠️ **`unknown_purchase` is retried, not concluded from.** A transaction made seconds ago is
routinely not yet queryable at the store, so the first answer about a perfectly real purchase can
be that one. Retrying costs a request an hour once the backoff has opened up; treating it as final
would throw away a purchase somebody had just made.

**A purchase that keeps failing is said out loud.** After roughly an hour of trying, a client says
so. Money was taken and nothing was granted, and saying nothing is the one response that leaves the
person with no way to find out.

## Steering to Allodia's own checkout

Allodia's own checkout is the cheaper shop and a client offers it where the platform permits.
**Whether it may be drawn is asked of the platform, never assumed**, because the answer varies by
storefront and changes without a release:

- **Apple** requires the external-purchase-link entitlement, a target declared in the app's
  property list, and Apple's own disclosure sheet shown before the browser opens. It is available
  only in the storefronts Apple has opened it in, under terms specific to each.
- **Play** requires enrolment in an alternative-billing or external-offer programme, per app and
  per country.
- **Windows, Linux and the website** have no such restriction: the checkout is a link.

⚠️ The exact entitlements, programmes and terms are each platform's to state and both have moved
repeatedly. Confirm them with Apple and Google before shipping rather than from this document;
what binds here is the shape, which is that the client asks and the core never assumes.

**On a platform where the link may not be drawn, the store price stands alone** and the app says
nothing about a cheaper price elsewhere, because both stores forbid it and the review that catches
it blocks the release.

### The shop follows the channel, not the platform

⚠️ **"Android" does not name one answer, because a store's payment rules bind the builds that
store distributes.** Play's apply to an app published through Play. They do not reach an APK
published on F-Droid or downloaded from Allodia, which therefore sells through Allodia's own
checkout with no programme to enrol in, exactly as Windows and Linux do. Reading the rule off the
platform instead of the channel gets the un-Googled build wrong in the expensive direction: it
would offer a person no way to pay at all.

**Android ships two flavours, `play` and `foss`**, and they are different APKs rather than one
behaving differently. Play Billing is a stub that binds to the Play Store app over IPC, so on a
device without one it can only answer `BILLING_UNAVAILABLE`; degrading gracefully is enough to
avoid a crash and **not** enough for F-Droid, whose inclusion policy is about what a build
contains. The flavour therefore drops the dependency (`playImplementation`), the Play sources
(`src/play`) and the merged `com.android.vending.BILLING` permission together.

| | `play` | `foss` |
|---|---|---|
| Sold by | Google Play | Allodia's own checkout |
| Google library in the APK | yes | **none** |
| `com.android.vending.BILLING` | merged in | **absent** |
| Goes to | Play | F-Droid, and the download we host |

Each flavour supplies its own `allodiaBillingProvider`, so nothing above that line branches on the
build. ⚠️ **The two shops do not have the same shape and the seam does not pretend otherwise**: Play
hands a purchase back for the device to attach, while a checkout hands back nothing at all, because
the person leaves for a browser and the subscription is created on the account. That is what
`SentToCheckout` is for, and why the checkout side reports nothing outstanding rather than leaving
it unimplemented.

This is not the brand axis. An unbranded `play` build still carries Play Billing, because branding
decides what a build is called and the flavour decides where it is sold.

## The account screen, and who may change what

`GET /subscription` answers the whole screen in one read: the subscription Allodia bills directly,
any the stores bill, today's list prices, and what the caller may actually do.

⚠️ **It gates nothing.** `GET /entitlement` does, and the two answer different questions: that one
is "what do I switch on", asked constantly and cached for thirty days, and this one is "what do I
say on this screen", asked when somebody opens it. Gating on this one would put a network read in
front of a feature, which is the single rule [`entitlement.md`](entitlement.md) is built on.

**`actions` decides which buttons exist, and a client draws exactly that.** It is passed through
rather than re-derived, because the rule it encodes has to account for three billers at once:
`canStartCheckout` is false for anybody already paying through *any* source, which is what stops a
desktop build selling a second subscription to somebody the App Store is charging. A client working
the same thing out locally would be a fourth copy of a rule that only the service can hold.

| | Who can change it | How |
|---|---|---|
| Allodia's own subscription | this API | cancel, switch period, resubscribe, start a checkout |
| A store's subscription | that store, and only that store | open its `manageUrl` |

⚠️ **A store's subscription is read-only here, and sending somebody to Allodia's page to cancel one
is how a cancellation quietly does not happen.** Cancelling, changing plan and refunding all belong
to the store that took the money.

⚠️ **One way in per store, never one per subscription.** An account can carry more than one live
subscription at a single store, because resubscribing leaves the lapsed one inside the period it
was paid for, and a store's own page belongs to the store rather than to any one subscription. A
client therefore names each biller once and offers each store's `manage_url` once. Two questions
decide it, and they part company exactly where it matters:

| | Which states | What it drives |
|---|---|---|
| Who is billing you | active, grace, cancelled | the biller's name, and `duplicateBilling` |
| Is there anything to do at the store | those three, plus on hold and paused | the manage route |

On hold and paused grant nothing, so neither may claim the first; but a failed card is replaced at
the store and a pause is lifted there, so withholding the second strands the person who most needs
it.

Three things a client has to say out loud, because each is money and none of them is visible from
the button:

- **Cancelling refunds nothing.** A month is sold as a month; access runs to `endDate`.
- **Switching period changes the next charge and nothing else.** No money moves today, and
  `nextPaymentDate` comes back unchanged. Left unsaid, the switch is silent until a date that may
  be a month away.
- **Two sources can charge at once.** `duplicateBilling` reports it and resolves nothing: a store
  will sell a second subscription without asking this service. A client names both and offers both
  exits, because cancelling one of them uninvited is a decision about somebody's money. Allodia's
  own billing is **named Allodia**, never the payment processor behind it: naming a company
  somebody has never heard of, in a message about being charged twice, is how a correct warning
  reads as a scam.

**Every refusal is a code, never a sentence.** The service sends its own `message` and no client
reads it, the same rule grant health already keeps. `Refusal` is what a client switches on, because
"you have already cancelled" and "that cannot change while a charge is being retried" are different
things to say and only the code distinguishes them.

**Prices arrive as minor units and a currency**, and a client formats them. The core carries no
locale data, so turning 199 and `EUR` into something readable is the platform's job, exactly as the
store's own formatted string is on the other route.

## What the stores require of the app itself

Two obligations that are review-blocking rather than optional, and one that turns out not to apply.

- **A way to manage or cancel the subscription** has to be reachable from inside the app. Apple's
  is `AppStore.showManageSubscriptions`, Play's is a deep link to its own subscriptions page.
  Neither is Allodia's page: a subscription bought from a store is cancelled at that store, and
  sending somebody to the wrong one is how a cancellation quietly does not happen.
- **The price, the period and what renewal means** are shown next to the purchase button, in the
  store's own words for the price. This is the anti-hype coupling too: the copy may not out-run
  [`../docs/capabilities.md`](../docs/capabilities.md).
- **Restoring purchases needs no button of its own.** The usual reason for one is a device that
  holds a purchase the app cannot see, and that cannot arise here: an entitlement is resolved from
  the Allodia account, so signing in *is* the restore, on any platform and any device, including
  ones with no store at all. What a store purchase adds to that account it adds permanently.

## Sovereignty scope

**Buying from the store that distributed the build is a carve-out from the `JurisdictionGate`**,
the fifth [`../AGENTS.md`](../AGENTS.md) carries. Decided 2026-09-18.

**What it covers.** A purchase dispatched to Apple through StoreKit or to Google through Play
Billing, and nothing else. The dispatch carries a product identifier and the account id the
purchase is tagged with; it happens only when somebody presses a purchase button; and the
counterparty is the platform the person bought their device from and is already signed in to.
[`../docs/privacy-policy.md`](../docs/privacy-policy.md) §§9 and 10 describe both stores as
independent controllers processing the purchase under their own policies.

**Why it cannot be gated rather than carved out.** StoreKit and Play Billing reach Apple and
Google through the operating system, not through a transport this app routes, so there is no point
at which a gate could stand. It is a property of the platform rather than a choice this app makes,
which is what separates it from a dispatch we could have routed and did not.

⚠️ **It reaches only the builds those stores distribute.** Android's `foss` flavour sells through
Allodia's own checkout, carries no Play Billing at all, and is therefore outside this entirely:
reading the carve-out off the platform rather than the channel would grant it to a build that
never makes the dispatch. The same rule as
[the shop follows the channel](#the-shop-follows-the-channel-not-the-platform), for the same
reason.

**What ends it.** It lapses on a platform as soon as Allodia's own checkout is reachable from
inside the app there, because the store dispatch is then no longer the only way to buy and the
carve-out is no longer load-bearing. For EU storefronts that turns on the external-purchase-link
entitlement, which is a date rather than a hope.

⚠️ **The transfer the policy names in §12 is a different one, and arguing the two as one would
rest this carve-out on something it does not cover.** That transfer is the account service asking
Apple or Google to verify a purchase, which happens on Allodia's side and leaves no device.
`JurisdictionGate` governs what leaves the **app**, so the policy's account of that transfer
neither supports this carve-out nor stands in its way.

The published policy describes this: `allodia.eu/privacy/mail-calendar` renders version 2.4 of
2026-09-18 in both locales, whose §10 covers all three routes. That gate is met.
[`../docs/privacy-policy.md`](../docs/privacy-policy.md) has since moved to 2.5, which rewrites
what the app does with contacts and the sender name and leaves §10 as it stood, so the page a
buyer reads is still the one that describes buying.

## What a client calls

The core decides; the client talks to the store it is running on, because no Rust exists that can.

| | |
|---|---|
| `Plan::store_product(store)` | what to ask StoreKit or Play for |
| `ordered(offers)` | the offers the store returned, in the order to draw them |
| `Ledger::sync(&purchases)` | every unfinished purchase the store now offers; new ones are taken up, ones it has stopped offering are dropped, retry pacing is kept |
| `Ledger::due(now)` | which purchases to attempt, oldest first |
| `AccountService::link_store_purchase(...)` | `POST /subscription/store`: attach one to the signed-in account |
| `Ledger::apply(id, outcome, now)` | fold in the answer, and learn whether the store is still owed anything |
| `Ledger::has_stuck(now)` | whether to tell the person a purchase has not gone through |
| `AccountService::subscription(...)` | `GET /subscription`: the whole account screen in one read |
| `AccountService::start_checkout(...)` | a hosted payment page to open in a **browser** |
| `AccountService::cancel_subscription(...)` | stop the recurring charge; access runs to `endDate` |
| `AccountService::switch_interval(...)` | change the next charge, and nothing today |
| `AccountService::resubscribe(...)` | restart on the authorisation already held, or a page |

## Per-platform status

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| The product catalogue, the ordering and the ledger | ✅ | n/a | n/a | n/a | n/a | n/a |
| Attach a store purchase, over the shared account-service transport | 🚧 | n/a | n/a | n/a | n/a | n/a |
| The account screen's read | ✅ | n/a | n/a | n/a | n/a | n/a |
| Its four writes: checkout, cancel, switch period, resubscribe | 🚧 | n/a | n/a | n/a | n/a | n/a |
| Talk to the platform's store: fetch, buy, collect, finish | n/a | 🚧 | 🚧 | n/a | ✅ | n/a |
| That half tested against a **simulated** store | n/a | ✅ | n/a | n/a | n/a | n/a |
| Draw the account screen: state, prices, buy | n/a | 🚧 | 🚧 | ✅ | ✅ | ✅ |
| Draw its four writes: checkout, cancel, switch period, resubscribe | n/a | 🚧 | 🚧 | ✅ | ✅ | ✅ |
| Open Allodia's own checkout in a browser | n/a | ⬜ | ⬜ | ✅ | ✅ | ✅ |
| Link out to that checkout from inside the app | n/a | ⬜ | ⬜ | n/a | ⬜ | n/a |
| Reach the store's own manage-or-cancel page | n/a | ✅ | ✅ | ✅ | ✅ | ✅ |
| Build with no Google library in it at all | n/a | n/a | n/a | n/a | ✅ | n/a |

Legend as [`README.md`](../README.md): ✅ shipped · 🚧 in progress · ⬜ planned · n/a not applicable.
Windows and Linux ship no store purchase: the Microsoft Store's commerce is not used and Flatpak
has none, so on both the only route is Allodia's own checkout, which both now draw. **Android is
two builds**: both are compiled and tested by `:app:test` in the gate, and the `foss` one opens the
checkout, which is why that row is ✅ for the three builds that sell through it and nowhere else.

⚠️ **The manage-or-cancel row is a store's obligation on a phone and the product's everywhere
else.** No store distributes the desktop builds and none reviews them, but one account carries
every device, so somebody whose subscription was bought on a phone opens this screen on a desktop
and needs the same page. What each opens is `manage_url` from the read, never a page assembled by a
client.

**Android's `play` half has been driven end to end against the real store** (2026-09-18, physical
device, production account service). The catalogue resolved, the sheet opened on a licence tester's
test card, and the purchase was taken, attached and granted without anything else being touched;
the card then redrew itself as paid, naming Google Play and offering its manage page. A later
**fresh install** of the app, which had bought nothing, signed in and came back paid from the
account alone, which is the cross-device claim proven on this platform rather than argued.

Both flavours draw the same card, under the account it belongs to, and each gets the shop its
flavour supplies without the card knowing which: `play` shows Play's own prices behind a buy
button, `foss` shows Allodia's two prices and opens the checkout in a browser. The **`foss`
checkout has still never been paid on** from Android, which is what holds that flavour where it is,
even though the page it opens has since been paid through from Windows.

⚠️ **What Play withholds until an app is published is the catalogue, and nothing else about the
build.** `queryProductDetails` answers nothing at all until a build has reached a track, which
reads exactly like a wrong product id and cost a day's worth of guessing; once one has, an
ordinary debug build installed over `adb` fetches, buys and collects like any other. The signing
key is not the gate, and believing it was sent this work down a slow loop of release builds it
never needed.

⚠️ **A sign-in older than the permission this read needs is an offer, not an outage**, and the
first device this card was opened on had one. The remedies are opposites, so the read's failure
alone may not decide the sentence: `allodia_grant_health` does, exactly as the account list's own
failure does ([`entitlement.md`](entitlement.md)). Both screens read it.

**Apple's screen is 🚧 rather than ✅, and the distance is not code.** Settings → Allodia account
draws the subscription: who is charging, until when, a retry that is not a lapse, every biller when
more than one is charging, the store's own manage page, the two periods with the store's own prices
behind a buy button, and the three writes that change a subscription Allodia bills directly, each
where `actions` permits and the two that cost money behind a confirmation. A purchase has been made
against the App Store **sandbox** end to end: taken, attached, granted, finished, and read back on
a second platform that never saw it.
What holds it at 🚧 is that no purchase has been made against the **production** store. That is
now the only thing between this screen and ✅: the published policy covers buying, so a sandbox
purchase proving the round trip is the last claim on this row still made from a test store rather
than a real one.

**What each mark means here, precisely, because a matrix that overstates is worse than none.** The
core's rules are unit-tested against a canned transport and a supplied clock.

The account screen's **read** is ✅ because it has been driven against the **production** service
(2026-09-11), through `signin_probe`: a real sign-in carrying all five `mailcal:*` scopes, and the
whole record parsed back, including the `actions` a screen would draw and `checkoutAvailable: true`.

The Apple store layer is driven against Xcode's own **simulated** store
(`clients/apple/Scripts/test-storekit.sh`, in the gate): the catalogue resolves, a bought purchase
is reported with its identifier and a seconds timestamp, reading the outstanding set finishes
nothing, finishing takes only what it was named, and one held for approval is not outstanding.

⚠️ That stops at the round trip. A locally simulated transaction is signed by a local test
certificate, so Apple's App Store Server API has never heard of it and the service answers
`unknown_purchase`; proving the rest needs products in App Store Connect and a sandbox tester.
**It is also why `unknown_purchase` is retried rather than concluded from**: under that
configuration it is the ordinary answer about a purchase that is perfectly real.

Play has no equivalent, and needs none: there is no local simulator, and the real store is
reachable from a developer machine as soon as the products exist in the console and **any** build
has reached a track. A licence tester then buys on a test card that is never charged, and a period
runs in minutes rather than a year, which is how the Android layer above was exercised.

Everything still 🚧 compiles and is covered by the suites that can reach it, and has **never run
against a real store**. **No money has moved at either store**: Android's purchase was a licence
tester's, on a card that is never charged, and Apple's was a sandbox one. Allodia's own checkout is
the one shop that has taken a payment, from Windows, and it was **a real charge on a real card**,
not a test mode standing in for one. That is the first money to move anywhere in this contract, and
it is why the four writes are no longer the least proven half of this page.

## Known gaps

- **The `foss` checkout has never been paid on**, though the shop behind it now has. Windows
  reaches the same hosted page through the same core call and a payment has completed there, so
  what is unproven on Android is its own launch path rather than the checkout: `WebBillingProvider`
  resolves the two prices and hands them to a Custom Tab, and nobody has followed that one through
  to a payment. Its price formatting is the one part with no test at all, because it needs a device
  locale.
- **The flavour is Android's only**, and it is recorded here rather than in a contract of its own.
  If it grows past purchasing, and the likeliest way is
  [`../docs/updates.md`](../docs/updates.md), because an F-Droid build is updated by F-Droid and a
  Play build by Play, it earns a doc beside
  [`../docs/windows-channels.md`](../docs/windows-channels.md), which is the same shape of problem.
- **Apple's store half has still only met a simulated store and a sandbox**, where Android's has
  now met the real one. A deployment with no billing configured answers `503 unavailable`, which
  the ledger treats as an outage and retries.
- ⚠️ **A StoreKit purchase sheet that never answers suspends its caller for good**, the same hole
  the Play listener has below, and observed rather than reasoned about: a sandbox sign-in that
  could not complete left `Product.purchase()` awaiting with nothing logged and nothing returned.
  Apple's screen holds the wait to the screen that started it, so closing the subscription section
  cancels it and every button comes back; the purchase itself is unaffected, because an unfinished
  transaction is re-offered at every launch and the updates listener starts a pass for it. What is
  still missing is any bound on the wait, so a person who stays on the screen waits forever, and
  choosing between a timeout and something better is a decision rather than an oversight.
- ⚠️ **Switching period does a different thing depending on who sold it, and nothing says so.**
  Through this API, on Allodia's own subscription, it changes the next charge and moves no money
  today. On the App Store it is Apple's own upgrade: the two products sit at different levels in
  the subscription group, so monthly to yearly takes the money at once and refunds the unused part
  of the month, and yearly to monthly waits for the renewal. Both are defensible and the levels are
  a deliberate choice, but somebody who has read one of them will be surprised by the other, and
  the copy for either switch has to be the store's rather than one sentence reused. Play will have
  its own answer again.
- **The four writes reach every screen, and Apple draws three of them on purpose.** Android draws
  all four: checkout is what the `foss` buy buttons already call, and cancel, switch period and
  resubscribe sit under the paid state, each drawn only where `actions` permits. Windows and Linux
  draw the same four, for the same reason and off the same `actions`; on both, checkout is the buy
  buttons themselves, there being no store on either to sell through. macOS and iOS draw the three
  that change a subscription and **never the checkout**, because starting one sends somebody to a
  payment page outside the app, which is the external purchase link this build carries no
  entitlement for; buying on Apple is the App Store's. ⚠️ **None of the three has been run from an
  Apple client against a real subscription**, where Android and Windows have each driven all four.
- ⚠️ **A restart can answer with a payment page, and on Apple that answer has nowhere to go.**
  `resubscribe` comes back either reactivated on the authorisation already held or with a URL, and
  the button is drawn only on a cancellation still inside its period, which is the reactivating
  shape. The other answer is not refused by anything, so if the service ever sends it there, the
  Apple screen says nothing and re-reads rather than opening a link it may not draw. Windows and
  Linux open it, correctly for platforms no store distributes.
- **The Linux drawing has been run against both shapes** (2026-09-20, production). Against an
  account billed by **Google Play**, the half with no writes on it: the store's page offered, and
  cancel, switch and restart correctly absent. Against a subscription of **Allodia's own**:
  cancelled and restarted, ending where it began, with the renewal date unmoved and the button set
  following `actions` rather than the card, the restart button appearing on the cancellation and
  going again on the restart. ⚠️ Starting a checkout and switching period are still unrun here, and
  a refusal, reachable only by racing the service, remains the least proven line on the screen.
- **A refusal now has words of its own.** `AllodiaSubscriptionRefusal` reaches a client as a code,
  as this contract requires, and rendering the **exception** instead puts a generated field name on
  screen: UniFFI builds one from the variant's own fields, so a refusal read `reason=NOT_SWITCHABLE`
  and everything else, an unreachable service included, read as a sentence with nothing after its
  colon or, on Windows, as the exception's own type name. All three clients that draw the writes now
  switch on the code, with five sentences in all seven locales and one more for a reason none of
  them can explain.
- ⚠️ **A card reads once and nothing tells it the answer changed.** Every client asks when the
  card appears and not again, so it goes stale the moment the subscription moves anywhere else, and
  the checkout route makes that the ordinary case rather than an edge one: the payment finishes in a
  **browser**, and the person comes back to a card still offering to sell them what they have just
  bought. The same staleness follows buying on a phone, subscribing on the website, a store
  cancelling at renewal and a charge failing, none of which this app is present for, which is why
  the remedy is the app being looked at again rather than anything a checkout could hand back.
  The two desktops re-read when their Settings window comes back to the front, which is what
  returning from the payment page looks like on both, and each ignores it until the section has
  answered once, so the round trip is spent on the page that draws one. Apple and Android start
  their read once per screen, neither re-runs on resume, and both still owe it.
- ⚠️ **The manage-at-the-store button is unreachable in the two states it was written for.**
  Every card draws the paid half only when `entitled` is true, and on hold and paused both grant
  nothing, so `entitled` is false and the button goes with the rest of that half. Those are exactly
  the two states the rule about which stores are manageable exists to keep it for: a card that
  failed is replaced at the store and a pause is lifted there, so the person who most needs the
  route is the one who has none. It is the same shape on all four clients, which is why it is
  recorded here rather than fixed on whichever one noticed.
- ⚠️ **The four clients that format a service date do not agree on which day it is.** The service
  sends an offset-bearing instant, and a period ending at midnight `+02:00` is the previous day in
  UTC. Android restates it in UTC and Apple in the reader's own zone, so the two already differ;
  Windows repeats the day the service wrote, which is the only one that cannot disagree with its
  own fallback, since a string it fails to parse is drawn as its leading ten characters. Linux
  takes Android's frame for an instant and Windows's for the plain calendar day the service also
  sends, because reading one of those as a local instant loses a day everywhere east of Greenwich.
  One rule should win, and the cheapest place to settle it is the service saying which frame it
  means.
- **All four writes have now been run, and Allodia's own checkout has been paid through.** A
  Windows client started a monthly checkout (2026-09-19), the hosted page took **a real payment on a
  real card**, and the subscription came back active and renewing a month later: the first money to
  move anywhere in this contract, and the end of the longest-standing gap on this page. The other three
  were driven against a **real** subscription of Allodia's own, from **two** clients: Android on 2026-09-18 and Windows on 2026-09-19, both against
  production, each switching period, cancelling, restarting and ending where it began. Two answers
  that had never been seen came back the first time and held the second: the period end does not
  move when the period changes, and a restart reactivates on the authorisation already held rather
  than handing back a payment page, so nobody re-enters a card to undo a cancellation. The Windows
  run also showed the button set following `actions` rather than the card: cancelling withdrew the
  switch button and produced the restart one, and restarting put it back. The free half was driven
  in the same session on a second account: both periods drawn in the core's order, priced from the
  service's own minor units at the same figures the switch above reported.
- **The store-billed shape is now drawn on a desktop client too**, which is the half `actions`
  exists for. A Windows client signed in to an account **Google Play** bills drew the store's name,
  its renewal date and its manage page, offered it once, and offered none of cancel, switch or
  restart, because the service refuses all three on a subscription it does not bill. Nothing in the
  card works that out. The same account, minutes earlier with its licence-tester period lapsed, drew
  the free state and both buy buttons instead, so both sides of the same passthrough were seen on
  one machine within the hour.
- ⚠️ **A switch reports an amount and no currency.** `AllodiaIntervalChange` carries minor units
  alone, so a client prices it from `prices.currency`, which is today's list currency rather than
  the one this subscriber was charged in. The two differ only for somebody whose billing currency
  has since changed, which the service does not currently do, so it is a latent wrong rather than a
  present one.
- ⚠️ **The account web page draws a store's heading and its manage link once per subscription
  row**, which is the rule above going the other way outside this repository. An account with three
  live Play rows, two cancelled and one renewing, drew three "Billed through Google Play" headings
  and three manage links at the same page (2026-09-19). Three headings from one store also read as
  being charged three times, which is what `duplicateBilling` is reserved for and would then be
  believable and wrong. The page belongs to the account service's own repository, so it cannot be
  fixed here; it is recorded because the rule it breaks is this contract's.
- **The invoice history is not modelled.** `GET /subscription` also returns each payment and what
  has been refunded of it; nothing draws that yet, and serde ignores what nothing asked for, so
  adding it later needs no service change.
- **Neither store's link-out has been applied for**, so the shape above is written against
  programmes this app is not enrolled in.
- ⚠️ **A store the service has not sent before would read as the App Store.** `source` in the
  account-screen read is a closed enum of `apple` and `google`, so a third cannot arrive without a
  change at both ends; until then the parser coerces rather than carrying an `Unknown` arm through
  `Store`, which also answers which product to ask for and whether a purchase needs attaching. The
  `manageUrl` is unaffected, because the service sends it.
- **The four writes send no idempotency key**, unlike attaching a store purchase. `start_checkout`
  is the one that shows: a lost response retried by hand leaves a second `pending_first_payment`
  subscription at the service. Worth a key before they are first driven for real.
- ⚠️ **What StoreKit does with a purchase held for Ask to Buy is not pinned down.** A test for it
  reported the held purchase as outstanding on roughly one run in three, so the suite states
  nothing about it. If it is surfaced, the purchase is handed over, the service answers
  `unknown_purchase` because Apple has no completed transaction for it, and the ledger retries
  until the approval lands: no money is lost, but the "not gone through" warning could reach
  somebody who is only waiting for a parent to approve, which is the wrong thing to tell them.
- ⚠️ **An unrelated Play update can steal the flow a purchase is awaiting.**
  `PurchasesUpdatedListener` is told about every update, not only the one `launchBillingFlow`
  started, so a renewal or a purchase made on another device arriving while the sheet is open
  completes the awaited flow with **that** result. No money is lost, because the person's own
  purchase then arrives through the same listener and starts a pass, but `buy` reports a failure
  for a purchase that succeeded. Play issues no correlation token for a launched flow, so the fix
  is a choice (match on the product bought, or time the wait out) rather than an oversight. The
  same listener is why a flow Play never answers leaves its caller suspended.
- ⚠️ **A pass says how it ended in counts, never in ids.** `attaching N` was the only line the
  redemption pass wrote, so a pass that granted nothing and one that granted everything read
  identically, and the ending that costs somebody an explanation was invisible in a support log.
  It now says how many settled, how many belonged to another account and how many are coming round
  again. Ids stay out: a purchase id is the store's and names the person's transaction.
- ⚠️ **A refused token reads as an unreachable service on the account screen.**
  `Error::Unauthorized` reaches a client as `Unreachable`, which is the collapse
  [`entitlement.md`](entitlement.md) forbids for grant health: a revoked grant and an outage are
  different answers. `allodia_grant_health` carries the truth, so a client reading both is not
  misled, and one reading only the screen's own error is. Whether the fix is a new variant or the
  health channel staying the single place that says it is a contract decision.
- **On Play, a settled purchase is attached again on every pass.** `Ledger::apply` drops a linked
  purchase, and `queryPurchasesAsync` reports an acknowledged subscription for its whole life, so
  the next `sync` takes it up again as new. It is idempotent at the service and costs one request,
  and the alternative is remembering settled identifiers, which on Apple would mean forgetting a
  `finish` that never happened and leaving StoreKit re-delivering it forever. Worth revisiting only
  if the request itself becomes a cost.
- **A grant stored before the account id was recorded carries none**, and no launch fetches one,
  so such a device buys untagged until its next sign-in. Untagged is what every purchase was
  before, so nothing regresses; what it loses is the repair path below. Buying with a guessed id
  would be worse than buying without one, because it would attribute somebody's money to an
  account that is not theirs.
