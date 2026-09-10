# The default-mail-app offer no longer crashes the Windows app at startup

Platforms: windows
Bump: patch

> The offer is raised by the account-list signal, which `MainWindow` subscribes inside its own
> constructor, so an account that connects in tens of milliseconds (a local server, or a primed
> cached snapshot) got there before the window had a visual tree. `ContentDialog.ShowAsync` on an
> unrooted element throws "This element does not have a XamlRoot" out of an `async void`, which
> reaches no catch and takes the process down: one launch in five, and only while the question was
> still unanswered, so it hid behind having been asked once. The offer now waits for `Loaded` and
> is put then, and `DialogHelper` refuses an unrooted dialog outright so no future caller can
> bring the crash back. The rule is `DefaultMailApp.WhenToAsk`, which is WinUI-free and therefore
> unit-tested; `docs/client-traps.md` carries the trap.

**English**

```
Fixed a crash at startup, on the launch where Allodia Mail & Calendar offers to become your default mail app.
```

**Nederlands**

```
Een crash bij het opstarten verholpen, bij de start waarin Allodia Mail & Calendar aanbiedt om je standaard e-mailprogramma te worden.
```

**Deutsch**

```
Ein Absturz beim Start wurde behoben, und zwar beim Start, bei dem Allodia Mail & Calendar anbietet, Ihr Standard-E-Mail-Programm zu werden.
```

**Français**

```
Correction d'un plantage au démarrage, lors du lancement où Allodia Mail & Calendar propose de devenir votre application de messagerie par défaut.
```

**Español**

```
Se ha corregido un fallo al iniciar, en el arranque en el que Allodia Mail & Calendar ofrece convertirse en su aplicación de correo predeterminada.
```

**Italiano**

```
Corretto un arresto anomalo all'avvio, nell'avvio in cui Allodia Mail & Calendar propone di diventare la tua app di posta predefinita.
```

**Português**

```
Corrigida uma falha ao iniciar, no arranque em que o Allodia Mail & Calendar se oferece para ser a sua aplicação de correio predefinida.
```
