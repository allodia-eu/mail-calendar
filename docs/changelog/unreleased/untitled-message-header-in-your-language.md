# An untitled message is headed in your own language on Windows

Platforms: windows
Bump: patch

> The Windows reading pane wrote its "(no subject)" placeholder as an English literal rather than
> reading `mail_no_subject` from the catalog, so a message with a blank subject was headed in
> English in all seven languages. Found while wiring the `.eml` export, which names the file after
> the subject the pane *displays* (`docs/reading-actions.md`): left as it was, an untitled message
> would also have exported under an English name on Windows alone.

**English**

```
A message with no subject is now headed in your own language, not in English.
```

**Nederlands**

```
Een bericht zonder onderwerp krijgt nu een kop in uw eigen taal, niet in het Engels.
```

**Deutsch**

```
Eine Nachricht ohne Betreff wird jetzt in Ihrer eigenen Sprache überschrieben, nicht auf Englisch.
```

**Français**

```
Un message sans objet s'affiche désormais avec un titre dans votre langue, et non en anglais.
```

**Español**

```
Un mensaje sin asunto ahora se encabeza en su propio idioma, no en inglés.
```

**Italiano**

```
Un messaggio senza oggetto ora è intestato nella tua lingua, non in inglese.
```

**Português**

```
Uma mensagem sem assunto passa a ter um cabeçalho no seu próprio idioma, e não em inglês.
```
