# Brief : versions mobiles des maquettes KyberNow

## Contexte
Maquettes de la refonte du site KyberNow, dans Claude Design (format `.dc.html`). Pages : carrefour (`Main`), Jouer (`Jeu`), Travailler (`Pro`), Télécharger (`Telecharger`), et le guide (`Guide`, `GuideArticle`, `GuideFaq`). Le bureau (≥ 900 px) est validé par l'auteur : **n'y touchez pas**. Votre travail : la version téléphone (cible 390 × 844, à vérifier aussi en 360 et 430 de large), qui doit être aussi soignée, immersive et élégante que le bureau, pas un bureau rétréci.

Fichiers : `/Users/matthieurequenna-cazanave/Sites/kyber-apps/docs/refonte-site-2026-11/design/project/<Page>.dc.html`. Chaque agent ne modifie QUE les pages qui lui sont confiées. Ignorez les fichiers `*Clair.dc.html` (ils seront régénérés à partir des pages sombres).

## Ce qui existe déjà
- Un menu mobile commun (bouton trois traits, feuille plein écran, classes `.burger`, `.sheet`, `.mi`) est déjà en place sous 900 px : gardez-le tel quel, sauf bug.
- Les sections épinglées et les effets au défilement ne s'activent qu'en mode « live » (classe `.live` sur la racine, posée par le script quand la largeur ≥ 900 et la hauteur < 1600). Sur téléphone la page est en mode « repos » : tout est visible, sans épinglage. Vous pouvez ajouter de l'animation mobile légère (apparitions, diaporamas existants, petites boucles), mais rien qui dépende d'un épinglage long ou qui fasse sauter la mise en page.
- Thème clair : la racine prend la classe `.light`. N'écrivez **aucune couleur en dur** : utilisez les jetons (`var(--ink0)` … `var(--frost)`, `var(--mist)`, `var(--sig)`, `var(--soft)`, `var(--sline)`, `rgba(var(--veil),…)`). Les écrans d'apps recréés restent sombres via les règles existantes.
- Le texte est validé par l'auteur : **ne changez aucun mot** (si un libellé doit raccourcir sur mobile, proposez-le dans votre bilan).

## Règles du format Design Component (elles cassent sans bruit)
- Gardez la ligne `<script src="./support.js"></script>` telle quelle ; fermez chaque élément ; guillemets sur chaque attribut.
- `{{ chemin }}` est une simple lecture (pas d'expression) ; calculez dans `renderVals()`.
- Toute l'interface est en balisage `<x-dc>` ; jamais de DOM construit en script (`innerHTML`, `appendChild`).
- Le CSS mobile va dans le `<style>` du `<helmet>`, dans des `@media (max-width: 900px)` (et plus fins si besoin). Un `style="…"` en ligne l'emporte sur une règle de feuille : utilisez `!important` dans la règle mobile ou déplacez ce style dans une classe (sans changer l'aspect bureau).
- Le script : JS classique, `class Component extends DCLogic`. Vérifiez votre JS avec `node --check` (contenu de `<script type="text/x-dc" data-dc-script>`, préfixé de `class DCLogic{}`).

## Ce qu'on attend (à adapter)
- Zéro défilement horizontal, gouttière de 20 px, zones tactiles d'au moins 44 px.
- Typographie mobile : titres d'affichage autour de 38-46 px, corps 16-17 px, `text-wrap: balance` sur les titres.
- Héros : hauteur adaptée à l'écran, photo cadrée sur le sujet (`object-position`), texte lisible sur l'image.
- Rangées de cartes : une colonne, ou un carrousel horizontal accroché (`scroll-snap`) avec un débord visible qui invite à glisser.
- Tableaux comparatifs : vraiment lisibles sur téléphone.
- Sections interactives repensées pour le pouce, pas seulement empilées.
- Respect de `prefers-reduced-motion`.

## Vérifier
Un serveur local rend les pages comme Claude Design : `http://localhost:5210/<Page>.dc.html` (images comprises). N'utilisez PAS le panneau navigateur de l'app (il est partagé). Captures avec Chrome sans fenêtre :
```
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars --virtual-time-budget=4000 --window-size=390,3000 --force-device-scale-factor=1 --screenshot=/Users/matthieurequenna-cazanave/Sites/kyber-apps/docs/refonte-site-2026-11/design/captures/<Page>-1.png "http://localhost:5210/<Page>.dc.html"
```
puis lisez l'image. Pour voir plus bas, augmentez la hauteur (jusqu'à 9000) ou ajoutez `#ancre`. Vérifiez aussi le bureau (`--window-size=1440,3000`) pour être sûr de n'avoir rien cassé. Rangez vos captures dans `design/captures/`.

## Ce que vous rendez
Vos pages modifiées, puis un bilan de dix lignes au plus : ce que vous avez changé, ce dont vous êtes le moins sûr, et toute proposition de libellé plus court.
