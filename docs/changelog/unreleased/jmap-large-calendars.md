# Sync large JMAP calendars and address books

Platforms: all
Bump: patch

> The engine asked for a whole calendar or address book in one request, and a server refuses a
> request past its `maxObjectsInGet`, so every sync of a large one failed (a Fastmail calendar of
> 9,808 events did). It now pages within the limit.

**English**

```
A JMAP calendar or address book with more entries than its server hands over in one go now syncs, such as a large Fastmail calendar.
```

**Nederlands**

```
Een JMAP-agenda of -adresboek met meer items dan de server in één keer meegeeft, wordt nu gesynchroniseerd, zoals een grote Fastmail-agenda.
```

**Deutsch**

```
Ein JMAP-Kalender oder -Adressbuch mit mehr Einträgen, als der Server auf einmal ausgibt, wird jetzt synchronisiert, etwa ein großer Fastmail-Kalender.
```

**Français**

```
Un calendrier ou carnet d’adresses JMAP comptant plus d’entrées que le serveur n’en fournit en une fois se synchronise désormais, comme un grand calendrier Fastmail.
```

**Español**

```
Un calendario o libreta de direcciones JMAP con más entradas de las que el servidor entrega de una vez ahora se sincroniza, como un calendario grande de Fastmail.
```

**Italiano**

```
Un calendario o una rubrica JMAP con più voci di quante il server ne fornisca in una volta ora si sincronizza, come un grande calendario Fastmail.
```

**Português**

```
Um calendário ou livro de endereços JMAP com mais entradas do que o servidor entrega de uma vez passa a sincronizar, como um calendário grande do Fastmail.
```
