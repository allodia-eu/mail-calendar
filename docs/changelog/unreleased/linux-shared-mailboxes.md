# Shared mailboxes on Linux

Platforms: linux
Bump: minor

> A mailbox shared with an IMAP or JMAP account (an RFC 2342 shared namespace, or a JMAP session's
> non-personal account) can be switched on from the account's page in Settings → Accounts. On
> Microsoft 365 it is a use of its own beneath mail, asking for `Mail.ReadWrite.Shared` and
> `Mail.Send.Shared` only while chosen, and a mailbox is added by its address, since Graph lists
> none. Each is listed as a mail account of its own, opened with the parent account's sign-in, and
> sends from its own address under the name its server holds. The Azure app registration needs
> both scopes as delegated permissions before this ships, or a sign-in asking for them is refused.
> The engine bump that brings this re-reads every account's folders once, so the first launch
> after updating syncs every account in full once, on every platform. Privacy policy 2.7.

**English**

```
Mailboxes shared with your account, such as a team's, including on Microsoft 365, can be added in Settings → Accounts. Each appears as a mailbox of its own and sends from its own address.
```

**Nederlands**

```
Mailboxen die met je account zijn gedeeld, zoals die van een team, ook bij Microsoft 365, kun je toevoegen in Instellingen → Accounts. Elke mailbox verschijnt als eigen mailbox en verstuurt vanaf zijn eigen adres.
```

**Deutsch**

```
Postfächer, die mit Ihrem Konto geteilt sind, etwa das eines Teams, auch bei Microsoft 365, lassen sich unter Einstellungen → Konten hinzufügen. Jedes erscheint als eigenes Postfach und sendet von seiner eigenen Adresse.
```

**Français**

```
Les boîtes aux lettres partagées avec votre compte, comme celle d’une équipe, y compris sur Microsoft 365, peuvent être ajoutées dans Réglages → Comptes. Chacune apparaît comme une boîte à part entière et envoie depuis sa propre adresse.
```

**Español**

```
Los buzones compartidos con tu cuenta, como el de un equipo, también en Microsoft 365, se pueden añadir en Ajustes → Cuentas. Cada uno aparece como un buzón propio y envía desde su propia dirección.
```

**Italiano**

```
Le caselle di posta condivise con il tuo account, come quella di un team, anche su Microsoft 365, si possono aggiungere in Impostazioni → Account. Ognuna appare come casella a sé e invia dal proprio indirizzo.
```

**Português**

```
As caixas de correio partilhadas com a sua conta, como a de uma equipa, também no Microsoft 365, podem ser adicionadas em Definições → Contas. Cada uma aparece como uma caixa própria e envia a partir do seu próprio endereço.
```
