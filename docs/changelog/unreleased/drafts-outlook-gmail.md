# Drafts save in Outlook, Gmail and signed-in Fastmail accounts

Platforms: all
Bump: patch

> The core wraps each signed-in account so its token is refreshed on every call, and those
> wrappers forward each provider verb by hand. The Graph, Gmail and JMAP sign-in wrappers left
> the two draft verbs out, so every save met the engine's default and failed with "provider does
> not support mail drafts". Graph and Gmail also left drafts out of the capabilities they report.

**English**

```
Drafts now save in Outlook and Gmail accounts, and in Fastmail accounts added by signing in.
```

**Nederlands**

```
Concepten worden nu opgeslagen in Outlook- en Gmail-accounts, en in Fastmail-accounts die je via inloggen hebt toegevoegd.
```

**Deutsch**

```
Entwürfe werden jetzt in Outlook- und Gmail-Konten gespeichert sowie in Fastmail-Konten, die Sie per Anmeldung hinzugefügt haben.
```

**Français**

```
Les brouillons s’enregistrent désormais dans les comptes Outlook et Gmail, ainsi que dans les comptes Fastmail ajoutés par connexion.
```

**Español**

```
Los borradores ya se guardan en las cuentas de Outlook y Gmail, y en las cuentas de Fastmail añadidas iniciando sesión.
```

**Italiano**

```
Le bozze ora vengono salvate negli account Outlook e Gmail e negli account Fastmail aggiunti con l’accesso.
```

**Português**

```
Os rascunhos passam a ser guardados nas contas Outlook e Gmail, e nas contas Fastmail adicionadas com início de sessão.
```
