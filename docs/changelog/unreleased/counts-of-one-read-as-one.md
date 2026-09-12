# Counts of one read as one

Platforms: all
Bump: patch

> The catalog had no plural mechanism, so every `{count}` message carried a single hard-plural
> form and a folder holding one thing said "1 conversations". `mailcal-l10n` now folds a
> `<key>_one` partner into the plural's accessor, which picks the form the active locale's
> grammar asks for; the partner gets no accessor of its own, so a caller passes a count and the
> four clients cannot disagree about the rule. No client code changed. French takes the singular
> at zero as well as one (CLDR), which is reachable: an empty folder still states its count.
> Deliberately narrow: only the eight keys that can actually render one are paired (the three
> mailbox counts, the two selection counts, the three reminder offsets); month counts never reach
> one, since `SYNC_DEPTHS` starts at three. The older pairs that spell the numeral into the
> sentence ("1 attendee") are two messages a call site chooses between, not plural forms, and are
> left alone; carrying the `{count}` is what separates the two, and a test pins it.

**English**

```
A list or a selection holding one thing now reads "1 conversation" rather than "1 conversations".
```

**Nederlands**

```
Een lijst of selectie met één item toont nu "1 gesprek" in plaats van "1 gesprekken".
```

**Deutsch**

```
Eine Liste oder Auswahl mit einem Element zeigt jetzt "1 Unterhaltung" statt "1 Unterhaltungen".
```

**Français**

```
Une liste ou une sélection ne contenant qu'un élément affiche désormais "1 conversation" et non "1 conversations".
```

**Español**

```
Una lista o selección con un solo elemento ahora muestra "1 conversación" en lugar de "1 conversaciones".
```

**Italiano**

```
Un elenco o una selezione con un solo elemento ora mostra "1 conversazione" anziché "1 conversazioni".
```

**Português**

```
Uma lista ou seleção com um só item passa a mostrar "1 conversa" em vez de "1 conversas".
```
