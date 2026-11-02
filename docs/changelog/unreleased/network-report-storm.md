# Fewer sign-ins when the network status flickers

Platforms: all
Bump: patch

> macOS's network monitor reports the network as available every few seconds with nothing
> changed (measured: seven reports in ninety seconds on a Mac with VPN interfaces). The core took
> each as the network coming back, dropping every idle IMAP connection and dialling every
> unreachable calendar and address book again, so a mail server saw a sign-in storm and answered
> by limiting the account. Only a report that ends an offline spell does that now.

**English**

```
Mail accounts sign in to their servers far less often on a device whose network status keeps changing, so servers no longer limit them for it.
```

**Nederlands**

```
E-mailaccounts melden zich veel minder vaak aan bij hun servers op een apparaat waarvan de netwerkstatus steeds verandert, zodat servers ze daarvoor niet meer beperken.
```

**Deutsch**

```
E-Mail-Konten melden sich auf einem Gerät, dessen Netzwerkstatus ständig wechselt, viel seltener bei ihren Servern an, sodass Server sie dafür nicht mehr einschränken.
```

**Français**

```
Les comptes de messagerie se connectent beaucoup moins souvent à leurs serveurs sur un appareil dont l’état du réseau change sans cesse, si bien que les serveurs ne les limitent plus pour cela.
```

**Español**

```
Las cuentas de correo inician sesión en sus servidores con mucha menos frecuencia en un dispositivo cuyo estado de red cambia constantemente, así que los servidores ya no las limitan por ello.
```

**Italiano**

```
Gli account di posta accedono ai loro server molto meno spesso su un dispositivo il cui stato di rete cambia di continuo, così i server non li limitano più per questo.
```

**Português**

```
As contas de correio iniciam sessão nos seus servidores com muito menos frequência num dispositivo cujo estado de rede muda constantemente, pelo que os servidores já não as limitam por isso.
```
