# Forwarding keeps the attachments

Platforms: all
Bump: patch

> A forward rendered the composer document and nothing else, so the original's attachments were
> left behind: the recipient got the quoted text of a message whose files were the point of
> forwarding it, and neither end could see that anything was missing. The core now stages them
> (`stage_forwarded_attachments`, the engine's own attachment list, so a quoted body's inline
> `cid:` images are not duplicated as files and an invitation's own `text/calendar` part is still
> not one) into a directory the client names, and each client seeds its composer's attachment list
> from the answer. They arrive as [`ComposerFileAttachment`]s, the shape a picked file and a shared
> one already take, so the list, the remove button and the submit path are the ones that already
> existed on all four clients. Two rules are worth keeping: the composer opens **after** staging,
> because one on screen holding nothing can be sent in the window before the files arrive; and
> files that cannot be read open the composer with its error line set, because an empty attachment
> list is a claim that the message had nothing attached.

**English**

```
Forwarding a message now brings its attachments into the composer, where you can remove any you would rather not pass on.
```

**Nederlands**

```
Bij het doorsturen van een bericht staan de bijlagen nu meteen in het opstelvenster, waar u kunt weglaten wat u liever niet meestuurt.
```

**Deutsch**

```
Beim Weiterleiten einer Nachricht stehen ihre Anhänge jetzt im Editor, wo Sie entfernen können, was Sie nicht mitsenden möchten.
```

**Français**

```
Le transfert d'un message place désormais ses pièces jointes dans la fenêtre de rédaction, où vous pouvez retirer celles que vous préférez ne pas transmettre.
```

**Español**

```
Al reenviar un mensaje, sus archivos adjuntos aparecen ahora en el editor, donde puede quitar los que prefiera no enviar.
```

**Italiano**

```
Inoltrando un messaggio, i suoi allegati compaiono ora nell'editor, dove è possibile rimuovere quelli che si preferisce non inviare.
```

**Português**

```
Ao reencaminhar uma mensagem, os seus anexos passam a aparecer no editor, onde pode remover os que prefira não enviar.
```
