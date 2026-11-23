# Privacy Policy: Allodia Mail & Calendar

**Version 3.0 · Effective: 2026-11-23**

Allodia Mail & Calendar is a mail and calendar app that runs on your device and connects to the
providers **you** choose. This policy says what stays on your device, what the app sends and to
whom, and what reaches us. By default nothing does: without an Allodia account, which is how the
app installs, we hold nothing about you.

## The short version

- **Your mail stays between your device and your provider.** We are not in that path and cannot
  read your mail, events or credentials. The one exception is a request you start through our
  writing assistant service, which passes through our servers and an EU router to an AI provider
  in the EU, and whose content is not kept (§5.2).
- **By default the app sends us nothing**: no telemetry, no crash reports, no identifiers, not even
  the fact that you installed it.
- **Three things can reach us, each only if you choose it:** an Allodia account (§4), requests to
  our writing assistant service (§5.2) and usage statistics (§6). A subscription you buy is kept
  with the account (§7).
- **An Allodia account never holds your mail**, a mailbox or a password: only who you are and the
  list of mail accounts it keeps in step across your devices.
- **AI acts only when you ask.** An assistant on your computer reaches only the accounts you tick,
  and the writing assistant sends only when you press Learn or Draft a reply (§5).
- **We never see your card, sell data, show ads, profile you, or use third-party analytics or
  tracking.** What we receive stays in the EU, with one exception we name (§10).
- **Your rights are the GDPR's**, and most of them you can exercise in the app (§12).

## 1. Who we are

Allodia, registered with the Dutch Chamber of Commerce (KvK) under no. **56789823**, Burg. van
Fenemaplein 2 a1701, 2042TA Zandvoort, the Netherlands. For anything in this policy:
**info@allodia.eu**.

We are the controller only for what reaches us: an Allodia account (§4), requests to our writing
assistant service (§5.2), usage statistics (§6), a subscription (§7), messages you send us (§8)
and the website (§9). What the app processes on your device we never receive, so for that we are
neither controller nor processor.

## 2. Your device

**What is kept there.** Your mail, events, contacts, attachments, account settings, signatures,
writing styles and a diagnostic log. None of it is uploaded to Allodia; there is no cloud copy of
ours.

**How it is protected.**

- Passwords, sign-in tokens and keys live in the platform keystore (Keychain, Windows Credential
  Manager, Android Keystore, or the Linux system keyring through Secret Service), never in the
  app's database or log. The message store is protected by your device's encryption at rest.
- Remote images are blocked by default, so a tracking pixel cannot report that you opened a
  message. You can load them one message at a time.
- Message HTML is sanitised and scripts never run, so a message cannot execute code, navigate the
  app, contact a server or read anything on your device.
- The diagnostic log holds counts, durations and technical events, never message content,
  subjects, addresses, credentials or signatures. It is capped at a few megabytes and leaves your
  device only if you send it to us (§8).
- Notifications are made on your device, never by a service of ours. Whether a preview (sender,
  subject, opening line) shows on the lock screen follows your system settings.

**How long.** Until you delete it, the app's data or the app.

## 3. Your mail, calendar and contact providers

**What goes where.** The app connects only to the providers you add, over IMAP, SMTP, JMAP,
CalDAV, CardDAV or a provider's own API (such as Microsoft 365 or Google). They process your data
under their own terms, as with any mail app. We are not part of that relationship, and nothing of
it is sent to Allodia.

**Signing in.** OAuth sign-in happens in your system browser, directly with the provider: we run no
sign-in server and never see your password. Where a JMAP server publishes its own sign-in, the app
asks only your own mail server, involves only your **domain** (never your full address) until you
reach the provider's page, and asks only for your mail, your calendars and staying signed in, never
your contacts or anything else. A server that publishes none is sent nothing.

**Finding your settings**, when you ask for it, sends only your **domain**, never your full
address: to your own and your provider's domain, to DNS, and to the Thunderbird project's public
autoconfig database (`autoconfig.thunderbird.net`, run by MZLA/Mozilla). Everything is tried over
HTTPS; a source reached otherwise is shown as untrusted and used only once you approve it. No
password is involved, Allodia receives nothing, and "Set up manually" skips the lookup.

**Contacts.** An account's address book is read from its provider and kept on your device; your
phone's own contacts are never read. The app also remembers who you sent mail to, from your Sent
folder. A contact you add or edit is written back to its address book only when you save, with
nothing you did not type, and the app never deletes one. Where a provider asks on its sign-in
screen for your contacts or your organisation's directory, declining leaves the rest of the account
working; sender pictures come from that provider.

**Shared mailboxes and calendars** are opened with the sign-in of the account they were shared
with and kept on your device like your own. With Microsoft, access is asked for only while that
option is ticked, and an address you type goes to Microsoft alone, to check that you can open it.
Nothing about them reaches Allodia, even with an Allodia account, and switching one off removes it
from your device.

**Your sender name** is also saved to the account's own settings where the provider allows it: the
only setting the app changes at your provider, and the app reads no other.

**What you start yourself** goes to parties you choose, never via Allodia:

- Mail and invitations you send go through your provider. A signature leaves your device only
  inside them, its images embedded, so nothing about the reading is reported back.
- Remote images you load come from their servers (often the sender's), which see your IP address.
- Links and attachments open in your browser or the app your system chooses; only `http`, `https`,
  `mailto` and `tel` links are handed on, and attachments are never rendered inside the app.
- A video meeting you add to an event is made where you choose. Any service but the calendar's own
  receives only the event's title and times.

**How long.** On your device, as §2 says; at your provider, under its terms.

## 4. Your Allodia account (optional)

**Whether.** The app recommends one when you add your first mail account, because it keeps your
accounts in step across your devices. Connecting without one is on the same screen, you are not
asked again, and declining costs you no part of the app. You can sign in later in **Settings →
Allodia account**.

**What we hold.** Your email address, your display name if you have one, and the sign-in. Once you
are signed in on a device, the list of mail accounts on it: for each, the email address, server
names and ports, user name and connection settings. If your account includes the writing
assistant, also that it does and, once you use our service for it, the balance of credits its
requests draw on and a usage record of each request, held under your account's identifier and
never your email address (§5.2). If you buy a subscription, what §7 lists. Nothing else, and the app
makes no other request to us.

**What we never hold.** Your mail, a mailbox, or a password or token for your provider: those stay
in your keystore, so a new device fills in your accounts and still asks for each password. A token
issued for the account cannot reach your provider.

**How it is stored.** On Allodia's own servers in the EU. The account list is encrypted there, so a
copy of our database alone does not reveal it, but with a key our servers hold: it is not
end-to-end encrypted, and we can read it. Your device keeps the account's sign-in token in its
keystore.

**Signing in with Apple or Google** is your choice, and nothing pushes you to it. That company
then learns that you have an Allodia account and gives us the address and name it releases;
signing in directly involves no third party.

**Legal basis.** Performance of our contract with you (GDPR Art. 6(1)(b)).

**How long, and how to stop.** Until the account is deleted. Signing out in Settings → Allodia
account erases the sign-in from that device; your mail accounts stay on it, and the list stays with
your account. **Settings → Allodia account → Delete account** deletes the account and what we hold
in it, apart from the writing assistant's records, which are erased on request (§5.2): it opens your
account page, also at **https://mailcal.allodia.eu/account** on any device without the app, where
you sign in and confirm. Or write to **info@allodia.eu**.

## 5. AI

### 5.1 An assistant on your computer

**What.** Over the Model Context Protocol (MCP), an AI assistant on your own computer can read and
act on the mail of the accounts you allow. It is available on macOS and Windows, and off until you
switch it on in **Settings → Advanced**.

**Where it goes.** Over a local channel that only your own operating-system user can reach: no
network port, no server of ours, nothing to Allodia. We do not know whether it is on, which
assistant you use or what you ask.

**What it can do.** Switching it on exposes no mailbox: each account is a separate tick, and
unticking takes effect at once. For a ticked account it can list, search and read messages, mark
them read, flag, archive, move to Trash, mark as spam, and open a prefilled draft for you to send;
it can never delete permanently. Sending is a separate switch, off by default, and with it on the
app still refuses recipients you have never emailed, unless you switch that off too.

**What to weigh.** What the assistant reads, its own provider may receive under its own policy: a
cloud assistant takes message content off your device when you ask it to read. That is your
dispatch, to a party you chose.

### 5.2 Writing style and drafted replies

**What.** The app can learn how you write from mail you have sent and draft a reply in that style,
for you to read, change and send; it never sends mail itself. In the versions we publish it is
early access, offered only while your Allodia account includes it.

**When.** Before learning, the app shows how many sent messages it found in the period you chose
and where they would go, and sends nothing until you press **Learn**. A reply is drafted only when
you press **Draft a reply**. Learning may fetch older sent mail from your own mail server, and
removes it from your device again once read.

**What a request carries.** To learn: the words you wrote in those messages, with quoted mail and
signatures cut away on your device, never their subjects or recipients. To draft: the message you
are answering, with its sender, date and the mail it quotes; your style's description and a few
passages of your own earlier mail; what you wrote in up to two of your latest messages to the same
address; any instruction you typed; and the text of your reply's signature. Never an attachment,
and never your address book.

**Where it goes.**

- **An AI endpoint of your own**, set in **Settings → Advanced**: any server that speaks the OpenAI
  chat interface, on your computer or chosen by you or your organisation. While one is set, every
  request goes there and nowhere else, Allodia is not in that path, and what happens there is up to
  whoever runs it. Its key stays in your keystore and goes only to the server it was given for.
- **Allodia's service**, when you have no endpoint of your own and are signed in to an Allodia
  account that includes it. The request goes to our servers in the EU, which pass it through
  **EU-router B.V.** (the Netherlands), a router that processes it on our behalf, to an AI provider
  in the EU, run by a European company, that our service chooses. Allodia keeps nothing of the
  request or the answer and logs no content. Of each request it keeps a usage record: the model
  and provider that answered, the tokens and credits used, how long it took, when, and the ids that
  tie the record to your balance of credits.

**Checked before anything is read.** Before the app reads your sent mail or the message you are
answering, and again before each request, it checks the destination: only a service processing in
the EU and run by a company based in the EU, or a model on your own computer, is admitted. Our
service passes requests only to such providers; an endpoint of your own is admitted only once you
have said so where you set it up. Otherwise nothing is read or sent.

**What stays on your device.** Your styles (each one's name, learned description, your notes, and
the passages of your mail it keeps as examples), stored nowhere else; your endpoint's address,
model and key; and only the identifiers of the replies a draft went into, sent or saved on this
device. A passage leaves the device only inside a request.

**A mark on replies it helped write.** So that no device learns your style from the model's words,
the app asks your provider to mark such a reply's copy in Sent and its draft in Drafts: a keyword
named `writing-assistant` on an IMAP or JMAP server that accepts one, a label on Gmail and a
category on Microsoft, named "Writing assistant", which you see there. Where a server keeps no
mark, only the device that sent or saved the reply knows. Nothing about the mark is sent to Allodia.

**Legal basis.** A request through our service: performance of our contract with you (GDPR Art.
6(1)(b)). The sender's personal data in the message you answer: legitimate interest, namely your
interest in replying to your own mail (Art. 6(1)(f)). With an endpoint of your own Allodia
processes nothing, so no legal basis of ours applies.

**How long, and how to stop.** A request's content is not kept. The balance of credits and the
usage records are kept while your Allodia account exists, and erased on request: write to
**info@allodia.eu**. Forgetting a style removes it from your device, and removing your endpoint
removes its address, model and key.

## 6. Usage statistics (optional)

**Whether.** At first start the app asks. The switch is **off**: nothing is stored or sent unless
you switch it on and confirm, and a no is remembered. The consent screen shows the data exactly as
it is sent: your device and account shape, how your settings are set, and an example of every kind
of event below, with every feature and protocol kind on its list. It is authoritative over this
summary.

**What is sent if you opt in.** Each row is one kind of data.

| Data | Value | Deliberately not |
|---|---|---|
| Install id | A random identifier, created only at the moment you opt in | Not derived from your device, account, or any hardware id |
| Platform + OS | e.g. `android`, OS major version only (`15`) | Never a build number |
| Device class | e.g. `iphone`, `mac-laptop`, `android-tablet` | Never a device model |
| App version + language | e.g. `1.4.0`, `nl` | Language only, never a full locale/region |
| Account shape | How many accounts (bucketed: `0`, `1`, `2`, `3–5`, `6+`) and which protocol kinds | Never which providers, hosts, or addresses |
| App opened | That the app was started | Nothing else about the launch |
| Setup and sync | Account setup started, completed or failed, and sync completed or failed, per protocol kind, with how long a sync took (bucketed) | Never why it failed, the server, or the account |
| Features used | That a feature on a fixed list was used: search, the calendar, creating an event, writing a new message, a reply or a forward, attaching a file, a background sync | Never what you did with it |
| Settings | How the settings on a fixed list are set: the message list threaded or flat, how a reply quotes, what each swipe direction does | Never anything you typed or configured as text |

**Never sent, by construction and not only by policy:** message content, subjects, senders,
recipients, email addresses, folder names, message counts, search queries, calendar titles or
event details, attachment names, server hostnames, device model strings. Every field is a label
from a fixed list or a bucket, and our server rejects anything else.

**Where.** Only to servers Allodia operates in the EU (Hetzner Online GmbH, Industriestraße 25,
91710 Gunzenhausen, Germany), never to a third-party analytics company. Your IP address is visible
when your device connects, but it is not stored and not linked to the data.

**Why.** Product decisions only, such as whether setup and sync succeed and which features are
used. The install id is stable while you stay opted in, so we treat this as pseudonymous personal
data, not as anonymous.

**Legal basis.** Your consent (GDPR Art. 6(1)(a); storing and reading on your device under the
national implementations of ePrivacy Art. 5(3)).

**How long, and how to stop.** At most 24 months, then deleted or reduced to aggregates that relate
to no install id. Withdraw in **Settings → Privacy**, one switch, as easy as opting in: the app
deletes the install id and consent record from your device and tells our server to erase
everything held under that id (Art. 17).

**What may be added.** From time to time we add labels within these kinds, such as a setting to
the settings row, under the same rules: a label from a fixed list, about how the app is used, never
content and never anything that identifies you. Each one appears on the consent screen and in this
table. The app asks for your consent again before it sends a new kind of data, a new row.

## 7. Paying, and the app stores

**The stores** you install and update from (Apple's App Store, Google Play, the Microsoft Store)
are independent controllers under their own policies. They give us only aggregated dashboards
(installs, active devices, OS versions, device models, crash statistics) that do not identify you,
fed according to your system's sharing settings. The app has no crash reporter and makes no update
checks of its own.

**A subscription** belongs to an Allodia account (§4), which carries it to every device you sign in
on, so buying from a store means signing in first. It adds no mail, no mailbox and no new access to
either. Wherever you buy, we never receive your card or bank details.

- **From us**, through **Mollie B.V.** (Amsterdam, the Netherlands) on its own hosted checkout. We
  keep the plan, its price, when the next payment is due and whether each payment succeeded, failed
  or was refunded, with Mollie's references for your customer record, your payment authorisation
  and each payment. We give Mollie your name and email address, to take the payment and invoice it.
- **From Apple or Google, in the app.** You pay that store, as an independent controller. From your
  device we receive only the store's reference for the purchase, which store, and when; nothing
  about your device. We ask the store whether the reference is genuine and what it entitles you to,
  and keep the store, the reference, the plan, whether it is active, cancelled, expired or refunded,
  what it runs until, whether it renews, and the price and currency charged. The app asks again
  from time to time, so that cancelling or letting it lapse takes effect here too.

**Legal basis.** Performance of our contract with you (GDPR Art. 6(1)(b)); for invoices and payment
records, our legal obligation to keep them (Art. 6(1)(c)).

**How long, and how to stop.** Until you delete your account, except invoices and payment records,
kept as Dutch tax law requires (seven years). Cancel a subscription from us on your account page,
and one from Apple or Google at that store, which also decides any refund. Cancelling stops the
next payment; it does not erase the record of payments already made.

## 8. When you contact us

What you send us by email (such as **info@allodia.eu**) or through the website's contact form (your
name and email address, an optional company name, your message and anything attached, which for
support may be the diagnostic log, holding no message content) is used only to answer you and
resolve the issue. Legal basis: our legitimate interest in answering the people who write to us
(Art. 6(1)(f)), or steps before a contract when you ask about buying (Art. 6(1)(b)). Our mailboxes
are hosted by **Soverin** in the Netherlands, and correspondence is deleted 24 months after the last
message.

## 9. The website

allodia.eu uses only the cookies and local storage it needs to work (language preference,
security, session handling), and no analytics or marketing cookies; if that ever changes, we will
ask first. It is hosted in the Netherlands and Germany. Its contact form is covered by §8.

## 10. What we never do

- We never sell or rent personal data, and never share it for advertising.
- We never use third-party analytics, tracking or advertising SDKs in the app.
- We never make automated decisions about you or profile you (GDPR Art. 22).
- We never train AI models on your mail. We have no access to it, except what a request to our
  writing assistant service carries while our servers pass it on, and we keep none of that (§5.2).
- We never transfer the personal data we hold outside the EU/EEA, with one exception we would
  rather name than bury: **checking a subscription bought from Apple or Google** means asking that
  store, and both are US companies (§7). The question carries the store's own reference for a
  transaction it already holds, and buying from us avoids it. Where your own provider is, and
  whether you sign in to an Allodia account with Apple or Google (§4), is your choice.

## 11. Retention at a glance

| Data | Kept | Where |
|---|---|---|
| Your mail, events, contacts, settings, signatures, writing styles, diagnostic log | Until you delete it, the app's data or the app | Your device |
| Your Allodia account and its mail-account list (§4) | Until the account is deleted | Allodia-operated servers, EU |
| The content of a request to our writing assistant service (§5.2) | Not kept | Passed through Allodia-operated servers, EU, and EU-router B.V., NL, to an AI provider in the EU |
| The writing assistant's balance of credits and usage records, once you use our service (§5.2) | While your Allodia account exists; erased on request | Allodia-operated servers, EU |
| Usage statistics, if you opt in (§6) | Until you withdraw, at most 24 months | Allodia-operated servers, EU |
| A subscription bought from us, and its payment history (§7) | Until your account is deleted, invoices excepted | Allodia-operated servers, EU; payments by Mollie, NL |
| A subscription bought from Apple or Google, and the store's reference (§7) | Until your account is deleted | Allodia-operated servers, EU |
| Invoices and payment records, if you buy from us (§7) | As Dutch tax law requires (7 years) | Allodia administration, EU |
| Support correspondence and contact-form messages (§8) | 24 months after the last message | Soverin, NL |

## 12. Your rights

Under the GDPR you can ask us for access to, correction, deletion or a copy (portability) of your
personal data, ask us to restrict processing, object to processing based on legitimate interest,
and withdraw any consent at any time, without affecting what was lawful before. You are not
required by law or contract to give us any personal data: the app works fully with every optional
choice left off.

The fastest route is usually the app, where your data already is. **Settings → Privacy** withdraws
and erases usage statistics; since an install id does not tell us who you are, we cannot find yours
by name or email, and that switch is the reliable way to have it erased.

For everything else, write to **info@allodia.eu**; we respond within a month (Art. 12(3)). You may
also complain to a data protection authority: ours is the Dutch **Autoriteit Persoonsgegevens**
(autoriteitpersoonsgegevens.nl), or you may use the one in your own EU/EEA country.

## 13. Security

Connections to your provider use the encrypted protocols it offers (TLS), and §2 says how your
device protects your data. The little we run server side is on EU infrastructure, with access
limited to those who need it and protected by multi-factor authentication and encryption at rest.

## 14. Changes to this policy

We update this policy, with its version and date, **before** a change to what data is handled
ships, and anything that would send a new kind of data asks you before it sends; for usage
statistics, that is a new row of the table in §6. The current version is always at
**https://allodia.eu/privacy/mail-calendar**.

## 15. Contact

Allodia · KvK 56789823 · Burg. van Fenemaplein 2 a1701, 2042TA Zandvoort, the Netherlands · **info@allodia.eu**
