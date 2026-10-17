# A mail link brings the running app forward on Linux

Platforms: linux
Bump: patch

> The running app activated its window without presenting it, which on Wayland leaves it behind,
> and the launch that handed the link over never exited: relm4's run ends with a blocking main
> context iteration that nothing woke in a process that only forwarded its command line.

**English**

```
Clicking an email link while the app is open now brings it to the front with the new message, and no longer leaves a process running in the background.
```

**Nederlands**

```
Als je op een e-maillink klikt terwijl de app open is, komt die nu naar voren met het nieuwe bericht, en blijft er geen proces meer op de achtergrond draaien.
```

**Deutsch**

```
Wenn Sie bei geöffneter App auf einen E-Mail-Link klicken, kommt sie jetzt mit der neuen Nachricht in den Vordergrund, und es läuft kein Prozess mehr im Hintergrund weiter.
```

**Français**

```
Cliquer sur un lien e-mail alors que l’app est ouverte la fait désormais passer au premier plan avec le nouveau message, sans laisser de processus en arrière-plan.
```

**Español**

```
Al hacer clic en un enlace de correo con la app abierta, ahora pasa al frente con el mensaje nuevo y ya no deja un proceso en segundo plano.
```

**Italiano**

```
Facendo clic su un link email mentre l’app è aperta, ora passa in primo piano con il nuovo messaggio e non lascia più un processo in esecuzione in background.
```

**Português**

```
Ao clicar numa ligação de email com a app aberta, esta passa agora para a frente com a nova mensagem e deixa de ficar um processo a correr em segundo plano.
```
