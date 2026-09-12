# Search answers the query you finished typing

Platforms: all
Bump: patch

> Intents are spawned, not queued, so typing a word ran a search per keystroke concurrently, and
> they finished by wildly different margins: a two-letter prefix matches half the mailbox and
> resolves every hit from the store, measured at 1.8 s against a tenth of a second for the
> finished query. `rebuild_snapshot` published unconditionally, so the screen went to whichever
> finished last, reliably the broadest and least useful one, and nothing corrected it until an
> unrelated rebuild landed: on the reported device, not until a new message arrived. The core now
> carries `App::search_generation`, the same guard contacts search has had: a rebuild reads it
> with the query it is answering and re-checks it before publishing, dropping a list a newer
> search has superseded. macOS and Windows also gained the 250 ms debounce Android and Linux
> already had, which is latency rather than correctness: the two halves do not substitute for
> each other, since searches 250 ms apart still overlap whenever one is slow.

**English**

```
Search results no longer flicker between the right answer and an unrelated list while you are still typing.
```

**Nederlands**

```
Zoekresultaten wisselen niet langer tussen het juiste antwoord en een lijst die er niets mee te maken heeft terwijl u nog typt.
```

**Deutsch**

```
Suchergebnisse springen nicht mehr zwischen dem richtigen Treffer und einer unpassenden Liste hin und her, während Sie noch tippen.
```

**Français**

```
Les résultats de recherche n'alternent plus entre la bonne réponse et une liste sans rapport pendant que vous tapez.
```

**Español**

```
Los resultados de búsqueda ya no alternan entre la respuesta correcta y una lista sin relación mientras sigue escribiendo.
```

**Italiano**

```
I risultati di ricerca non oscillano più fra la risposta giusta e un elenco che non c'entra mentre state ancora digitando.
```

**Português**

```
Os resultados da pesquisa deixam de alternar entre a resposta certa e uma lista sem relação enquanto continua a escrever.
```
