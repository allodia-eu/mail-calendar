# The mailbox keeps your place

Platforms: android
Bump: patch

> The list's scroll position and the search chrome were both `remember`ed inside MailboxScreen, and
> opening a message replaces that screen rather than covering it, so both died with the composition:
> every message read from halfway down the inbox cost the user their place, and a message opened
> from search results dropped them back into a list still narrowed by a query with nothing on screen
> saying so. Both now live on the activity. The search half also matches a lifetime: the core clears
> its query on nothing but this client asking, so the chrome has to last exactly as long.

**English**

```
Going back from a message returns you to the row you were on, and keeps the search you were in.
```

**Nederlands**

```
Als je een bericht sluit, kom je terug bij het bericht waar je was, en blijft je zoekopdracht
staan.
```

**Deutsch**

```
Wenn Sie eine Nachricht schließen, kehren Sie zu der Zeile zurück, bei der Sie waren, und Ihre
Suche bleibt bestehen.
```

**Français**

```
En quittant un message, vous revenez à la ligne où vous étiez, et votre recherche est conservée.
```

**Español**

```
Al cerrar un mensaje vuelves al mensaje en el que estabas y se mantiene tu búsqueda.
```

**Italiano**

```
Chiudendo un messaggio torni alla riga in cui eri e la tua ricerca resta attiva.
```

**Português**

```
Ao fechar uma mensagem, volta à linha onde estava e a sua pesquisa mantém-se.
```
