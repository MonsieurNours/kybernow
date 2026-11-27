# Brief de rédaction : site KyberNow (maquettes)

## Le produit
KyberNow affiche un ordinateur sur un autre écran et permet de le prendre en main, chez soi ou loin de chez soi (jeu à distance, poste de travail déporté). Libre, gratuit, en français et en anglais. Quatre apps : KyberNow Station (sur l'ordinateur à retrouver : Windows, Mac, Linux), KyberNow Link (sur l'appareil devant soi : télé, tablette, téléphone, Steam Deck, autre ordinateur, navigateur, Raspberry Pi avec KyberNow Link OS), KyberNow Share (faire entrer quelqu'un : invité, spectateur, prêt de l'ordinateur), KyberNow Admin (réglages de la Station). Le site a deux entrées : Jouer (particuliers, violet) et Travailler (pros : création, station déportée, studios de jeu, équipes, bleu).

## Ce que l'auteur reproche aux textes actuels
« Ça fait encore trop IA. » Il veut une langue accessible mais d'un bon niveau, sans surexplication. Exemple de correction qu'il a faite lui-même :
- avant : « Les mêmes apps font les deux. Choisissez la porte qui vous ressemble, vous passerez d'une page à l'autre quand vous voudrez. »
- après : « Les mêmes apps font les deux. Choisissez l'usage qui vous ressemble. »
Autre correction : « Gratuit, pour de bon » → « Gratuit ».

## Le ton visé : celui de Playruo (projet dérivé du même moteur, Kyber)
Analyse du site playruo.com :
- Direct et sûr de lui, chaleur discrète, une pointe de légèreté, humour rare.
- Titres courts (3 à 8 mots), affirmations concrètes ; titres de fonctions = substantifs concrets (« Data Accuracy », « Security ») ; titre principal en deux temps (« Showcase games, engage players »).
- Phrases de corps variées, paires parallèles (« No downloads, no installs »), questions seulement en accroche finale (« Ready for a demo? »).
- Jargon technique assumé mais suivi d'une explication utile ; chiffres précis.
- Petites tournures conversationnelles (« you set up your demo, we do the rest »).
- À NE PAS reprendre de Playruo : « seamless », « groundbreaking », superlatifs vides, triades systématiques.

## Règles du site (obligatoires)
Lisez /Users/matthieurequenna-cazanave/Sites/kyber-apps/docs/ton-du-site.md en entier : ses règles priment (vouvoiement, typographie française avec espaces fines insécables `&#8239;` avant ; : ! ? et dans « », `&nbsp;` entre nombre et unité, virgule décimale, majuscule en début de titre seulement, pas de tiret cadratin).
En plus :
- Zéro formule « IA » : pas de « Ce qui change, c'est… », « Mesuré, pas promis », pas d'antithèse « pas X, mais Y », pas de titre-slogan creux, pas de deux-points à effet, pas de personnification, pas de triade automatique, pas de « fluide », « sans effort », « en toute simplicité », « révolutionnaire », « nouvelle génération ».
- Ne décrivez jamais la page elle-même (« touchez un appareil pour voir… », « faites défiler »). Exception : les libellés de boutons et d'onglets.
- Message agnostique : « votre ordinateur sur tous vos appareils » ; pas de liste de plateformes dans les accroches (les plateformes réelles restent dans les fiches de téléchargement et les cartes d'appareils).
- Ne présentez jamais une fonction comme « pas encore essayée » ou « nouvelle ».
- Ne modifiez AUCUN fait : chiffres (9,4 ms, 5,19 ms…, 4K, 5K à 60 Hz, 4K à 160 Hz, 3840 × 2160…), noms de produits, prérequis (versions de systèmes, modèles), noms de fichiers, comparaisons Parsec et Sunshine/Moonlight. Vous pouvez les reformuler, pas les changer, ni en ajouter d'inventés.
- Les textes des écrans d'apps recréés (petites maquettes d'interface : « Installation de KyberNow Station », « Code de l'appareil », « Accepter », « PC du salon », « STATION-RENDU-02 », la console d'équipe, le terminal) ne sont pas à réécrire.
- Gardez la longueur ou raccourcissez. Pas de texte plus long qu'avant, sauf correction d'une phrase bancale.

## Fichiers
Cinq pages, à lire en entier (HTML ; les textes sont dans le balisage et dans les chaînes du script en bas de chaque fichier : scènes des héros, appareils, usages, étapes…) :
/private/tmp/claude-501/-Users-matthieurequenna-cazanave--claude-spares-spare-WKOkr0/4a6f7a67-54d0-4c5f-8020-0893765f2e82/scratchpad/design/project/Main.dc.html (carrefour)
/private/tmp/claude-501/-Users-matthieurequenna-cazanave--claude-spares-spare-WKOkr0/4a6f7a67-54d0-4c5f-8020-0893765f2e82/scratchpad/design/project/Jeu.dc.html (Jouer)
/private/tmp/claude-501/-Users-matthieurequenna-cazanave--claude-spares-spare-WKOkr0/4a6f7a67-54d0-4c5f-8020-0893765f2e82/scratchpad/design/project/Pro.dc.html (Travailler)
/private/tmp/claude-501/-Users-matthieurequenna-cazanave--claude-spares-spare-WKOkr0/4a6f7a67-54d0-4c5f-8020-0893765f2e82/scratchpad/design/project/Telecharger.dc.html (Télécharger)
Ne modifiez pas ces fichiers vous-même.

## Ce que vous rendez
Un fichier JSON par page dans /private/tmp/claude-501/-Users-matthieurequenna-cazanave--claude-spares-spare-WKOkr0/4a6f7a67-54d0-4c5f-8020-0893765f2e82/scratchpad/design/copy/ , nommé `<Page>.<etape>.json` (étape donnée dans votre consigne), contenant une liste :
`[{"old": "...", "new": "...", "pourquoi": "trois à dix mots"}]`
- `old` est copié À L'IDENTIQUE depuis le fichier (entités HTML comprises, ex. `&#8239;`, `&nbsp;`, balises `<em>` incluses si elles sont dans le passage), et n'apparaît qu'UNE fois dans le fichier. Prenez un passage assez long pour être unique, mais restez dans un seul nœud de texte ou une seule chaîne JS.
- `new` garde les mêmes balises internes (`<em>`, `<b>`, `<span class="w" …>`) quand il y en a, et les mêmes entités typographiques. Dans une chaîne JS entre apostrophes simples, n'utilisez pas d'apostrophe droite non échappée : écrivez ’ (apostrophe typographique) ou gardez les guillemets d'origine.
- Ne touchez que les textes qui le méritent. Mieux vaut 25 vraies améliorations que 80 retouches.
Terminez par un court bilan (dix lignes) : les principes que vous avez appliqués et les trois changements dont vous êtes le plus content.
