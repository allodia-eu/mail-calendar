# Open a message in its own window on Linux

Platforms: linux
Bump: minor

> The Linux half of [`docs/reading-window.md`](../../reading-window.md); the core half was already
> there and is platform-neutral, so this is a client change. The reading view is now drawn for a
> named reader rather than for "the pane": every input it raises carries a `ReadingSource`, the
> model resolves that to one reader's state, and a window's open is the pane's own intent aimed at
> a different slot, so it inherits the mark-read, the bounded retry and the loading threshold. Both
> detached windows are `AdwWindow`s hanging off the mailbox and outside the `GtkApplication`, which
> is what keeps the app ending when the mailbox does; the reading window is the pane's own view and
> the composer window the pane's own composer, so neither is a second implementation of anything.
> `GApplication` already handed a second launch to the running process and GTK offers no "New
> Window" to remove, so the one-core rule needed a guard, not a fix.

**English**

```
Double-click a message to open it in a window of its own, and reply or forward from there in a window of its own too.
```

**Nederlands**

```
Dubbelklik op een bericht om het in een eigen venster te openen, en beantwoord of stuur het daar door in eveneens een eigen venster.
```

**Deutsch**

```
Öffnen Sie eine Nachricht per Doppelklick in einem eigenen Fenster, und antworten oder leiten Sie von dort ebenfalls in einem eigenen Fenster weiter.
```

**Français**

```
Double-cliquez sur un message pour l'ouvrir dans sa propre fenêtre, et répondez-y ou transférez-le depuis celle-ci dans une fenêtre distincte.
```

**Español**

```
Haga doble clic en un mensaje para abrirlo en su propia ventana, y responda o reenvíelo desde allí en otra ventana propia.
```

**Italiano**

```
Fate doppio clic su un messaggio per aprirlo in una finestra dedicata, e rispondete o inoltratelo da lì in un'altra finestra dedicata.
```

**Português**

```
Faça duplo clique numa mensagem para a abrir numa janela própria e responda ou reencaminhe a partir daí noutra janela própria.
```
