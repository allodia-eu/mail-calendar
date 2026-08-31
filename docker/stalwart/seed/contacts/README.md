# Contact fixtures

vCard 4.0 cards seeded into the harness over CardDAV, so the clients' Contacts list has
deterministic content to render and assert against.

They exist to pin the two halves of the dedup rule (`docs/contacts.md`), which is why the
set looks redundant at a glance:

- **`shared-*.vcf` are the same person, in two different accounts** (alice and bob) at one
  address. The engine joins on shared canonical email, so these must render as **one** row
  marked as being in 2 accounts. A merge that silently showed one card, or that showed two
  rows, are both failures this catches.
- **`namesake-a.vcf` / `namesake-b.vcf` share a name and nothing else.** Names must never
  join (two people commonly share one), so these must stay **two** rows. Without this the
  first fixture alone would pass just as well against an implementation that merged on name.
- **Some cards carry a `PHOTO` and some deliberately do not.** A mailbox where everyone has a
  face proves only half of `docs/avatars.md`: the monogram is the common case, and the two have
  to be visible together. `namesake-a` and `namesake-b` split on exactly this (same name, one
  face, one monogram), and `numeric` (an organisation) stays a monogram too.
- **`bestuur.vcf` and `bob.vcf` are the cards whose addresses match seeded mail senders**
  (`board@test.local`, `bob@test.local`). That pairing is what makes a face reach a *mail row*
  at all: the avatar path only runs for a sender the user has a card for, so a card nobody
  writes to exercises the contacts list and nothing else.
- **`shared-alice` and `shared-bob` carry the same photo**, because they are the same person in
  two accounts. The merged row must show one face, not pick one of two.
- **The photos live in `photos/` and the seeder inlines them** as `PHOTO;…;base64,…` at PUT
  time (`seed.sh`, `card_body`). Inline is the shape a real vCard carries and the one that
  reaches the CardDAV *and* JMAP adapters: the same card served two ways. They are kept out of
  the `.vcf` files because a base64 blob is 200 unreadable lines in a fixture whose point is
  that a human can read it, and git stores the JPEG far better than its expansion. They are
  240x240, the size Microsoft Graph serves, so what the app decodes here is what it decodes in
  life.

  **Check a photo actually landed** before believing an app that shows monograms: the seeder is a
  `#!/bin/sh` script run by dash inside the container, so a bash-only test in it fails *silently*
  and every card is seeded without its face:

  ```sh
  curl -su alice@test.local:harness-alice-pw     http://127.0.0.1:28080/dav/card/alice@test.local/default/bob.vcf | grep -c PHOTO
  ```

  Source: the *Untitled UI* avatars library, used under the licence Allodia holds for it.
  The six were **chosen by looking**, for a set that is visibly diverse in the way a real
  address book is: two women with markedly different skin tones among them, and a range
  across the rest. A showcase built from one kind of face is its own kind of wrong. They are
  stock portraits and are not chosen to "match" any contact's name.
- The rest are ordinary cards giving the list realistic shape: with and without a phone,
  with and without an organisation, and one whose name starts with a digit (it files under
  `#`, not its own section).
