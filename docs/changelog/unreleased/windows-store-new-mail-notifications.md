# New-mail notifications on the installed Windows app

Platforms: windows
Bump: patch

> A packaged app's `AppNotificationManager.Register()` looks for the COM server its manifest
> declares with the runtime's activation argument, and the manifest declared none, so every
> Store and direct-download build logged `notifications: could not register (COMException
> 0x80004005)` and raised no toast. The manifest now declares the toast activator and its COM
> server, under a CLSID derived from the package identity so the Store and direct packages, which
> can be installed together, do not share one. The UI suites run the unpackaged build, which
> registers its server at runtime, so they never saw it; `test_msix_manifest` now does.

**English**

```
New mail now raises a notification on Windows when the app is installed from the Microsoft Store or our website.
```

**Nederlands**

```
Nieuwe e-mail geeft op Windows nu een melding wanneer de app is geïnstalleerd via de Microsoft Store of onze website.
```

**Deutsch**

```
Neue E-Mails lösen unter Windows jetzt eine Benachrichtigung aus, wenn die App aus dem Microsoft Store oder von unserer Website installiert wurde.
```

**Français**

```
Un nouveau message déclenche désormais une notification sous Windows lorsque l’application est installée depuis le Microsoft Store ou notre site.
```

**Español**

```
El correo nuevo muestra ahora una notificación en Windows cuando la aplicación se instala desde Microsoft Store o desde nuestro sitio web.
```

**Italiano**

```
La nuova posta ora mostra una notifica su Windows quando l’app è installata dal Microsoft Store o dal nostro sito.
```

**Português**

```
O correio novo apresenta agora uma notificação no Windows quando a aplicação é instalada a partir da Microsoft Store ou do nosso site.
```
