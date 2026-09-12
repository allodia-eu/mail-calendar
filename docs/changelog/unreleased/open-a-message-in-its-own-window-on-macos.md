# Open a message in its own window on macOS

Platforms: macos
Bump: minor

> The first half of [`docs/reading-window.md`](../../reading-window.md); Windows and Linux follow.
> The core kept one reading slot, so two readers could not hold two messages: `Intent::OpenMessage`
> now names its reader and the body lands in that reader's slot, which is what lets a window and
> the pane show different mail. Everything else about the open is unchanged and shared, the
> bounded retry for an account still dialing, the threshold before a loading state is announced,
> and the mark-read, so a window inherits all three rather than restating any. Closing a window
> frees its body on both sides, which matters because a sanitised body carries every inline image
> resolved into it. On the client, `ContentView` no longer owns the model: SwiftUI's default ⌘N
> would have built a second one, and a second model is a second connection to the same SQLite
> store. The composer window is the pane's own `ComposeHost`, so a reply raised in a window is
> seeded, signed and sent by the paths that already existed.

**English**

```
Double-click a message on macOS to open it in a window of its own, and reply or forward from there in a window of its own too.
```

**Nederlands**

```
Dubbelklik op macOS op een bericht om het in een eigen venster te openen, en beantwoord of stuur het daar door in eveneens een eigen venster.
```

**Deutsch**

```
Öffnen Sie unter macOS eine Nachricht per Doppelklick in einem eigenen Fenster, und antworten oder leiten Sie von dort ebenfalls in einem eigenen Fenster weiter.
```

**Français**

```
Sur macOS, double-cliquez sur un message pour l'ouvrir dans sa propre fenêtre, et répondez-y ou transférez-le depuis celle-ci dans une fenêtre distincte.
```

**Español**

```
En macOS, haga doble clic en un mensaje para abrirlo en su propia ventana, y responda o reenvíelo desde allí en otra ventana propia.
```

**Italiano**

```
Su macOS fate doppio clic su un messaggio per aprirlo in una finestra dedicata, e rispondete o inoltratelo da lì in un'altra finestra dedicata.
```

**Português**

```
No macOS, faça duplo clique numa mensagem para a abrir numa janela própria e responda ou reencaminhe a partir daí noutra janela própria.
```
