# Save a message as an .eml file

Platforms: all
Bump: minor

> The reading view's action row gains a menu at its end, after archive and delete, holding the
> actions on the message as a document rather than on its place in the mailbox
> (`docs/reading-actions.md`). Its first item writes the message out as a `.eml`. The file is the
> raw source the engine cached, byte for byte, so it opens elsewhere as the message the sender
> actually sent: a file rebuilt from the reading view would carry our headers, our part order and
> our transfer encodings, and no signature over the original would still verify. The engine gained
> `Engine::message_source` for it, since every existing read over that blob hands back a decoding
> of it. The name comes from the core, from the subject the client displays, so a subject holding
> a slash or a right-to-left override cannot decide where the file lands.

**English**

```
An open message can now be saved as an .eml file, from the new menu at the end of the reading toolbar.
```

**Nederlands**

```
Een geopend bericht kan nu worden opgeslagen als .eml-bestand, via het nieuwe menu aan het eind van de leesbalk.
```

**Deutsch**

```
Eine geöffnete Nachricht lässt sich jetzt als .eml-Datei sichern, über das neue Menü am Ende der Leseleiste.
```

**Français**

```
Un message ouvert peut désormais être enregistré au format .eml, depuis le nouveau menu à la fin de la barre de lecture.
```

**Español**

```
Un mensaje abierto ya se puede guardar como archivo .eml, desde el nuevo menú al final de la barra de lectura.
```

**Italiano**

```
Un messaggio aperto ora può essere salvato come file .eml, dal nuovo menu alla fine della barra di lettura.
```

**Português**

```
Uma mensagem aberta já pode ser guardada como ficheiro .eml, a partir do novo menu no fim da barra de leitura.
```
