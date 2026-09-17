# Open a message in its own window on Windows

Platforms: windows
Bump: minor

> The Windows half of [`docs/reading-window.md`](../../reading-window.md); the core half already
> shipped with macOS. The reading view now takes its message and its body from a reader rather than
> from the model directly, so the same view draws the pane and the whole of a window, and the action
> row's behaviour is handed in: a window replies into a composer window of its own and closes on
> archive or delete. One trap is Windows' own: the list raises its click on the FIRST press of a
> double-click, so the pane opens the row before anything knows a window was wanted. The pane is put
> back once the double-tap arrives, and the trailing click is refused so it cannot undo that
> correction, which is the ordering the whole arrangement turns on and is measured rather than
> assumed. The same press is why the window is shown a dispatcher turn after it is asked for: shown
> from inside the gesture, it was still there when the list focused the row under the pointer, and
> that focus activated the mailbox back over it. WinUI offers no "New Window" and the app already
> redirected a second launch to the running process, so one core over one store held without a
> change.

**English**

```
Double-click a message on Windows to open it in a window of its own, and reply or forward from there in a window of its own too.
```

**Nederlands**

```
Dubbelklik in Windows op een bericht om het in een eigen venster te openen, en beantwoord of stuur het daar door in eveneens een eigen venster.
```

**Deutsch**

```
Öffnen Sie unter Windows eine Nachricht per Doppelklick in einem eigenen Fenster, und antworten oder leiten Sie von dort ebenfalls in einem eigenen Fenster weiter.
```

**Français**

```
Sous Windows, double-cliquez sur un message pour l'ouvrir dans sa propre fenêtre, et répondez-y ou transférez-le depuis celle-ci dans une fenêtre distincte.
```

**Español**

```
En Windows, haga doble clic en un mensaje para abrirlo en su propia ventana, y responda o reenvíelo desde allí en otra ventana propia.
```

**Italiano**

```
Su Windows fate doppio clic su un messaggio per aprirlo in una finestra dedicata, e rispondete o inoltratelo da lì in un'altra finestra dedicata.
```

**Português**

```
No Windows, faça duplo clique numa mensagem para a abrir numa janela própria e responda ou reencaminhe a partir daí noutra janela própria.
```
