# The Mac window holds still whatever a message holds

Platforms: macos
Bump: patch

> A pane of the Mac window answered SwiftUI with its content's height whenever that was taller than
> the window: the reading pane and the composer do not scroll as a whole, and their frames filled
> without capping. A tall invitation card or a long file list made the window's whole content taller
> than the window, and SwiftUI centred it, so the command bar slid under the title bar and
> Calendar, Contacts and Settings dropped off the bottom. Every pane, and both detached windows, now
> takes the height it is offered and no more. Across the width, the panes' minimums came to
> 1,160 pt against a 1,080 pt window, so with the sidebar widened the reading header ran off the
> right edge; the window's minimum is now the sum of the panes'.

**English**

```
On the Mac, a long meeting invitation no longer moves the whole window's contents, so the action bar and Settings stay in view, and the reading pane is no longer cut off in a narrow window.
```

**Nederlands**

```
Op de Mac verschuift een lange vergaderuitnodiging niet langer de hele inhoud van het venster, zodat de actiebalk en Instellingen in beeld blijven, en het leesvenster wordt in een smal venster niet meer afgesneden.
```

**Deutsch**

```
Auf dem Mac verschiebt eine lange Besprechungseinladung nicht mehr den gesamten Fensterinhalt, sodass die Aktionsleiste und die Einstellungen sichtbar bleiben, und der Lesebereich wird in einem schmalen Fenster nicht mehr abgeschnitten.
```

**Français**

```
Sur Mac, une longue invitation à une réunion ne décale plus tout le contenu de la fenêtre : la barre d'actions et les Réglages restent visibles, et le volet de lecture n'est plus coupé dans une fenêtre étroite.
```

**Español**

```
En el Mac, una invitación larga a una reunión ya no desplaza todo el contenido de la ventana, así que la barra de acciones y los Ajustes siguen a la vista, y el panel de lectura ya no se corta en una ventana estrecha.
```

**Italiano**

```
Sul Mac, un lungo invito a una riunione non sposta più tutto il contenuto della finestra, quindi la barra delle azioni e le Impostazioni restano visibili, e il riquadro di lettura non viene più tagliato in una finestra stretta.
```

**Português**

```
No Mac, um convite longo para uma reunião já não desloca todo o conteúdo da janela, pelo que a barra de ações e as Definições continuam visíveis, e o painel de leitura já não fica cortado numa janela estreita.
```
