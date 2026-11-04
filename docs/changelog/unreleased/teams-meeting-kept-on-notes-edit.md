# Long notes and a Teams meeting survive an edit of a Microsoft event

Platforms: all
Bump: patch

> The engine read a Graph event's HTML body only as `bodyPreview`, which Graph cuts at 255
> characters, so a notes edit wrote back the cut text; on a Teams meeting Exchange then dropped the
> meeting, because it keeps one only while the body holds Teams' block. The engine now reads the
> whole body as text and re-reads stored events. Comes in with the engine pin.

**English**

```
Editing the notes of an event on a Microsoft calendar no longer cuts long notes short or removes its Teams meeting.
```

**Nederlands**

```
Als je de notities van een afspraak in een Microsoft-agenda bewerkt, worden lange notities niet meer ingekort en verdwijnt de Teams-vergadering niet meer.
```

**Deutsch**

```
Wenn Sie die Notizen eines Termins in einem Microsoft-Kalender bearbeiten, werden lange Notizen nicht mehr gekürzt und die Teams-Besprechung nicht mehr entfernt.
```

**Français**

```
Modifier les notes d’un événement d’un calendrier Microsoft ne tronque plus les longues notes et ne supprime plus sa réunion Teams.
```

**Español**

```
Editar las notas de un evento de un calendario de Microsoft ya no recorta las notas largas ni elimina su reunión de Teams.
```

**Italiano**

```
Modificare le note di un evento in un calendario Microsoft non accorcia più le note lunghe e non rimuove più la sua riunione Teams.
```

**Português**

```
Editar as notas de um evento num calendário da Microsoft já não corta as notas longas nem remove a reunião do Teams.
```
