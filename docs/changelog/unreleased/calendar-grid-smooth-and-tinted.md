# The calendar grid scrolls and zooms smoothly, and events are drawn as Apple's Calendar draws them

Platforms: macos, ios
Bump: minor

> The grid pulled every visible week across the FFI several times per frame, created date
> formatters per frame, re-ran the whole calendar screen (toolbar included) on every frame of
> motion, and built a SwiftUI view per event. Pages are now held per calendar version, the grid's
> position lives in its own observable object, and each week is one drawn canvas with an
> accessibility layer placed by the same geometry. The tinted block, the wrapped title and the
> corner glyphs are a product rule for every platform (docs/calendar.md §1, §4); the core serves
> the colours to all clients, and Windows, Android and Linux follow.

**English**

```
The calendar now scrolls and zooms smoothly, and its events are tinted in their calendar's colour, show long titles in full where there is room, and mark repeating events.
```

**Nederlands**

```
De agenda scrolt en zoomt nu soepel, en afspraken krijgen een tint van de kleur van hun agenda, tonen lange titels voluit waar er ruimte is en markeren terugkerende afspraken.
```

**Deutsch**

```
Der Kalender scrollt und zoomt jetzt flüssig, und Termine erscheinen in einem Farbton ihres Kalenders, zeigen lange Titel vollständig, wo Platz ist, und kennzeichnen wiederkehrende Termine.
```

**Français**

```
Le calendrier défile et zoome désormais avec fluidité, et les événements prennent une teinte de la couleur de leur calendrier, affichent les titres longs en entier quand la place le permet et signalent les événements récurrents.
```

**Español**

```
El calendario se desplaza y amplía ahora con fluidez, y los eventos adoptan un tono del color de su calendario, muestran los títulos largos completos cuando hay espacio y señalan los eventos que se repiten.
```

**Italiano**

```
Il calendario ora scorre e si ingrandisce in modo fluido, e gli eventi assumono una tinta del colore del loro calendario, mostrano per intero i titoli lunghi quando c'è spazio e segnalano gli eventi ricorrenti.
```

**Português**

```
O calendário desliza e amplia agora com fluidez, e os eventos ganham um tom da cor do respetivo calendário, mostram os títulos longos por inteiro quando há espaço e assinalam os eventos que se repetem.
```
