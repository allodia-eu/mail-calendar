# Product docs: Allodia Mail & Calendar

Cross-cutting specs and contracts for the product core and its clients: the things that
must hold the same way across **every** platform, so they live in one enforced place rather
than drifting between clients.

> The rules that reach beyond any one surface (brand, sovereignty, security) are in
> [`../AGENTS.md`](../AGENTS.md) → "Non-negotiables". These docs add the product-specific contracts
> on top.

## Contents

| Doc | What it governs |
|---|---|
| [`architecture.md`](architecture.md) | The onboarding map (diagrams, not a contract): the thin-client / shared-Rust-core / engine layering, the dispatch → snapshot reactive loop, the crate map, the host-service ports each platform implements, and the generated-bindings boundary. |
| [`analytics.md`](analytics.md) | The cross-platform contract for consented product analytics: opt-in consent (ePrivacy Art. 5(3): the *act* of writing an identifier needs consent regardless of whether the data is personal), the install id minted only at consent, the closed-enum payload that structurally cannot carry mail content, the "see exactly what we send" preview, one-click withdrawal + backend erasure, and the sovereignty carve-out. |
| [`background-sync.md`](background-sync.md) | The cross-platform contract for background mail delivery + new-mail notifications: the shared `run_background_sync` FFI port + headless build, the persisted per-account high-water-mark (inbound-Inbox-only, first-run seeding), per-platform mechanisms (desktop live runtime · Android WorkManager · iOS BGAppRefreshTask), the notification content policy, and the future opt-in paid push seam. |
| [`composer-security.md`](composer-security.md) | The cross-platform security contract for the rich HTML composer: bundled editor assets, local-only WebView execution, narrow bridge shape, paste/import sanitisation, attachment/CID handling, and native network/navigation gates. |
| [`logging.md`](logging.md) | The cross-platform contract for diagnostic logging: the shared `Logger` FFI port, the rotating/size-capped, privacy-safe local log file every client keeps, per-platform paths/format/level defaults, and the foundation for a future auto-attach support feature. |
| [`mcp.md`](mcp.md) | The cross-platform contract for agent (MCP) access: writes go through the same door the user does while reads take a different one, the stdio-over-socket transport and why there is no token, the shared bar (off by default, an empty account allow list, direct send behind its own toggle, the known-recipient guard, bodies only from `get_message` as fenced plain text, no irreversible primitive), the tool set and what was cut from it, and an honest account of what none of it defends against. |
| [`provider-oauth.md`](provider-oauth.md) | The cross-platform contract for connecting an OAuth mail account (Microsoft 365 first): PKCE public-client sign-in in the system browser, custom-scheme redirect with `state` validation, refresh/rotation, and tokens held only in the OS keystore. |
| [`jmap.md`](jmap.md) | The cross-platform contract for connecting a JMAP account (RFC 8620/8621): a base URL + one credential (HTTP Basic or a bearer/API token, no on-device OAuth), session autodiscovery, one account-wide provider, sync-depth windowing, and what works today vs. the engine gaps (mail actions, attachments, push). |
| [`onboarding.md`](onboarding.md) | The first-run screen that adds the first mail account: what is on it and in what order, what disappears in a build carrying no Allodia registration, and the bound on what the recommendation card may claim. |
| [`account-autodetect.md`](account-autodetect.md) | The cross-platform contract for email-first account setup: the four detection strategies (JMAP probe · autoconfig · ISPDB · host-resolved MX) raced in priority order, the trusted-vs-untrusted approval gate, the never-email-in-URLs privacy rule, the host `MxResolver` DNS port, and the routing onto a prefilled JMAP / IMAP / Microsoft form with manual as the escape. |
| [`rendering-security.md`](rendering-security.md) | The cross-platform security contract for rendering untrusted message content (the reading view): sanitisation, the strict-CSP document, and the native WebView gates every client must implement. |
| [`reporting.md`](reporting.md) | The cross-platform contract for reporting a message: marking spam is a **report to the provider**, not a folder move: the report files the message itself, so a client must never move it as well; which verdicts exist is read from `Capabilities::mail_report` (Gmail has no phishing verdict); and no client may claim the provider acted unless its evidence is `Acknowledged`. |
| [`timestamps.md`](timestamps.md) | The cross-platform contract for mail timestamp display: a compact **relative** label on the list row (today → time · past six days → weekday · this year → day + month · older → with year) and the **full absolute** date in the reading header, formatted client-side (the core is tzdata-free), with the pure bucket-selection factored out and unit-tested. |
| [`versioning.md`](versioning.md) | The cross-platform contract for the one app version: the top-level `/VERSION` single source of truth (marketing version), the per-store derived build numbers (Apple dotted-timestamp `CFBundleVersion`, Windows MSIX `.0` revision, Android `versionCode` formula), the two committed mirrors, and the `version-sync` drift guard. |
| [`store-listing.md`](store-listing.md) | The cross-platform contract for app-store copy: the stores' field limits, the rule that one description body is written once and reused verbatim across every store, how the Linux metainfo is generated, and the two hard couplings: copy may not out-run the README capability matrix, and every privacy claim must match `privacy-policy.md`. The copy itself lives beside the brand it belongs to, in `branding/<brand>-listing.md`. |
| [`changelog.md`](changelog.md) | The cross-platform contract for release notes ("What's new"): a change writes one **fragment** under `changelog/unreleased/` (per-locale notes, the `Platforms:` it reached, `minor` or `patch`), and `scripts/dev/release.py` assembles the pending ones into `changelog/released/X.Y.Z.md` and moves `/VERSION`. One file per change, so two PRs never conflict; each note is measured against the tightest store its own platforms reach, with the same anti-hype (matches the README matrix) and privacy-consistency couplings as the store listing. |

## Adding a doc

Keep each doc a single, enforceable contract: state the rule, then a **per-platform matrix**
that makes compliance checkable at a glance. When a doc defines a gate that clients must meet,
add the enforcement rule to [`../AGENTS.md`](../AGENTS.md) so it's binding, and link the doc
from the relevant client code.
