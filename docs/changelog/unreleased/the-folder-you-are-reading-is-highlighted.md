# The folder you are reading is highlighted again on Apple

Platforms: macos, ios
Bump: patch

> The pane's highlight was set on the row's button, and every row but two is an `HStack` with a
> chevron beside that button, so SwiftUI read `listRowBackground` off a view that was not the
> `List`'s child and drew nothing. The two flat rows lit perfectly, which is what kept it
> invisible. It is now `sidebarRowHighlight` on the row itself, and exactly one row carries it: the
> one that **is** the scope, never the tree it sits in, so a folder lights without its account
> lighting too and a shut tree takes the highlight off screen the way a shut All Accounts group
> already takes the unified Inbox.

**English**

```
The folder pane now shows which folder you are reading: the row is highlighted, where before nothing in the pane said where you were.
```

**Nederlands**

```
Het mappenpaneel laat nu zien welke map u leest: die regel is gemarkeerd, waar eerder niets in het paneel aangaf waar u was.
```

**Deutsch**

```
Der Ordnerbereich zeigt jetzt, welchen Ordner Sie lesen: Die Zeile ist hervorgehoben, während zuvor nichts im Bereich sagte, wo Sie gerade waren.
```

**Français**

```
Le volet des dossiers indique désormais le dossier que vous lisez : sa ligne est mise en évidence, alors qu'auparavant rien dans le volet ne disait où vous étiez.
```

**Español**

```
El panel de carpetas ahora muestra qué carpeta estás leyendo: su fila aparece resaltada, mientras que antes nada en el panel indicaba dónde estabas.
```

**Italiano**

```
Il pannello delle cartelle ora mostra quale cartella stai leggendo: la riga è evidenziata, mentre prima nulla nel pannello diceva dove ti trovavi.
```

**Português**

```
O painel de pastas mostra agora que pasta está a ler: a linha fica realçada, ao passo que antes nada no painel indicava onde estava.
```
