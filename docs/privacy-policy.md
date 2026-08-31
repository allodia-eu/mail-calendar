# Privacy Policy: Allodia Mail & Calendar

**Version 2.2 · Effective: 2026-08-28**

Allodia Mail & Calendar is a mail and calendar app that runs on your device and connects to the
mail provider **you** choose. This policy explains, in plain language, what that means for your
data: what stays on your device (almost everything), what the app sends and to whom (by default,
nothing to us), and the two things you can choose to share with us (an Allodia account, and usage
statistics, both off unless you switch them on).

It has two halves for that reason. **Without an Allodia account**, which is the state the app
installs in, we hold nothing about you at all (§2). **With one**, which the app recommends when you
add your first mail account and which you can skip in a tap, we hold who you are and the list of
mail accounts you asked us to keep in step across your devices, and still never your mail (§3).

## The short version

- **Your mail never touches Allodia.** The app syncs directly between your device and your own
  mail provider. We are not in that path, we have no servers in that path, and we cannot read
  your mail, your events, or your credentials.
- **By default, the app sends us nothing.** No telemetry, no crash reports, no identifiers, not
  even the fact that you installed it.
- **An Allodia account is optional, and the app recommends one.** When you add your first mail
  account the app offers it as the easier route, because it is what keeps your accounts in step
  across your devices. Connecting your mail directly is on the same screen, and costs you nothing.
  An account never carries your mail: it holds addresses and server names, not a mailbox (§3).
- **One thing the app offers to send, and you decide.** At first start the app asks whether you want
  to share usage statistics. The switch is off; the app shows you the exact data before you decide;
  saying no costs you nothing and is remembered. You can withdraw in one click at any time.
- **An AI assistant can be given access to your mail, and only if you say so.** It is off, it
  runs on your own computer, it reaches nothing until you tick an account, and Allodia is not in
  that path either (§7).
- **We never sell data, never show ads, never profile you, and never use third-party analytics
  or tracking services.** Anything we do receive stays on servers we ourselves operate in the EU.
- **Your rights are the GDPR's**, and most of them you can exercise directly in the app, because the
  data is already in your hands.

## 1. Who we are

Allodia ("Allodia", "we"),
registered with the Dutch Chamber of Commerce (KvK) under no. **56789823**, Kamerlingh Onnesweg 2, 3316GL Dordrecht, the Netherlands.

For anything in this policy: **info@allodia.eu**.

We are the "controller" only for the little that actually reaches us: an Allodia account if you
create one (§3), optional usage statistics (§6), messages you send us (§8), and the website
(§10). For everything the app processes on your device, we are neither controller nor processor,
because we never receive it (§2).

## 2. The default: everything stays on your device

The app stores your mail, calendar events, contacts, attachments, account settings, your email
signatures, and a local diagnostic log **on your device**. None of your content is uploaded to
Allodia. There is no cloud copy and no backend of ours holding it, and unless you choose to sign in
to an Allodia account (§3), we hold nothing about you whatsoever. Signing in adds exactly one thing
to that picture, your list of mail accounts, and §3 says precisely what is in it.

The only network connections the app makes by default are between your device and **the mail,
calendar and address-book providers you connect**, over open standards (IMAP, SMTP, JMAP, CalDAV,
CardDAV) or a provider's own API (e.g. Microsoft 365, or Google for Gmail + Google Calendar). Your provider processes your
mail under **its** terms and privacy policy, exactly as it would with any other mail app;
connecting it here changes nothing about that relationship, and does not add us to it.

Signing in to a provider that uses OAuth (e.g. Microsoft 365, Google, or a JMAP server such as
Fastmail) happens in your system browser, directly between you and the provider. Allodia operates
no sign-in server, never sees your password, and the resulting tokens are stored only in your
device's secure keystore.

For a **JMAP** account the app can offer this sign-in even for a provider we have never
integrated, by reading what your server itself publishes: it asks your server where its sign-in
service is and what permissions exist, then registers itself with that server as an app on your
device. Three things bound it. Those requests go **only to your own mail server**, never to
Allodia, and never to any third party. Only your **domain** is involved, never your full email
address, until you reach your provider's own sign-in page. And the app asks for the **narrowest**
permissions that let it do its job: your mail, your calendars, and permission to stay signed in.
Never your contacts, and never anything else your server may offer. If your server does not
publish this, nothing is sent and you simply sign in with a password or an API token as before.

**Your contacts.** If the account you connect has an address book, the app can read it (over
CardDAV or JMAP, from the same provider, using the same credentials), so you can look your
contacts up in the app and get suggestions while addressing a message. They are stored on your
device like your mail, they are never uploaded to Allodia, and the app does not read your phone's
own contacts. The app also remembers who you have **sent** mail to, from your own Sent folder on
your own server, to make those suggestions useful before you have added anyone. Contacts are
read-only in this version: the app does not create, change or delete anything in your address
book.

When you sign in with **Microsoft, Google, or a provider that uses a sign-in screen**, the app
now asks for access to your contacts as part of that sign-in, and where the provider has one, to
your organisation's directory. You see exactly what is being asked on the consent screen before
you agree. It uses them for the same things as any other address book (looking a contact up,
and suggesting an address), plus the small picture shown next to a sender, which it downloads
from your provider and keeps on your device. Nothing is uploaded to Allodia, and if you decline,
the rest of the account still works; you simply see no contacts and no pictures.

The consent screen will describe this as **full access to your contacts**, which includes
permission to change them. That is deliberate, and it is more than the app does: contact editing
is a feature we are building, and asking now means you consent once instead of being sent back
through sign-in when it arrives. Until then the read-only promise above holds exactly as written:
the app creates, changes and deletes nothing in your address book, and this policy will say so
until the day that changes.

**Your signatures.** A signature you write (its text, and any image you embed in it, such as a
logo) is stored in a file on your device alongside your other settings, and nowhere else. It is
never uploaded to Allodia. It leaves your device only where you would expect it to: inside a
message **you** send, to the recipients **you** address it to, through your own provider. An
embedded image travels as part of that message; it is not fetched from anywhere at the moment your
recipient reads it, so nothing about their reading can be reported back. Signature content is never
written to the diagnostic log.

Privacy protections that are built in, on every platform:

- **Remote images in messages are blocked by default.** Tracking pixels are remote images; a
  message cannot report that you opened it. You can load images per message, and the choice
  resets on the next message (§5).
- **Message HTML is sanitized and scripts never run.** A message cannot execute code, navigate
  the app, phone home, or read anything on your device.
- **The diagnostic log never contains content.** It records counts, durations, and technical
  events, never message content, subjects, addresses, or credentials. It is capped at a few
  megabytes, and stays on your device unless you yourself choose to send it to us (§8).
- **New-mail notifications are generated on your device.** No notification service of ours sees
  your mail. Whether a preview (sender, subject) appears on your lock screen follows your OS
  notification settings.
- **Credentials live in the platform keystore** (Keychain, Windows Credential Manager, Android
  Keystore, or the Linux system keyring through Secret Service), never in the app's database. The
  message store is protected by your device's encryption at rest.

**Finding your settings when you add an account.** So you don't have to type server names, the
app can work them out from your email address when you ask it to. It looks in the standard
places for your provider's published settings: the mail, JMAP, and calendar autodiscovery
addresses on **your own email domain and your provider's domain**, a normal DNS lookup for your
provider's mail host, and the **Thunderbird project's public autoconfig database**
(`autoconfig.thunderbird.net`, run by MZLA/Mozilla), a shared directory of provider settings.
Two rules bound all of it: only your **domain** is ever sent, never your full email address;
and everything is attempted over HTTPS, and a settings source reached any other way is shown to
you as untrusted and is never used to connect until you approve it. No password is involved
(this happens before you sign in), Allodia receives none of it, and "Set up manually" skips the
lookup entirely.

No cloud service of ours sits between you and your provider. The app can hand your mail to an AI
assistant running on your own computer, but only if you switch that on and only for the accounts
you pick (see §7). If we add a feature that changes how data is handled, we will update this
policy **first**, and anything that would send new data will ask you before it sends.

## 3. If you sign in to an Allodia account (optional)

**You are offered one, and you can skip it.** Everything in §2 describes the app without an
account, and that is where you stay unless you decide otherwise. When you add your first mail
account the app recommends creating one, because that is what keeps your accounts in step across
your devices. Connecting your mail directly is on the same screen, nothing later asks you again,
and declining costs you no part of the app. The sign-in also lives in
**Settings → Allodia account**, its own place, separate from your mail accounts.

**What it is.** An account with us, for the parts of Allodia Mail & Calendar that need a server of
ours to work at all. It carries no mailbox: it cannot read, send or store your mail, and a token
issued for it cannot reach your provider. Signing in changes nothing in §2: your mail still syncs
directly between your device and the provider you chose, and we are still not in that path.

**What we hold.** Your email address, your display name if the account has one, and the sign-in
itself. Then, once you are signed in on a device, the list of mail accounts on it: for each one the
email address, the server names and ports, the user name, and the connection settings. That is the
whole of it.

**Never a password, and never a token for your provider.** Those stay in your device's own
keystore, are never sent to us, and are entered once on each device you use. It is why signing in
on a new device fills your accounts in for you and then still asks you for each password.

**We can read the account list we hold.** It is stored on our servers in ordinary readable form,
not encrypted in a way that locks us out. It tells us which providers you use and under which
address; it tells us nothing about your mail, which we do not have. If that is more than you want
to share, this is the part of the app that is optional: use it without an Allodia account and none
of it leaves your device.

**What your device holds.** The sign-in token, in your operating system's secure keystore (Keychain,
Windows Credential Manager, Android Keystore, or the Linux system keyring), the same place your
mail passwords already live, and never in the app's database.

**Where.** Allodia's own servers, in the EU.

**How you sign in, and what that costs you.** Directly with us, or with Apple or Google if you would
rather. The last two are your choice and nothing pushes you towards them. Take one and that company
learns you have an Allodia account, and hands us the address and name it releases; that is then what
we hold, exactly as described above, and nothing more. Sign in directly and no third party is in it
at all. Either way the account is ours, on our servers in the EU, and your mail is in none of it.

**Signing out, and deleting.** Signing out in Settings → Allodia account erases this device's copy of the
sign-in. Your mail accounts stay on the device, and the list we hold stays with your Allodia account
until that account is deleted. To delete the account itself, and everything we hold in it, that list
included: **Settings → Allodia account → Delete account**. That opens your account page in your
browser, where you sign in and confirm it — the app does not delete the account itself. You can open
that page directly at **https://mailcal.allodia.eu/account**, on any device and without the app. If
you would rather ask us, write to **info@allodia.eu**. Your rights are in §13.

**What it does today.** An identity, and the account list above. Beyond those the app makes no
further request to us. Any further feature that genuinely needs a server of ours will be written
into this policy **before** it ships, and will ask you before it sends anything (§15).

## 4. Data you are not required to provide

None of it. There is no legal or contractual requirement to provide us any personal data, and
the app is fully functional if you never send us anything and leave every optional switch off.

## 5. Things that happen only when you act

Some actions in a mail app necessarily send data somewhere, but to parties **you** choose, at
the moment you choose, never via Allodia:

- **Connecting an account** sends your credentials to that provider and syncs your mail with it.
- **Loading remote images** in a message fetches them from the servers that host them (often the
  sender's), which see your IP address. That is why the app blocks them by default and asks per
  message.
- **Tapping a link or opening an attachment** hands it to your system browser or the app your OS
  associates with the file. Only ordinary web and mail links (`http`, `https`, `mailto`) are ever
  handed off; attachments are never rendered inside the app.
- **Sending mail or invitations** delivers them to your recipients through your provider.

These are your dispatches, not ours: Allodia receives nothing from any of them.

## 6. Optional usage statistics (off by default)

The one thing the app can send us, and **only if you opt in**.

At first start, before any account is set up, the app asks whether you want to share usage
statistics. The switch is **off**. Nothing is stored or sent unless you switch it on and confirm.
Declining is remembered, and we don't ask again. The consent screen has a **"see exactly what we
send"** panel showing the literal data, byte for byte; the description below summarizes that
panel, but the panel is authoritative.

**What is sent if you opt in:**

| Data | Value | Deliberately not |
|---|---|---|
| Install id | A random identifier, created only at the moment you opt in | Not derived from your device, account, or any hardware id |
| Platform + OS | e.g. `android`, OS major version only (`15`) | Never a build number |
| Device class | e.g. `iphone`, `mac-laptop`, `android-tablet` | Never a device model |
| App version + language | e.g. `1.4.0`, `nl` | Language only, never a full locale/region |
| Account shape | How many accounts (bucketed: `0`, `1`, `2`, `3–5`, `6+`) and which protocol kinds | Never which providers, hosts, or addresses |
| Events | App opened; account setup started/completed/failed and sync completed/failed (per protocol kind); a feature was used (from a fixed list); which settings are switched on | Never anything you typed or configured as text |

**What is never sent, by construction and not just by policy:** message content, subjects, senders,
recipients, email addresses, folder names, message counts, search queries, calendar titles or
event details, attachment names, server hostnames, device model strings. Every field above is a
label from a fixed list or a bucket; there is no field in the payload that *could* carry your
content, and our server rejects anything outside that fixed list.

**Where it goes and how long it stays:** only to servers Allodia itself operates in the EU
(Hetzner Online GmbH, Industriestraße 25, 91710 Gunzenhausen, Germany).
No third-party analytics company is involved, ever. Your IP address is technically visible when
your device connects, as with any internet connection, but it is **not stored** and not linked to
the usage data. Usage events are kept for at most 24 months,
after which they are deleted or reduced to aggregate statistics that no longer relate to any
install id.

**What it is:** pseudonymous usage data. The install id doesn't contain your name, address, or
anything about you, but it is stable while you stay opted in (that is what lets us see whether
setup succeeds and whether people keep using the app), so we treat it as personal data under the
GDPR rather than calling it anonymous.

**What it is for:** product decisions and nothing else, such as which platforms and languages
need attention, whether account setup and sync succeed, which features are actually used, and
whether an update made things worse.

**Withdrawing:** Settings → Privacy, one switch, any time. The app deletes the install id and
consent record from your device and instructs our server to erase everything held under that id
(GDPR Art. 17). Withdrawal is exactly as easy as opting in was.

**If we ever want to send more,** the app treats your earlier consent as not covering it and asks
you again, showing the new data before anything changes. A payload can never grow under a consent
that was given for less.

Legal basis: your consent (GDPR Art. 6(1)(a); storage and reading on your device per the national
implementations of ePrivacy Art. 5(3)).

## 7. AI assistant access (off by default)

The app can let an **AI assistant running on your own computer** read and act on your mail, over
the Model Context Protocol (MCP). It is off unless you switch it on in **Settings → Advanced**,
and it exists only on macOS and Windows.

What this is, concretely:

- A private channel **on your own device**. The app opens a local socket that only your own
  operating-system user account can reach. No network port, no server of ours, and nothing to
  configure with a password or a key.
- **Nothing goes to Allodia.** We are not in this path at all. We do not know whether you have
  turned it on, which assistant you connected, or what you asked it.
- **You choose which accounts it reaches.** Turning the feature on exposes *no* mailbox. Each
  account is a separate tick, and unticking one takes effect immediately.
- The program you connect **can read and act on** the mail of the accounts you ticked: list and
  search messages, read one message in full, mark read, flag, archive, move to Trash, mark as
  spam, and open a prefilled draft in the app for you to review and send. It cannot permanently
  delete anything.
- **Sending is a separate switch**, off by default. With it off, an assistant can only open a
  draft that you read and send yourself. With it on, the app additionally refuses to send to
  anyone you have not emailed before, unless you turn *that* off too.

What you should know before switching it on: whatever the assistant reads, its own provider may
also receive, under **their** privacy policy, not ours. If it is a cloud assistant, your message
content leaves your device the moment you ask it to read something, because you asked it to. That
is the same shape as forwarding a message or loading a remote image: your dispatch, to a party you
chose. It is worth being deliberate about which accounts you tick.

## 8. When you contact us

If you email us (e.g. **info@allodia.eu**), we process what you send, namely your address,
your message, and anything you attach, solely to answer you and resolve the issue. For support,
an attachment may include the app's diagnostic log, which by design contains no message content
(§2).

Our mailboxes are hosted by **Soverin (Netherlands)**, our EU email provider. Support
correspondence is kept for 24 months after the last message,
then deleted.

Legal basis: our legitimate interest in answering the people who write to us (Art. 6(1)(f)), or
steps prior to a contract where you're asking about purchasing (Art. 6(1)(b)).

## 9. App stores and updates

You install and update the app through Apple's App Store, Google Play, or the Microsoft Store.
The stores process your purchase and device data as **independent controllers** under their own
privacy policies, not on our behalf. What we receive from them are aggregated dashboards
(installs, active devices, OS versions, device models, crash statistics) that don't identify you
to us; whether your device contributes to those is governed by your OS-level sharing settings.
The app itself contains no crash reporter and performs no update checks of its own.

## 10. The allodia.eu website

Our website uses only cookies and local storage necessary for it to function (language
preference, security, session handling), and no analytics or marketing cookies. If that ever
changes, we will ask first. If you use the contact form, we process your name, email address,
optional company name, and message to reply to you, as in §8. The site is hosted in the
Netherlands and Germany.

## 11. What we never do

- We never sell or rent personal data, and never share it for advertising.
- We never use third-party analytics, tracking, or advertising SDKs in the app.
- We never make automated decisions about you or profile you (GDPR Art. 22).
- We never train AI models on your mail. We have no access to your mail at all.
- We never transfer the personal data we hold outside the EU/EEA. (Where **your own provider**
  is located is your choice and a direct relationship between you and them. So is choosing Apple or
  Google to sign in to an Allodia account, which is a route you can simply not take, §3.)

## 12. Retention at a glance

| Data | Kept | Where |
|---|---|---|
| Your mail, events, contacts, settings, signatures, diagnostic log | On your device, under your control; delete the app's data or the app and it's gone | Your device |
| Your Allodia account and the mail-account list it keeps in step, if you create one | Until you ask us to delete it | Allodia-operated servers, EU |
| Usage statistics (opt-in) | Until you withdraw, at most 24 months | Allodia-operated servers, EU |
| Support correspondence | 24 months after the last message | Soverin, NL |
| Website contact-form messages | As support correspondence | Soverin, NL |
| Invoices/records, if you buy from us | As long as Dutch tax law requires (7 years) | Allodia administration, EU |

## 13. Your rights

Under the GDPR you can ask us for access to, correction, deletion, or a copy (portability) of
your personal data, ask us to restrict processing, object to processing based on legitimate
interest, and withdraw any consent at any time (which doesn't affect what was lawful before).

In practice, the fastest route is usually the app itself: your mail and settings are already in
your hands, and the usage-statistics switch (Settings → Privacy) is the withdrawal-and-erasure
mechanism for the one dataset we hold about the app. One honest limitation: an install id doesn't
tell us who you are, so we can't look yours up from your name or email. The in-app switch, which
proves control of the device the id belongs to, is the reliable way to have that data erased.

For everything else: **info@allodia.eu**. We respond within a month (Art. 12(3)). You can also
complain to a data protection authority. Ours is the Dutch **Autoriteit Persoonsgegevens**
(autoriteitpersoonsgegevens.nl), but you may use the authority of your own EU/EEA country.

## 14. Security

Credentials are stored only in your device's secure keystore and are never written to the app's
database or its logs. Connections to your provider use the encrypted protocols it offers (TLS).
Inbound HTML is sanitized in a hardened renderer where scripts never run and remote content is
blocked by default. The little we operate server-side runs on EU infrastructure with access
limited to those who need it, protected by multi-factor authentication and encryption at rest.

## 15. Changes to this policy

If we change what data is handled, we update this policy **before** the change ships and bump
the version and date at the top. Where the change would widen what the app sends, the app asks
for your consent again rather than assuming it. The current version is always at
**https://allodia.eu/privacy/mail-calendar**.

## 16. Contact

Allodia · KvK 56789823 · Kamerlingh Onnesweg 2, 3316GL Dordrecht, the Netherlands · **info@allodia.eu**
