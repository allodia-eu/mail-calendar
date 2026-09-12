# A steadier mailbox screen on the phone

Platforms: ios
Bump: patch

> The message list was a row inside a `VStack`, so it was not the scroll view the navigation bar
> sizes itself against. The search field became pinned chrome that hid itself the moment the
> refresh control moved the content offset: a pull dropped the field and threw every row up by its
> height, then dropped them back when the sync finished. The list is now the navigation stack's
> content, with the folder name as a large title, which is what puts the title and the field inside
> the scroll view; the horizon line, the selection bar and the sync strip are safe-area insets, so
> they draw where stack rows drew them without standing between the list and the bar. The list's
> overflow menu went with it: conversation grouping and Reset database both live in Settings, and
> the menu was the last of the surfaces that predated them.

**English**

```
Pulling down to sync on the mailbox list no longer hides the search field or jolts the messages, and the list's menu is gone: both of its options are in Settings.
```

**Nederlands**

```
Naar beneden trekken om te synchroniseren verbergt het zoekveld niet meer en laat de berichten niet meer verspringen, en het menu van de lijst is weg: beide opties staan in Instellingen.
```

**Deutsch**

```
Das Herunterziehen zum Synchronisieren verbirgt das Suchfeld nicht mehr und lässt die Nachrichten nicht mehr springen, und das Menü der Liste ist weg: beide Optionen stehen in den Einstellungen.
```

**Français**

```
Tirer vers le bas pour synchroniser ne masque plus le champ de recherche ni ne fait sauter les messages, et le menu de la liste a disparu : ses deux options sont dans les Réglages.
```

**Español**

```
Tirar hacia abajo para sincronizar ya no oculta el campo de búsqueda ni desplaza los mensajes, y el menú de la lista ha desaparecido: sus dos opciones están en los Ajustes.
```

**Italiano**

```
Trascinare verso il basso per sincronizzare non nasconde più il campo di ricerca né fa sobbalzare i messaggi, e il menu dell'elenco non c'è più: entrambe le opzioni sono nelle Impostazioni.
```

**Português**

```
Arrastar para baixo para sincronizar já não esconde o campo de pesquisa nem faz saltar as mensagens, e o menu da lista desapareceu: ambas as opções estão nas Definições.
```
