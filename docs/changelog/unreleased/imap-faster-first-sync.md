# The first sync of an IMAP folder is faster, most of all on Yahoo

Platforms: all
Bump: patch

> Two engine changes to the metadata fetch. Whether a message has an attachment is now read
> from its top-level headers when they show one text body, and `BODYSTRUCTURE` is asked for
> only the rest; that fetch is then shared over up to three spare pooled connections. A cold
> sync of a 2577-message Yahoo inbox went from about 65 s to about 34 s.

**English**

```
The first sync of a mail folder is faster, especially for Yahoo accounts.
```

**Nederlands**

```
De eerste synchronisatie van een e-mailmap is sneller, vooral bij Yahoo-accounts.
```

**Deutsch**

```
Die erste Synchronisierung eines E-Mail-Ordners ist schneller, besonders bei Yahoo-Konten.
```

**Français**

```
La première synchronisation d'un dossier de messagerie est plus rapide, surtout pour les comptes Yahoo.
```

**Español**

```
La primera sincronización de una carpeta de correo es más rápida, sobre todo en las cuentas de Yahoo.
```

**Italiano**

```
La prima sincronizzazione di una cartella di posta è più rapida, soprattutto per gli account Yahoo.
```

**Português**

```
A primeira sincronização de uma pasta de correio é mais rápida, sobretudo nas contas do Yahoo.
```
