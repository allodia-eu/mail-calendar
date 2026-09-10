# Forwarding sends the attachments too

Platforms: all
Bump: patch

> A forward rendered the composer document and nothing else, so the original's attachments were
> left behind: the recipient got the quoted text of a message whose files were the point of
> forwarding it, and neither end could see that anything was missing. The core now reads the
> original's attachments through the engine (the same list the reading view shows, so a quoted
> body's inline `cid:` images are not duplicated as files and an invitation's own `text/calendar`
> part is still not one) and puts them on the outgoing draft. Files that cannot be read fail the
> send instead of going out missing, which is `sending.md`'s existing rule about the Sent copy
> applied a step earlier. It sits in the submit use case, so all four clients get it without a
> line of client code; what they still do not show is the strip of files a forward is about to
> send, recorded as a known gap.

**English**

```
Forwarding a message now sends its attachments along with it.
```

**Nederlands**

```
Bij het doorsturen van een bericht gaan de bijlagen nu mee.
```

**Deutsch**

```
Beim Weiterleiten einer Nachricht werden ihre Anhänge jetzt mitgesendet.
```

**Français**

```
Le transfert d'un message envoie désormais aussi ses pièces jointes.
```

**Español**

```
Al reenviar un mensaje ahora se envían también sus archivos adjuntos.
```

**Italiano**

```
Inoltrando un messaggio ora vengono inviati anche i suoi allegati.
```

**Português**

```
Ao reencaminhar uma mensagem, os seus anexos passam a seguir com ela.
```
