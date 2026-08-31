# Store copy: the unbranded default

What a build says about itself in a software centre when no brand file overrides this one. It is
the listing twin of [`default.env`](default.env): always present, never Allodia's, and replaced
wholesale by a fork that ships its own product.

The **rules** are in [`../docs/store-listing.md`](../docs/store-listing.md): what may be claimed,
which locales move together, the stores' field limits, and which of the fields below the Linux
metadata generator reads. This file is only an answer to them.

**It is deliberately English-only and deliberately short.** The generator requires English and
treats every other locale as optional, falling back to the untagged paragraph, so a neutral build
loses nothing by not translating copy nobody has reviewed. And it makes no capability claim at all:
the anti-hype rule measures copy against the capability matrix per platform, and an
unbranded build is not one whose reach anybody has checked.

**It never names the app.** The name is injected ([`../docs/branding.md`](../docs/branding.md)) and
a software centre draws it directly above this text, so writing it here would be both a second
source for one fact and a word the reader has just read.

⚠️ **The short description comes before the shared body, and the order is load-bearing.** A `##`
section runs to the next `##`, so a `###` block placed after the body is read as a second fenced
block inside it. The scraper says so rather than guessing ("expected exactly 1"), but the failure
reads as a broken body when what moved was a heading.

---

### Google Play — Short description (≤80)

**English**
```
Mail and calendar over open standards, on servers you choose.
```

---

## Shared description — English

```
An email and calendar client that talks to the servers you already use: your own, or any provider, anywhere. It connects over open standards (JMAP, IMAP, SMTP, CalDAV and CardDAV), so nothing asks you to move your mailbox somewhere new and your mail stays where you put it.

Mail syncs straight between your device and your provider. Credentials are kept in the platform's own keystore, incoming HTML is sanitised before it is shown, and remote tracking images are blocked by default.
```
