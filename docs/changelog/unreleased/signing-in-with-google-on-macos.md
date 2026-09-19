# Signing in with Google on macOS

Platforms: macos
Bump: patch

> Google's Desktop client type redirects to `http://127.0.0.1:<port>/`, so the macOS client binds a
> listener on loopback for the length of one sign-in. The App Sandbox refuses that bind without
> `com.apple.security.network.server`, which the entitlements did not grant, so the flow ended at
> `EPERM` before the browser ever opened and the setup sheet showed the raw errno. The sandbox
> applies to the Mac App Store build alone, so the notarized `.dmg` from the same commit signed in
> correctly and nothing in a build, a test or an upload could see it. The grant is the sandbox's and
> the reach is still the listener's own: it names `127.0.0.1` as its required local endpoint and is
> dropped when the redirect lands. The browser hop also writes to the diagnostic log now, per stage
> and for all four sign-in routes, so a sign-in that ends early says where.

**English**

```
Signing in with Google now works on macOS; it used to stop with "Operation not permitted" before the browser opened.
```

**Nederlands**

```
Aanmelden met Google werkt nu op macOS; eerder stopte het met "Operation not permitted" voordat de browser openging.
```

**Deutsch**

```
Die Anmeldung mit Google funktioniert jetzt unter macOS; zuvor brach sie mit "Operation not permitted" ab, bevor der Browser aufging.
```

**Français**

```
La connexion avec Google fonctionne désormais sur macOS ; auparavant elle s'arrêtait sur "Operation not permitted" avant même l'ouverture du navigateur.
```

**Español**

```
Iniciar sesión con Google ya funciona en macOS; antes se detenía con "Operation not permitted" antes de que se abriera el navegador.
```

**Italiano**

```
L'accesso con Google ora funziona su macOS; prima si fermava con "Operation not permitted" prima che si aprisse il browser.
```

**Português**

```
Iniciar sessão com a Google já funciona no macOS; antes parava com "Operation not permitted" antes de o navegador abrir.
```
