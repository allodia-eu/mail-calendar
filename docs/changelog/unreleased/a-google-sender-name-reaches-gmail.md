# A sender name set on a Google account reaches Gmail

Platforms: all
Bump: patch

> A Google account advertises a writable sender identity, but `mail.google.com` reaches
> `users.settings.sendAs.list` and not its `patch`, so the push that follows a name change was
> refused and only the local copy moved. Sign-in now also requests `gmail.settings.basic`, the
> scope that write needs. It is a restricted scope, so it is requested now rather than when a
> later settings feature wants one: adding a restricted scope after verification is a second
> security assessment, not an amendment. Scopes are granted by incremental consent, so an
> account connected before this keeps its narrower grant until its next reconnect, which is
> why the note says *a Google account you connect* rather than every one.

**English**

```
The sender name you set for a Google account you connect from now on is also saved to Gmail, so your other apps send under the same name.
```

**Nederlands**

```
De afzendernaam die je instelt voor een Google-account dat je vanaf nu koppelt, wordt ook in Gmail bewaard, zodat je andere apps onder dezelfde naam versturen.
```

**Deutsch**

```
Der Absendername, den Sie für ein ab jetzt verbundenes Google-Konto festlegen, wird auch in Gmail gespeichert, damit Ihre anderen Apps unter demselben Namen senden.
```

**Français**

```
Le nom d'expéditeur que vous définissez pour un compte Google connecté à partir de maintenant est aussi enregistré dans Gmail, afin que vos autres applications envoient sous le même nom.
```

**Español**

```
El nombre de remitente que defines para una cuenta de Google que conectes a partir de ahora también se guarda en Gmail, para que tus otras aplicaciones envíen con el mismo nombre.
```

**Italiano**

```
Il nome mittente impostato per un account Google che colleghi d'ora in poi viene salvato anche in Gmail, così le tue altre app inviano con lo stesso nome.
```

**Português**

```
O nome de remetente que define para uma conta Google que ligue a partir de agora passa a ser guardado também no Gmail, para que as suas outras aplicações enviem com o mesmo nome.
```
