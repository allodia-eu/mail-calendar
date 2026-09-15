# Settings says what keeps this copy up to date, and checks when asked

Platforms: windows
Bump: minor

> Windows ships through two mechanisms and only the OS knows which one installed a given copy,
> so the channel is read from `Package.Current.GetAppInstallerInfo()` rather than from the
> brand: a Store install is sent to the Store's own updates page, and a copy installed from an
> `.appinstaller` is checked in place. The dev loop shows nothing, having no package to
> replace. A check that could not reach a conclusion reports that rather than reporting
> success, which is the one mistake that would turn a dead update channel into silence.
> `UpdatesTests` pins the resolution, which `Package.Current` makes untestable at its call
> site. The same release gives the hosted channel a background check every eight hours, so a
> copy left running for a fortnight no longer waits a fortnight for its updates.

**English**

```
Settings → About now says what keeps your copy up to date, and checks for a new version when you ask it to.
```

**Nederlands**

```
Instellingen → Over laat nu zien wat jouw versie up-to-date houdt, en kijkt op verzoek of er een nieuwe versie is.
```

**Deutsch**

```
Einstellungen → Über zeigt jetzt, was Ihre Version aktuell hält, und sucht auf Wunsch nach einer neuen Version.
```

**Français**

```
Réglages → À propos indique désormais ce qui maintient votre version à jour, et cherche une nouvelle version quand vous le demandez.
```

**Español**

```
Ajustes → Acerca de ahora indica qué mantiene tu versión al día, y busca una versión nueva cuando se lo pides.
```

**Italiano**

```
Impostazioni → Informazioni ora indica che cosa mantiene aggiornata la tua versione e cerca una versione nuova quando glielo chiedi.
```

**Português**

```
Definições → Acerca indica agora o que mantém a sua versão atualizada e procura uma versão nova quando lho pedir.
```
