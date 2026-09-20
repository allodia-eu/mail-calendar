# Mail software on your own machine can be connected

Platforms: macos, ios, windows, android, linux
Bump: minor

> Proton Mail Bridge serves its local IMAP and SMTP listeners a self-signed CA certificate as its
> own end-entity certificate, which fails with `CaUsedAsEndEntity` and which no trust anchor can
> rescue: the verifier reads the leaf's own basic constraints before it consults one. The connect
> now reports the certificate rather than a transport string, and a person who recognises their own
> server accepts that one certificate for that one server. Verification is not relaxed anywhere:
> the exception is consulted only after the policy has refused, and the same server presenting
> anything else is refused as before. See `docs/certificate-exceptions.md`.

**English**

```
Mail software running on your own machine, such as Proton Mail Bridge, can now be connected: the certificate it identifies itself with is shown to you, and the account connects once you accept it.
```

**Nederlands**

```
Mailsoftware op je eigen machine, zoals Proton Mail Bridge, kun je nu koppelen: je ziet het certificaat waarmee die zich identificeert, en zodra je het accepteert wordt de account verbonden.
```

**Deutsch**

```
E-Mail-Software auf Ihrem eigenen Rechner, etwa Proton Mail Bridge, lässt sich jetzt verbinden: Sie sehen das Zertifikat, mit dem sie sich ausweist, und sobald Sie es annehmen, wird das Konto verbunden.
```

**Français**

```
Les logiciels de messagerie installés sur votre machine, comme Proton Mail Bridge, se connectent désormais : le certificat avec lequel ils s'identifient vous est montré, et le compte se connecte dès que vous l'acceptez.
```

**Español**

```
El software de correo que se ejecuta en tu propia máquina, como Proton Mail Bridge, ya se puede conectar: se te muestra el certificado con el que se identifica y la cuenta se conecta en cuanto lo aceptas.
```

**Italiano**

```
Il software di posta in esecuzione sul tuo computer, come Proton Mail Bridge, ora si può collegare: ti viene mostrato il certificato con cui si identifica e l'account si collega appena lo accetti.
```

**Português**

```
O software de correio a funcionar na sua própria máquina, como o Proton Mail Bridge, já pode ser ligado: mostramos-lhe o certificado com que se identifica e a conta liga-se assim que o aceitar.
```
