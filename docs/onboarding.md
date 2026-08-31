<!--
SPDX-FileCopyrightText: 2026 Allodia
SPDX-License-Identifier: GPL-3.0-only
-->

# First run: the screen that adds the first mail account

Every person who opens the app reaches one screen they cannot avoid, and it is the one where they
add their first mail account. This file decides what is on it, in what order, and what happens when
half of it is not there. It binds every platform that ships the screen.

Two things it deliberately does not decide, because they are decided once elsewhere: what an
Allodia account is and what it stores ([`privacy-policy.md`](privacy-policy.md) §3, the published
contract and the only place that describes it), and how server settings are found from an address
([`account-autodetect.md`](account-autodetect.md)).

## The order the screen is in

1. **The recommendation.** One card, marked as recommended, offering an Allodia account. Its
   subtitle says what the account does: it keeps your accounts the same on your phone and your
   computer.
2. **The way back for someone who already has one.** A single line under the card, not a second
   card of equal weight.
3. **A divider that names what follows**: connecting a mail account directly.
4. **The direct route**, which is the existing email-address field and nothing else. The provider
   is detected from the address; nobody is asked to pick one.

The order is the rule. A client lays it out as its platform expects, but may not promote the direct
route above the card, and may not demote it into a menu, a second screen or an overflow.

## The four rules

- **Skipping is one action, and it is on this screen.** Typing an address and continuing is the
  whole of declining. There is no confirmation, no second ask later, and no part of the app that
  stays locked because the card was ignored.
- **No card without a registration.** A build carrying no `MAILCAL_ALLODIA_CLIENT_ID` has no
  Allodia sign-in at all ([`BUILDING.md`](../BUILDING.md)), so items 1 to 3 are absent together and
  the screen is the direct route alone. The divider goes with the card: a lone "or connect
  directly" heading under nothing is the tell that a client gated the wrong thing.
  `allodia_sign_in_available()` is the single question, and no client reads a credential itself.
- **The copy may not out-run the [README](../README.md) capability matrix.** Today that means
  **phone and desktop**. There is no web client, so no card says "and web", in any locale.
- **The card claims sync and nothing else.** Not storage, not backup, not "your mail everywhere":
  what travels is the account list, never the mail and never a password. The words on the card and
  the words in the policy describe the same thing, or one of them is wrong.

## Nobody may be stranded on it

This is the one screen a person cannot skip, so the busy state a sign-in puts it in **must always
have a way out**. Signing in leaves for a browser, and what happens there is not this app's to
control: the page can fail, the service behind it can be down, the redirect can never arrive. What
is left behind is a spinner on a window that rejects a close.

- **It retires the attempt, not just the spinner.** A metadata read or an exchange still running
  must not open a browser, or store a grant, for a sign-in somebody has escaped.
- **Escaping is not a failure.** The card goes back to the offer it started from and says nothing:
  a person who gave up on a sign-in does not need an error about it.
- **The pass that follows a sign-in is not this state.** It is a bounded network call rather than a
  wait on somebody in another application, so it draws no way back.

**When it is drawn is the client's choice, within one bound.** Drawing it at once is always
correct. A client may instead hold it back until the hop has outlasted a threshold (no longer than
ten seconds) because an ordinary hop puts the browser in front of the person within one, and a
button drawn for that one is noise on every sign-in that ever works. Windows draws it at once,
beside the Settings panel's own; macOS, iOS, Android and Linux hold it for eight seconds.

A platform whose own idiom already provides the escape has met the rule by that route: on Android
returning to the app clears a dismissed browser, which covers everything after the browser opens
but nothing before it. The threshold is what covers the rest.

## Where it sits relative to the welcome screen

The analytics consent screen comes first and is unchanged ([`analytics.md`](analytics.md)): it is
about what leaves the device, it is answered before anything is connected, and its answer is not
this screen's business. This screen follows it. Someone who declined analytics still sees this
card; the two choices are unrelated, and neither is evidence about the other.

## Adding the second account

**The card is first-account-only. The offers are not.** They are two different things and were
gated as one, which is the bug this section used to describe as a rule.

The **card** is a pitch, and it is made once: someone who has already decided about an Allodia
account is not asked again, so Settings → Accounts → Add opens without it.

The **offers** are not a pitch: they are accounts the person already has. Somebody with three
linked accounts sets one up and the screen closes with two still to go; gating those with the card
left them reachable only from a section on the Settings page, while the "Add account…" button
beside it asked for an address they could have picked from a list. So **any** add screen shows the
offers still outstanding, and the divider goes with whatever is above it: nothing outstanding is
the direct route alone, with no heading over it.

The empty-answer message is part of the card, not the offers: "no mail accounts yet" is a sentence
about a new Allodia account, and somebody adding their second mail account has one.

## Accessibility

The card is one control rather than a heading beside a button, so a screen reader announces the
offer and its action together. Its accessible label carries the action and not the recommendation
marker: "Create an Allodia account", with the subtitle as the description.

On Linux both halves come from the row's own title and subtitle, through the `labelled-by` and
`described-by` relations an `AdwActionRow` publishes: an explicit label *or* description is
silently ignored, because a relation beats the matching property
([`AGENTS.md`](../AGENTS.md) → Client conventions). A check reads those relations: the row's
AT-SPI `description` field is empty on a row that is working correctly.

## Per-platform status

Legend: ✅ shipped · 🚧 in progress · ⬜ planned · n/a not applicable.

| | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| The screen, in the order above | ✅ | ✅ | ✅ | ✅ | ✅ |
| The absent-registration case, verified | ✅ | ✅ | ✅ | ✅ | ✅ |
| The card's accessible label | ✅ | ✅ | ✅ | ✅ | ✅ |
| A way out of a hop that does not come back | ✅ | ✅ | ✅ | ✅ | ✅ |
| An empty answer says so, in words | 🚧 | ✅ | 🚧 | ✅ | ✅ |
| Offers on a later add, not just the first | 🚧 | 🚧 | 🚧 | ✅ | ✅ |
| Set up takes the record's own route | 🚧 | 🚧 | 🚧 | ✅ | ✅ |

## What signing in here does

The card is not a detour. Signing in on this screen runs a sync pass at once, and what the person's
other devices hold replaces the card: each account they already have becomes a row with the address
and a **Set up** button.

**Set up takes the route the record names**, rather than re-deriving one from the address. The
record already says which provider, which hosts, which ports and which security, because a device
that set the account up wrote it down, and re-deriving that is the one thing account sync exists
to avoid. It spends a round trip re-learning what is in front of us, and for an address whose
provider cannot be found from its domain (a hosted IMAP domain publishing no autoconfig) it learns
*less*, dropping the person onto the manual form for an account another device set up without
trouble. One core call decides it (`setup_from_offer`), so no client re-implements the routing.

Two things that do not change. The **password is still entered on this device**, because no
password ever travels. And a record naming no server falls back to detection rather than presenting
an empty form: detection finds the same server the other device found.

An offer's settings are **trusted by construction**. `is_trusted` gates the approval an
*undiscovered* config needs ([`account-autodetect.md`](account-autodetect.md)): the case where a
non-HTTPS hop could have chosen the server a password is about to be sent to. Nothing here was
discovered: the settings were approved on the person's own device and arrived over HTTPS from their
own account, so asking again asks them to re-answer a question they have already answered.

**An empty answer is still an answer, and it has to be given in words.** Somebody signing in on a
first device (or on an account nobody has added a mail account to yet) gets no offers back. Left
to itself that state draws a divider over an address field with the card gone, which reads as the
sign-in having failed: the person finished a sign-in and the screen looks like it lost it. So the
card is replaced by a short statement naming what was found (nothing) and what to do next (add the
first account below), and saying that what they add will then be offered on their other devices,
which is the whole reason they signed in.

Two shapes it must not take. A **heading over an empty list** ("From your other devices", with
nothing under it) is worse than the silence it replaces. And it is **not an error**: nothing failed,
so no error styling, no retry, and no apology.

**Only a pass that answered may say this.** "We have not looked" and "there is nothing" are
different answers, and a pass that failed (the service down, the network gone) has given neither.
A client that flattens the two (`report?.offers ?? []` and its equivalents) tells somebody their
account is empty on the strength of a question that was never answered, and invites them to fix it
by typing an address. Until a pass has answered, this state draws nothing at all.

## The trap this screen sits in

**Nothing tells the card which window it is on.** A client that fills the card's state where it
builds the first-account screen has written the wrong thing on every platform where the analytics
consent screen comes first: that screen is opened later, from the consent answer, by code that
knows nothing about registrations. What comes up is the direct route alone, which is exactly what
a build with no registration is supposed to look like, so nothing reads as broken.

Fill it at boot, from `allodia_sign_in_available()`, and let whichever window opens pick it up.

## Known gaps

- **What Apple has on screen, it has by hand.** The way out and the absent-registration case are
  both there and both were seen: the second on a build with the registration removed, macOS by eye
  and iOS against its accessibility tree. There is no Apple UI-test target, so what the suite pins
  is the threshold and that an escaped hop ends as a cancellation rather than an error; that either
  is ever *drawn* is invisible to every gate.
- **The empty answer is written on all five and seen on three.** Android, Linux and iOS have been
  run and looked at; macOS and Windows are written against the same contract and compile, so their
  cells stay 🚧 until somebody reaches the state on them.
- **The later-add offers and the record's own route are written on all five and built on two.**
  Android and Linux are compiled and asserted; Apple and Windows are written against the same
  contract but were not built on the host this was written on. Reaching either needs an Allodia
  account with **two** linked mail accounts, which no client can fake.
- The **absent-registration case** is asserted by the Android, Linux and Windows suites
  (`OnboardingAllodiaCardTest`, `setup_onboarding_tests`, `uitests/Onboarding.Tests.ps1`, which
  derives the expectation from the build's own registration, and so states the rule for both build
  shapes). It is the rule that fails while rendering perfectly (a lone "or connect directly"
  heading under nothing), so Apple still owes it a check of its own.
- **What carries the "not answered yet" distinction is the type, and only at the render site.**
  Each client's card takes an optional list, so flattening it there is a compile error on three of
  the four (Kotlin, Swift, Rust). What no test covers on any client is the **wiring** (the one
  line that reads `report?.offers` and could go back to `?? []`) because every suite builds the
  card directly rather than through the model. It is caught in review.
- **No client asserts the way out against a real hop.** Reaching that state opens a browser at the
  account service and no client has a hook that fakes it, so what the suites assert is that the
  control is drawn when the state says so; that the state is ever reached is verified by hand.
