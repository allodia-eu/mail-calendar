# HTML mail renders with its own styling again

Platforms: all
Bump: patch

> Every HTML message rendered unstyled, on every client, and every inline image was blocked with
> it: the base stylesheet, the message's `<style>` block and its inline styles all had no effect,
> so mail came out as browser-default serif on a bare page. The reading document's
> `Content-Security-Policy` separated two of its directives with a comma, and a `content`
> attribute is a policy *list*, so that started a second policy enforced alongside the first. The
> document got the intersection of the two, which put `default-src 'none'` in one policy and the
> `style-src 'unsafe-inline'` and `img-src data:` meant to soften it in the other, and both fell
> back to `'none'`. The separator is now a semicolon, and
> `the_document_carries_one_policy_rather_than_two` asserts on the separators, which is what the
> existing per-directive assertions structurally could not see.

**English**

```
HTML messages show their own layout, fonts and inline images again, instead of plain unstyled text.
```

**Nederlands**

```
HTML-berichten tonen weer hun eigen opmaak, lettertypen en ingesloten afbeeldingen, in plaats van kale tekst zonder opmaak.
```

**Deutsch**

```
HTML-Nachrichten zeigen wieder ihr eigenes Layout, ihre Schriften und eingebetteten Bilder statt reinem Text ohne Gestaltung.
```

**Français**

```
Les messages HTML retrouvent leur mise en page, leurs polices et leurs images intégrées, au lieu d'un texte brut sans mise en forme.
```

**Español**

```
Los mensajes HTML vuelven a mostrar su propio diseño, sus tipografías y sus imágenes insertadas, en lugar de texto sin formato.
```

**Italiano**

```
I messaggi HTML mostrano di nuovo il proprio impaginato, i caratteri e le immagini incorporate, invece di testo senza formattazione.
```

**Português**

```
As mensagens HTML voltam a mostrar o seu próprio grafismo, tipos de letra e imagens incorporadas, em vez de texto sem qualquer formatação.
```
