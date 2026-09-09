# Clear a search from the field on macOS

Platforms: macos
Bump: minor

> The three other desktop and phone fields carry a clear button from their platform control
> (`AutoSuggestBox`, `gtk::SearchEntry`, SwiftUI's `.searchable`); the wide macOS layout uses a
> plain `TextField` in the list header and had none, so leaving search meant selecting the text
> and deleting it. The button empties the field and nothing else: `onChange` already dispatches
> the clear, so the core keeps one route in and rule 6 (clearing resets the scope) holds without
> a second call site.

**English**

```
Leave a search on macOS with the clear button in the search field.
```

**Nederlands**

```
Verlaat een zoekopdracht op macOS met de wisknop in het zoekveld.
```

**Deutsch**

```
Verlassen Sie eine Suche unter macOS über die Schaltfläche zum Zurücksetzen im Suchfeld.
```

**Français**

```
Quittez une recherche sur macOS grâce au bouton d'effacement du champ de recherche.
```

**Español**

```
Salga de una búsqueda en macOS con el botón de borrado del campo de búsqueda.
```

**Italiano**

```
Uscite da una ricerca su macOS con il pulsante di cancellazione nel campo di ricerca.
```

**Português**

```
Saia de uma pesquisa no macOS com o botão de limpar do campo de pesquisa.
```
