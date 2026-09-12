# What changed, under every version in your software centre

Platforms: linux
Bump: patch

> The metainfo listed every release as a version and a date and nothing else, so a software centre
> drew "No details for this release" under all twelve of them. The copy had existed all along: a
> released note carries the same per-locale bullets the stores are given, so the generator now reads
> its `linux` section through `changelog_fragments.parse_release` and emits a `<description>`, one
> `<p>` for a release that changed one thing and a `<ul>` for one that changed several, each
> translation interleaved after the paragraph it translates so AppStream's locale fallback lands on
> the right original. A release with no `linux` section keeps the bare `<release>` it had: it shipped
> on other platforms only, and another platform's copy would describe a build this reader could not
> install.
>
> Two claims in the same file were also wrong about the app. `<control>touch</control>` was missing,
> and a control left out is read as one the app cannot be used with rather than one left unstated;
> the controls move to `<supports>`, because a *recommended* keyboard is what filed a tablet as
> incompatible. And `display_length` defaults to `side="shortest"`, so the bare 960 claimed that much
> *height* and put every 1366x768 laptop outside what the app recommends. `appstreamcli
> check-syscompat` went from two incompatible chassis to none.

**English**

```
Your software centre now shows what changed in each release, and lists the Linux app as one that works with a touchscreen.
```

**Nederlands**

```
Je softwarecentrum laat nu zien wat er in elke versie is gewijzigd, en vermeldt de Linux-app als een app die met een touchscreen werkt.
```

**Deutsch**

```
Ihr Software-Center zeigt jetzt, was sich in jeder Version geändert hat, und führt die Linux-App als App auf, die mit einem Touchscreen funktioniert.
```

**Français**

```
Votre logithèque affiche désormais ce qui a changé dans chaque version et présente l'application Linux comme compatible avec un écran tactile.
```

**Español**

```
Tu centro de software ahora muestra qué ha cambiado en cada versión y presenta la aplicación para Linux como compatible con una pantalla táctil.
```

**Italiano**

```
Il tuo centro software ora mostra che cosa è cambiato in ogni versione e presenta l'applicazione per Linux come compatibile con uno schermo touch.
```

**Português**

```
O seu centro de software passa a mostrar o que mudou em cada versão e apresenta a aplicação para Linux como compatível com um ecrã tátil.
```
