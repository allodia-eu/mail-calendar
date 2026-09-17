# Message windows on Linux are peers of the mailbox

Platforms: linux
Bump: patch

> Both windows were built `transient_for` the mailbox, which is how GTK stacks a child above its
> parent and is `xdg_toplevel.set_parent` on Wayland, so the compositor held every message and
> composer window in front and the mailbox could not be clicked back over one. Neither window has a
> parent now; `close_all()` from the mailbox's own close handler is what still sweeps them. The
> same change would have cost them their identity: a window that is in no `GtkApplication` carries
> the process name as its Wayland `app_id`, so the desktop filed each one under a second,
> unnamed application, out of reach of the switcher that moves between one app's windows. The
> process claims the application id as its program name, which fixes every window it opens,
> Settings included.

**English**

```
A message opened in its own window can now be moved behind the mailbox, and the desktop lists it under the app, so the window switcher moves between them.
```

**Nederlands**

```
Een bericht in een eigen venster kan nu achter de postbus worden gezet, en het bureaublad toont het onder de app, zodat de vensterwisselaar ertussen schakelt.
```

**Deutsch**

```
Eine Nachricht in einem eigenen Fenster lässt sich jetzt hinter das Postfach legen, und der Desktop führt sie unter der App, sodass der Fensterwechsler zwischen beiden wechselt.
```

**Français**

```
Un message ouvert dans sa propre fenêtre peut désormais passer derrière la boîte aux lettres, et le bureau le classe sous l'application, si bien que le sélecteur de fenêtres circule entre les deux.
```

**Español**

```
Un mensaje abierto en su propia ventana ya puede pasar detrás del buzón, y el escritorio lo agrupa con la aplicación, de modo que el selector de ventanas alterna entre ambas.
```

**Italiano**

```
Un messaggio aperto in una finestra propria ora può passare dietro la casella, e il desktop lo elenca sotto l'app, così il selettore di finestre passa dall'una all'altra.
```

**Português**

```
Uma mensagem aberta numa janela própria passa a poder ficar por trás da caixa de correio, e o ambiente de trabalho lista-a na aplicação, pelo que o seletor de janelas alterna entre ambas.
```
