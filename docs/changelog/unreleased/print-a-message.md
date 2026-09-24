# Print a message

Platforms: all
Bump: minor

> The reading view's overflow menu gains Print, below Save as .eml. The page is built once in the
> core (`render_print_document`): the subject and the From, To, Cc, Bcc and Sent lines the reading
> header draws, as escaped text, above the sanitised body, inside the same strict-CSP document the
> reading view loads, so a printout loads a remote image only when the reader already did. Each
> client lays it out in a second web view carrying the reading host's gates and hands that to the
> platform's print dialog (`docs/reading-actions.md`).

**English**

```
An open message can now be printed, from the menu at the end of the reading toolbar.
```

**Nederlands**

```
Een geopend bericht kan nu worden afgedrukt, via het menu aan het eind van de leesbalk.
```

**Deutsch**

```
Eine geöffnete Nachricht lässt sich jetzt drucken, über das Menü am Ende der Leseleiste.
```

**Français**

```
Un message ouvert peut désormais être imprimé, depuis le menu à la fin de la barre de lecture.
```

**Español**

```
Un mensaje abierto ya se puede imprimir, desde el menú al final de la barra de lectura.
```

**Italiano**

```
Un messaggio aperto ora può essere stampato, dal menu alla fine della barra di lettura.
```

**Português**

```
Uma mensagem aberta já pode ser impressa, a partir do menu no fim da barra de leitura.
```
