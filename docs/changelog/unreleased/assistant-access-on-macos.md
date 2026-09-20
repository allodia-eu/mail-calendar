# Assistant access on macOS

Platforms: macos
Bump: patch

> The listener keeps the socket's parent directory owner-only, and refused to start when the
> `chmod` that sets it fails. On the sandboxed build that parent is the App Group container, which
> macOS creates `0700` and owned by the user and then denies a `chmod` on, so the directory already
> met the requirement and the listener turned itself off anyway, leaving one `WARN` and no
> explanation. It now checks the property the `chmod` exists to establish, that no group or other
> bit is set, and starts when the directory already keeps every other user out. The guarantee is
> unchanged: a directory anyone else can traverse still refuses the start.

**English**

```
Assistant access now starts on macOS instead of quietly staying off after it was switched on.
```

**Nederlands**

```
Assistent-toegang start nu op macOS, in plaats van stilletjes uit te blijven nadat u hem aanzette.
```

**Deutsch**

```
Der Assistenzzugriff startet jetzt unter macOS, statt nach dem Einschalten still aus zu bleiben.
```

**Français**

```
L'accès pour l'assistant démarre désormais sur macOS, au lieu de rester discrètement désactivé après son activation.
```

**Español**

```
El acceso para el asistente ahora se inicia en macOS, en vez de quedarse apagado en silencio después de activarlo.
```

**Italiano**

```
L'accesso per l'assistente ora si avvia su macOS, invece di restare spento in silenzio dopo averlo attivato.
```

**Português**

```
O acesso do assistente arranca agora no macOS, em vez de ficar discretamente desligado depois de o ligar.
```
