# Change how much mail to keep while an account is still downloading

Platforms: all
Bump: patch

> A depth change cleared the mail cursors, but a pass still running under the old depth wrote
> its own back over the clear, so a long Gmail first sync went on under the old depth and the
> older mail the new one includes was never fetched. The change now stops that pass, and a
> cursor written under a narrower depth is never resumed by a wider one.

**English**

```
Choosing to keep more mail while an account is still downloading now fetches the older mail too.
```

**Nederlands**

```
Als je meer e-mail wilt bewaren terwijl een account nog wordt gedownload, wordt de oudere e-mail nu ook opgehaald.
```

**Deutsch**

```
Wenn Sie mehr E-Mails behalten möchten, während ein Konto noch heruntergeladen wird, werden die älteren E-Mails jetzt ebenfalls abgerufen.
```

**Français**

```
Choisir de conserver plus de messages pendant qu’un compte est encore en cours de téléchargement récupère désormais aussi les messages plus anciens.
```

**Español**

```
Si eliges conservar más correo mientras una cuenta aún se descarga, ahora también se descarga el correo más antiguo.
```

**Italiano**

```
Se scegli di conservare più posta mentre un account è ancora in download, ora viene scaricata anche la posta più vecchia.
```

**Português**

```
Se optar por guardar mais correio enquanto uma conta ainda está a ser transferida, o correio mais antigo passa agora também a ser obtido.
```
