# Certificate exceptions: reaching a server whose certificate does not verify

How a person connects a mail account on a server that identifies itself with a certificate no
recognised authority vouches for, without any client ever turning verification off.

The case this exists for is mail software running on the user's own machine. **Proton Mail
Bridge** serves its local IMAP and SMTP listeners a self-signed **CA certificate as its own
end-entity certificate**, which fails with `CaUsedAsEndEntity`. Self-hosted servers and
appliances do the same. It applies to any certificate the verifier refuses, whatever the reason.

## The rules

1. **Verification is never relaxed.** The account's TLS policy (bundled Mozilla roots ∪ the OS
   store) runs first and unchanged. An exception is consulted **only once that policy has already
   refused**, so a server that verifies normally cannot be affected by one. `engine-tls` enforces
   this, so no client can get it wrong.

2. **An exception is a pin, not a setting.** It names one **TLS server name** and one certificate,
   by the SHA-256 of its DER. The same server presenting a different certificate is refused exactly
   as it was, and no other server is affected at all. This is deliberately not
   "stop checking certificates for this host".

3. **Nothing is accepted that has not been shown.** The failed handshake records the certificate it
   refused; a client shows what that certificate claims, **and its fingerprint**, and the person
   accepts that certificate or does not. An exception is never inferred, never offered
   pre-selected, and never minted from a second, unseen handshake.

4. **What is shown.** The server name asked for; what the certificate is valid for and who
   issued it, each with the organisation in brackets and omitted when the certificate names
   neither; the validity window; and the SHA-256, uppercase and colon-separated, as every other
   tool prints one. All of it is the certificate's own claim, which is exactly what failed to
   verify, and nothing but the person's decision may rest on it. A certificate whose bytes do not
   parse still shows its fingerprint, which is taken over those bytes.

   ⚠️ **"Valid for" is the `subjectAltName`, never the `CN`.** A verifier matches a host against
   the `subjectAltName` and has no `CN` fallback, so a certificate whose `CN` is the server
   somebody expected and whose `subjectAltName` is another's fails *because of the latter*.
   Showing the `CN` would put the expected name in front of them as the certificate's claim while
   asking them to pin it. The `CN` is used only where there is no `subjectAltName` at all, which
   is itself a reason a certificate cannot verify.

   The engine normalises every claimed string before a client sees it: control and
   bidirectional-formatting characters become separators and the value is cut. These are
   attacker-controlled strings whose only purpose is to be rendered in this panel.

5. **Connect stays inert until the box is ticked**, the same gate the untrusted-settings approval
   uses ([`account-autodetect.md`](account-autodetect.md) rule 3). The two are independent:
   answering one does not answer the other. The submit path re-checks both, so the disabled button
   is the affordance and not the gate.

6. **Accepting is permanent, and scoped to the account.** The exception is written into the
   account's stored config, so every later connect of that account carries it and nobody is asked
   twice; removing the account removes it. It is not a global trust store, and it is not offered as
   "just this once": a connection the user makes every few minutes cannot ask every time and stay a
   decision.

   **Once accepted, it is carried for the rest of that setup**, before there is an account to
   store it against. The retry that follows an acceptance is the one most likely to fail on the
   password, and re-asking the certificate question over a typo would make the answer look like
   it had not been heard.

7. **The panel replaces the error, it does not sit under it.** While the certificate is on screen
   the transport's own message is not: the panel says the same thing in the reader's language, with
   the certificate beside it, and the raw text would be a second, worse copy of the question.
   A refusal on a route that cannot carry an exception keeps its message, because nothing else
   would say it.

8. **Only the IMAP route offers acceptance today.** An `[imap]` account's stored config carries
   `[[certificate_exception]]`; a JMAP account's does not (see Known gaps). Every other route
   reports the refusal plainly. Taking an answer and ignoring it is worse than not asking.

9. **A log line never carries the certificate.** The refusal's own message does, which is why the
   clients do not use it for the log; what is logged is the transport's reason, as before
   ([`logging.md`](logging.md)).

## Where each part lives

| Part | Where |
|---|---|
| The trust decision, and the refusal record | `engine-tls` (`CertificateException`, `TlsClientConfig::rejected`), engine repo |
| Reading the certificate's claims out of its DER | `engine-tls` (`CertificateSummary`), engine repo |
| The stored form, and the account's TLS build | `mailcal-account` (`certificate.rs`, `tls.rs`) |
| Classifying a connect failure as a refusal | `mailcal-account` (`AccountError::CertificateRejected`) |
| The FFI record and the way back in | `mailcal-bindings` (`RejectedCertificate`, `AccountSetup::accepted_certificate`) |
| What is on screen, and the date formatting | each client |

The stored shape, in the account config a host keeps in its OS secure store:

```toml
[[certificate_exception]]
server_name = "127.0.0.1"
sha256 = "48b861f39321...1714"
```

Written as lowercase hex, read back from either that or the displayed `48:B8:…` form. An entry
whose fingerprint cannot be read is **dropped**, not refused: the connect it would have permitted
then fails as it always did and asks the person again, where refusing to load the account would
lock them out of a mailbox over a file they never see.

## Per-platform matrix

Legend: ✅ implemented · 🚧 code-complete, runtime unverified · ⬜ planned.

| Gate | Shared core | macOS / iOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| Refusal carries the certificate, not just a message | ✅ | ✅ | ✅ | ✅ | ✅ |
| Certificate shown before it can be accepted | ✅ | ✅ | ✅ | 🚧 | 🚧 |
| Connect inert until accepted | ✅ | ✅ | ✅ | 🚧 | 🚧 |
| Acceptance stored with the account, asked once | ✅ | ✅ | ✅ | 🚧 | 🚧 |
| Detected card **and** manual form | ✅ | ✅ | 🚧 | 🚧 | 🚧 |
| Exception applies to IMAP, SMTP and CalDAV of that account | ✅ | ✅ | ✅ | ✅ | ✅ |

The **shared TLS config is per account**, so an accepted certificate covers every provider of that
account whose server name it matches. Proton Mail Bridge serves the same certificate on its IMAP
and SMTP listeners under the same name, so one acceptance connects both.

macOS/iOS is verified end to end: a connect to a local listener serving a `CA:TRUE` self-signed
certificate was refused and reported it, the panel drew the subject, issuer, validity window and
fingerprint, Connect stayed disabled until the box was ticked, and the retry completed the TLS
handshake and failed on the login instead.

Windows is verified against a running Proton Mail Bridge: the connect was refused and named the
server, the panel's subject, issuer, validity window and SHA-256 matched what `openssl` read off
the listener, Connect stayed inert until the box was ticked, the account then connected and
synced, and a later launch did not ask again. Only its detected card has been driven, so the
manual form keeps its 🚧.

Android and Linux are written against the same core surface and covered by their own unit suites,
and are owed a run on their platform.

## Known gaps

- **A certificate that changes after setup is not offered again.** The exception pins one
  certificate, so a server that regenerates its own (a Bridge reinstall, a rotated self-signed
  certificate) stops connecting and reports the refusal. There is no per-account repair surface to
  raise the question on: only Linux has one at all today. Removing and re-adding the account is the
  way back, which costs the account its cached mail. The mechanism needs nothing new; the surface
  does.
- **JMAP accounts cannot carry an exception.** A JMAP account's stored config is its own type and
  has no `certificate_exception` (rule 8). The engine and the account's TLS build already support
  it, so this is config plumbing and a second form, not a design question.
- **CalDAV and SMTP are covered but never asked about.** They share the account's TLS config, so an
  exception accepted for the mail host admits them when their server name matches. A calendar or
  submission host on a *different* name that also fails to verify is refused, and the person is
  never offered that one: the mail connect is the only one they wait on.
- **An acceptance does not survive leaving the flow.** It is carried across the attempts of one
  setup and dropped when the form closes, because until the account exists there is nothing to
  store it against. Somebody who cancels and starts again is asked once more.
- **The manual form is implicit-TLS only.** Inherited from
  [`account-autodetect.md`](account-autodetect.md) → Known gaps, and it bites here: Proton Mail
  Bridge is STARTTLS on 1143/1025, so a Bridge account is reachable through the **detected** card
  (Proton publishes autoconfig naming those ports) and not by typing the host by hand.

## Testing

- **Engine** (`engine-tls/tests/exceptions.rs`, engine repo): against a real handshake with a
  server presenting a `CA:TRUE` leaf. The first test is the one that decides the design, that such
  a certificate is refused **even as a trust anchor**, so the exception is not a convenience for
  something a custom root could have done. The rest hold the pin: an exception admits the
  certificate it names, not a second certificate for the same name, not the same certificate under
  another name, and a valid certificate still verifies normally alongside one.
- **Core** (`mailcal-account/tests/certificate_exception.rs`): the whole loop through the real
  connect path against an in-process server with that certificate. The refusal reports the
  certificate and its claims; accepting it gets the account through TLS, so the failure moves on to
  IMAP; a second certificate and another server name are both still refused.
- **Config** (`mailcal-account`): the stored TOML round-trip, that an ordinary account writes no
  exception at all, and that an unreadable fingerprint drops rather than failing the account.
- **Clients**: the connect gate is a plain, Compose/SwiftUI/XAML-free object in each client and is
  unit-tested there (Apple `AccountSetupDetectTests`, Android `AccountSetupDetectTest`, Windows
  `AccountDetectFormTests`, Linux `setup_model_tests`), including that accepting a certificate does
  not also approve untrusted settings. Linux additionally drives the rendered widget tree.
