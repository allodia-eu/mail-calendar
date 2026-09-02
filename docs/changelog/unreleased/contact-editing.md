# Adding and editing contacts

Platforms: all
Bump: minor

> Contacts have been readable since 0.6; this makes them writable. The shape follows from the one
> thing the list already does: a row is a **person**, which the engine assembled from the cards
> several accounts hold. A write cannot be a person, so an edit names a card, the client asks which
> account when there is more than one, and the form is seeded from that card rather than from the
> merged detail on screen. The create destination is a writable address book and nothing else, so a
> user with none is offered no create at all rather than a save that fails on the server after they
> have typed everything in. A patch carries only the fields that changed, so an edited phone number
> cannot quietly strip an address's work label, an organisation's departments, or a postal address
> and photo the form never showed. Deleting a contact is deliberately not here: it is the one
> contacts write nobody can undo, and it wants its own confirmation.

**English**

```
Add and edit contacts. Fill in a name, organisation, role, addresses and numbers, and save into any
address book your accounts can write to. Editing a person who is in two accounts asks which one you
mean, and changes nothing you did not touch.
```

**Nederlands**

```
Contacten toevoegen en wijzigen. Vul een naam, organisatie, functie, adressen en nummers in en sla
op in elk adresboek waar je accounts naartoe kunnen schrijven. Bij een contact in twee accounts
vraagt de app welke je bedoelt, en verandert er niets wat je niet hebt aangeraakt.
```

**Deutsch**

```
Kontakte hinzufügen und bearbeiten. Tragen Sie Name, Organisation, Position, Adressen und Nummern
ein und sichern Sie in jedes Adressbuch, in das Ihre Konten schreiben dürfen. Bei einem Kontakt in
zwei Konten fragt die App, welches Sie meinen, und ändert nichts, was Sie nicht angefasst haben.
```

**Français**

```
Ajoutez et modifiez des contacts. Saisissez un nom, une organisation, une fonction, des adresses et
des numéros, puis enregistrez dans n'importe quel carnet d'adresses où vos comptes peuvent écrire.
Pour un contact présent dans deux comptes, l'app demande lequel vous visez, et ne change rien
d'autre.
```

**Español**

```
Añade y edita contactos. Escribe un nombre, una organización, un cargo, direcciones y números, y
guarda en cualquier libreta de direcciones en la que tus cuentas puedan escribir. Si el contacto
está en dos cuentas, la app te pregunta a cuál te refieres y no cambia nada que no hayas tocado.
```

**Italiano**

```
Aggiungi e modifica i contatti. Inserisci nome, organizzazione, ruolo, indirizzi e numeri e salva in
qualsiasi rubrica su cui i tuoi account possono scrivere. Se il contatto è in due account, l'app
chiede quale intendi e non cambia nulla che tu non abbia toccato.
```

**Português**

```
Adicione e edite contactos. Introduza um nome, uma organização, um cargo, endereços e números e
guarde em qualquer livro de endereços onde as suas contas possam escrever. Num contacto que esteja
em duas contas, a aplicação pergunta qual pretende e não altera nada em que não tenha tocado.
```
