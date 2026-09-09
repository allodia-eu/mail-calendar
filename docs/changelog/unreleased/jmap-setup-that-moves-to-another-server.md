# JMAP setup that moves to another server

Platforms: all
Bump: patch

> RFC 8620 discovery starts at the user's own domain, and a hosted provider routinely redirects
> that to the host actually running the server. The engine's session walker resolved each hop's
> `Location` against the configured base rather than against the URL that issued it, and put it
> through `SessionUrlPolicy` on the way, which forced every target back onto the origin the walk
> started from. A cross-origin hop therefore resolved to the URL it came from, the chain
> oscillated in place until the hop budget ran out, and the user saw "too many session redirects".
> Fixing the walk alone was not enough: that same base is what the session's advertised `apiUrl`
> resolves against, so after an origin change every method call would have been aimed at a host
> that never had the session. The engine now resolves a `Location` against the URL that issued it
> (RFC 9110), refuses a hop that leaves TLS since these requests carry the account's credentials,
> and hands the session the URL that actually served it. Nothing changed in this repository beyond
> the engine pin: autodetect deliberately stores the typed domain rather than an ephemeral
> redirect target, and that decision holds now that the engine follows the chain correctly.

**English**

```
Mail accounts on JMAP providers that hand setup to a separate server, such as Thundermail, can now be added.
```

**Nederlands**

```
E-mailaccounts bij JMAP-aanbieders die het instellen doorverwijzen naar een aparte server, zoals Thundermail, kunnen nu worden toegevoegd.
```

**Deutsch**

```
E-Mail-Konten bei JMAP-Anbietern, die die Einrichtung an einen separaten Server weiterleiten, etwa Thundermail, lassen sich jetzt hinzufügen.
```

**Français**

```
Les comptes de messagerie chez les fournisseurs JMAP qui confient la configuration à un serveur distinct, comme Thundermail, peuvent désormais être ajoutés.
```

**Español**

```
Las cuentas de correo de proveedores JMAP que dirigen la configuración a un servidor distinto, como Thundermail, ya se pueden añadir.
```

**Italiano**

```
Gli account di posta su provider JMAP che affidano la configurazione a un server separato, come Thundermail, ora possono essere aggiunti.
```

**Português**

```
As contas de correio em fornecedores JMAP que encaminham a configuração para um servidor separado, como o Thundermail, já podem ser adicionadas.
```
