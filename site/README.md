# www.kybernow.app

[English](README.en.md) · Français

Site de présentation et de téléchargement de KyberNow. Statique, sans npm ni étape de compilation : on sert le dossier tel quel.

```sh
python3 -m http.server 5200 --directory site
```

(ou la configuration `site` de `.claude/launch.json`, qui désactive le cache). Les chemins sont absolus (`/assets/…`) : le site doit être servi à la racine d'un domaine, et `file://` ne fonctionne pas.

## Pages et langues

| Français | Anglais | Contenu |
|---|---|---|
| `/` | `/en/` | Accueil, côté produit : vidéo de présentation, les trois apps avec leurs écrans, Link sur chaque appareil, fonctions phares, preuves que c'est un produit réel, trois étapes, téléchargements |
| `/technologie/` | `/en/technology/` | Le trajet d'une image (9,4 ms, section épinglée), QUIC (simulation), Kyber, la comparaison avec Sunshine + Moonlight, les fonctions et les profils |
| `/guide/` | `/en/guide/` | Guide pas à pas (KyberNow Station, puis les appareils avec KyberNow Link : Mac, Windows, Linux, Apple TV, iPhone et iPad, Android, téléviseur LG, Steam Deck et SteamOS, navigateur, chacun derrière sa tuile ; VPN, mises à jour, désinstallation) et FAQ |
| `/mentions-legales/` | `/en/legal/` | Éditeur, hébergement, propriété intellectuelle, confidentialité, crédits |

Chaque page déclare ses deux versions (`<link rel="alternate" hreflang>`). `assets/lang.js`, chargé dans `<head>`, envoie un visiteur dont le navigateur n'est pas en français vers la version anglaise (et l'inverse), sauf s'il a choisi une langue avec le sélecteur : ce choix est gardé dans `localStorage` (`kn-lang`). Les robots ne sont jamais redirigés. Les textes produits par `main.js` (trajet, simulation, profils) suivent `<html lang>`.

Les deux langues sont écrites à la main : une modification de texte se reporte dans les deux fichiers.

## Produits et logos

KyberNow est la marque ; elle regroupe trois produits, nommés comme dans le reste du dépôt :

- **KyberNow Station** : le logiciel installé sur l'ordinateur Windows qui diffuse l'image (`KyberNow-Station-Setup.exe`). Dans les textes, « la Station » après la première mention ; « l'ordinateur » reste quand on parle de la machine.
- **KyberNow Admin** : l'administration de la Station dans le navigateur (« Admin », sans article, après la première mention).
- **KyberNow Link** : les apps qui affichent l'image (Mac, Windows, Linux et Steam Deck, Apple TV, iPhone et iPad, Android, téléviseurs LG) et leur version web `/ecran` (`KyberNow-Link-Setup.exe` pour Windows, `KyberNow-Link-macOS.zip` pour le Mac, `KyberNow-Link-Android.apk` pour Android, `KyberNow-Link-webOS.ipk` pour les téléviseurs LG, `KyberNow-Link-Linux-x86_64.flatpak` et `KyberNow-Link-Linux-aarch64.flatpak` pour Linux et le Steam Deck).

L'accueil les présente dans la section « La famille KyberNow » (`#apps`, juste après la vidéo), chacun avec son logo signé et son écran recréé en HTML (voir « Écrans des apps ») (`assets/img/kybernow-{station,admin,link}-dark.svg`, copies de `brand/v2/logo/`). Les cartes de téléchargement et l'encart « Avant de commencer » du guide reprennent les logos de Station et de Link. Un logo signé ne descend pas sous 32 px de haut (en dessous, la signature n'est plus lisible) ; le site n'a qu'un thème sombre, seules les versions `-dark` sont copiées.

## Accueil : vidéo, écrans des apps, preuves

Refonte du 24 octobre 2026 : l'accueil parle produit, la technique est sur `/technologie/`. Dans l'ordre : hero (l'app Mac et l'iPhone derrière le texte, symbole 3D en retrait), **la présentation** (`#presentation`), **les trois apps** (`#apps`), **Link sur chaque appareil** (`#appareils`), **fonctions phares** (`#fonctions` : définition et fréquence de l'écran, son 7.1, fréquence variable, presse-papiers et 4:4:4, toutes les manettes, chiffrement ; puis « l'ordinateur devient une console », l'app lancée au début de la session selon l'appareil, et la navigation à la manette intégrée), **déjà entre vos mains** (`#reel` : dernière version, nombre de versions, latence mesurée, licence, signatures, mises à jour, communauté), trois étapes et résumé technique (`#comment`), téléchargements (`#telecharger`, grille compacte à trois colonnes, bouton principal selon l'appareil).

- **Vidéo de présentation** (motion design, sans voix ni son) : `<video muted playsinline loop>` dans `.video`, fichiers `assets/video/presentation.webm` et `presentation.mp4` (à déposer : ils ne sont pas encore dans le dépôt, la figure garde son image d'attente tant qu'ils manquent), image d'attente `assets/video/presentation-poster.jpg` rendue par `scripts/site-seo.py images` depuis `scripts/site-og/video-poster.html`. `main.js` la lance quand elle arrive à l'écran (45 % visible) et la met en pause quand elle le quitte ; boutons pause, revoir, plein écran ; avec « Réduire les animations », rien ne démarre seul. Le bouton « Voir la présentation » du hero fait défiler jusqu'à elle.
- **Écrans des apps** : recréés en HTML et CSS (`assets/screens.css`, préfixe `.sc-`) d'après le code des apps (chaînes d'Admin et de l'installateur, accueil du Mac, captures du téléviseur LG et d'Android), jamais des captures d'écran : nets à toute taille, sans rien à refaire à chaque version. Chaque écran est dessiné à sa taille réelle (`--w`, `--h` sur `.shot`) et `main.js` le réduit à la largeur de sa carte (`--s`, `transform: scale`). Décoratifs : `aria-hidden`, décrits par l'`aria-label` de `.shot`. Écrans : Link Mac (accueil), Link iPhone, Link au salon (téléviseur), Link sur le Steam Deck (mode Jeu), Link en session (barre de mesures), KyberNow Admin (vue d'ensemble), l'installateur de KyberNow Station (Bienvenue). Les données sont celles d'un ordinateur d'exemple, « Atelier » (RTX 3080 Ti, celui de la mesure). Écrits à la main dans les deux langues, comme le reste.
- **Preuves** : les chiffres (dernière version, date, nombre de versions) sont écrits dans la page et mis à jour par `main.js` depuis l'API GitHub (`releases?per_page=100`) ; à refaire à la main de temps en temps pour que le repli reste juste.
- **Scène 3D** : gardée, discrète. États `hero-soft`, `edge-l`, `edge-r` (symbole au bord de l'écran, petit, éteint) et `assemble-soft` (téléchargement) dans `scene.js` ; les états riches (`flow` pour le trajet, `side`…) restent sur `/technologie/`.
- **KyberLink OS** (Raspberry Pi, image à flasher) : pas encore publié, un commentaire marque sa place dans la section Télécharger. À ajouter aussi dans « Sur vos appareils » quand il arrivera.

## Bêta

Tant que KyberNow n'est pas en 1.0, le site le dit partout : badge « Bêta » à côté du logo sur chaque page et en tête de l'accueil (classe `.beta`), encart « KyberNow est en bêta » dans la section Télécharger (`.notice-beta`, icône `flask`), badge après le numéro de version, question « Pourquoi bêta ? » de la FAQ. À la 1.0, retirer ces éléments dans les deux langues.

L'app KyberNow Link pour Apple TV n'a pas de fichier à télécharger : sa carte mène à la section « Sur l'Apple TV » du guide (`/guide/#apple-tv`), qui résume [apps/tvos/SIDELOAD.md](../apps/tvos/SIDELOAD.md) (installation depuis un Mac avec Xcode). De même pour l'app Link pour iPhone et iPad : carte vers « Sur l'iPhone et l'iPad » (`/guide/#iphone`), qui résume [apps/ios/SIDELOAD.md](../apps/ios/SIDELOAD.md). À reprendre quand les apps seront sur TestFlight ou l'App Store. L'app Android (Android 11 ou plus : téléphone, tablette, téléviseur) se télécharge en APK ; sa carte mène aussi au guide (`/guide/#android`) pour l'installation au téléviseur, par `adb` depuis un ordinateur. `#android-tv`, l'ancre des premiers liens, ouvre la même section. L'app des téléviseurs LG (webOS 5 ou plus) s'installe en mode développeur depuis un ordinateur : sa carte mène à « Sur un téléviseur LG » (`/guide/#lg`), qui résume [apps/webos/INSTALLATION.md](../apps/webos/INSTALLATION.md) ; un visiteur dont le navigateur est celui d'un téléviseur LG (`Web0S` dans l'agent) voit cette carte mise en avant. L'app Linux se télécharge en Flatpak (`x86_64`, et `aarch64` en second lien) ; sa carte mène aussi à « Sur le Steam Deck et SteamOS » (`/guide/#steam-deck`). Le guide a deux tuiles, « Linux » (`#linux`, le bureau) et « Steam Deck » (`#steam-deck` : mode Bureau, ajout à Steam, mode Jeu, salon), qui résument [apps/linux/INSTALLATION.md](../apps/linux/INSTALLATION.md). Un navigateur sous Linux met la carte en avant ; dans le guide, celui de Steam (`Valve Steam` dans l'agent) ouvre la tuile du Steam Deck, les autres celle de Linux.

**Connecter un appareil** (guide) : une tuile par système (`.os-tiles`, rôle `tablist`), son installation dessous (`.os-panel`). `main.js` ouvre celle de l'ancre (`#mac`, `#android`…, liens existants compris), sinon celle de l'appareil du visiteur ; flèches du clavier entre les tuiles. Sans JavaScript, toutes les installations restent à la suite. Un nouveau système : une tuile et un panneau dans les deux langues ; une dernière tuile seule sur sa ligne prend toute la largeur (`doc.css`).

## Publication

Hébergé sur GitHub Pages par le workflow `.github/workflows/pages.yml` : chaque push sur `main` qui touche `site/` publie le dossier tel quel.

Mise en route, une seule fois :
1. GitHub › Settings › Pages › Source : « GitHub Actions ».
2. Vérifier le domaine (Settings du compte › Pages › Add a domain › `kybernow.app`, enregistrement TXT chez le registrar) avant de toucher aux DNS, pour éviter qu'un autre compte ne le prenne.
3. DNS de `kybernow.app` : quatre enregistrements `A` (185.199.108.153, 185.199.109.153, 185.199.110.153, 185.199.111.153), les `AAAA` 2606:50c0:8000::153 à 8003::153, et `www` en `CNAME` vers `monsieurnours.github.io`.
4. Settings › Pages › Custom domain : `www.kybernow.app` (les enregistrements `A` de `kybernow.app` font rediriger le domaine nu vers `www`), puis « Enforce HTTPS » dès que le certificat est prêt (jusqu'à 24 h ; `.app` exige le HTTPS).

## Référencement

Cible : qu'une recherche de jeu à distance, de bureau à distance, de « cloud gaming » chez soi ou d'alternative à Sunshine et Moonlight mène au site, dans les moteurs comme dans les assistants IA.

- **Message agnostique** : les textes de présentation et de partage (image, descriptions, données structurées, `llms.txt`) disent « votre ordinateur sur tous vos appareils », jamais une liste de plateformes. Les plateformes réelles n'apparaissent que là où l'on installe : section Télécharger et guide.
- **En-tête de chaque page** : titre et description avec les mots recherchés, URL canonique, Open Graph complet (type, nom du site, langue et langue alternative, image 1200 × 630 avec texte alternatif) et carte Twitter `summary_large_image` ; `apple-touch-icon.png` pour l'écran d'accueil iPhone et iPad. Les mentions légales restent en `noindex`.
- **Images de partage** : `assets/og/kybernow-fr.jpg` et `kybernow-en.jpg`, composées en HTML dans `scripts/site-og/card.html` (polices, logo et symbole allumé du site) et rendues par `scripts/site-seo.py images` avec Chrome sans fenêtre (`CHROME=<chemin>` pour un autre Chromium) ; le même script rend l'image d'attente de la vidéo. Les refaire quand le message, la latence mesurée ou les plateformes changent.
- **Données structurées** (JSON-LD) : sur l'accueil, `SoftwareApplication` « KyberNow » (catégorie, systèmes, gratuité, licence, téléchargement, fonctions, `subjectOf` la page Technologie, `isBasedOn` Kyber) avec ses parties `#station` (KyberNow Station, Windows, qui contient la `WebApplication` KyberNow Admin) et `#link` (KyberNow Link, macOS, Windows, tvOS, iOS, iPadOS, navigateur), `WebSite` et `Organization` (Les Lignes Claires), écrits à la main dans les deux langues ; sur les guides, `FAQPage`, produit par `scripts/site-seo.py faq` à partir des questions visibles. **Relancer `scripts/site-seo.py faq` après toute modification de la FAQ.**
- **Robots** : `robots.txt` ouvre tout, assistants IA compris, et annonce `sitemap.xml` (accueil, technologie et guide, avec leurs versions de langue ; date `lastmod` à avancer quand le texte change). `llms.txt` résume KyberNow pour les assistants IA (en anglais, avec les faits mesurés et la comparaison) : à tenir à jour avec la page d'accueil.
- **Textes** : la section Comparer (sur `/technologie/`, entrée « Comparer » de la navigation) s'intitule « KyberNow, l'alternative à Sunshine + Moonlight » ; la FAQ répond à « peut-il remplacer Sunshine et Moonlight », « est-ce du cloud gaming », « jouer à ses jeux PC sur Mac ou Apple TV ». `lang.js` ne redirige jamais un robot.

## Communauté

Serveur Discord KyberNow, invitation permanente : https://discord.gg/sVy2mu5WdT (ouvre #bienvenue). Liens dans la navigation, le menu mobile, le téléchargement, le guide et le pied de page.

## Contenu

| Fichier | Rôle |
|---|---|
| `index.html`, `technologie/`, `guide/`, `mentions-legales/`, `en/` | Les pages (voir plus haut) |
| `assets/lang.js` | Choix de la langue (détection, préférence retenue) |
| `assets/doc.css` | Mise en page des pages de lecture (sommaire, étapes, FAQ, mentions) |
| `assets/kybernow.css` | Jetons de la charte v2 (copie de `brand/v2/tokens/kybernow.css`) |
| `assets/site.css` | Mise en page |
| `assets/main.js` | Apparitions au défilement, menu mobile, vidéo de présentation, mise à l'échelle des écrans des apps, section épinglée du trajet (compteur, étapes), simulation QUIC / flux unique, profils, bouton principal selon le système du visiteur, version et nombre de versions lus sur GitHub ; textes en français ou en anglais selon la page |
| `assets/screens.css` | Les écrans des apps recréés en HTML (voir « Accueil ») |
| `assets/video/` | Image d'attente de la vidéo de présentation ; les fichiers `presentation.webm` et `presentation.mp4` sont à y déposer |
| `assets/scene.js` | Scène 3D (Three.js) : symbole « Relais » extrudé depuis la géométrie de `brand/v2/tools/geometry.json`, couture en néon (bloom), flux de particules ; un état par section (`data-scene`) |
| `assets/icons.svg` | Icônes Phosphor 2.1.1 (MIT, `assets/PHOSPHOR-LICENSE`) en sprite, plus quelques tracés simples dans le même style (lecture, pause, plein écran, roue, plus…) pour l'accueil |
| `assets/og/` | Images de partage, française et anglaise (voir « Référencement ») |
| `apple-touch-icon.png`, `robots.txt`, `sitemap.xml`, `llms.txt` | Icône iPhone et iPad, robots, plan du site, résumé pour les assistants IA |
| `assets/img/` | Copies de `brand/v2/` : logo (`kybernow-logo-dark.svg`), logos signés des trois produits (`kybernow-{station,admin,link}-dark.svg`), symboles (`kybernow-mark-now-dark.svg` pour l'icône iPhone et iPad, `kybernow-mark-now-lit-dark.svg` sans WebGL et pour l'image de partage) |
| `fonts/` | Archivo et Martian Mono (OFL) |
| `vendor/three/` | Three.js r170 (MIT) : `three.module.min.js` et les quelques modules d'exemples utilisés (bloom, SVGLoader, RoomEnvironment) |
| `updates/appcast.xml` | Flux des mises à jour de l'app Mac (voir « Mettre à jour ») |

Rien n'est chargé depuis un CDN. Seule requête externe : l'API GitHub, pour afficher la dernière version et la taille des fichiers (le texte statique reste si elle échoue). La taille est donnée comme le système l'affichera une fois le fichier téléchargé : en unités de 1 048 576 octets, tronquée, pour les `.exe` (Windows), en millions d'octets pour le reste (macOS), à une décimale ; les valeurs de secours de `index.html` sont à refaire à chaque version qui change nettement de taille.

## Règles de la charte appliquées

- Néon allumé seulement dans les moments actifs : allumage d'une seconde à l'arrivée, flux de l'image, mesure du trajet, ré-allumage au téléchargement. Éteint dans les sections de lecture.
- Sur téléphone, le symbole 3D se range en haut de l'écran, plus petit (`MOBILE` dans `scene.js`), pour ne jamais passer sous le texte.
- Les écrans des apps et la vidéo passent devant le symbole, jamais l'inverse.
- Un seul bouton principal par vue ; au téléchargement, il suit le système du visiteur (Mac : l'app KyberNow Link, Windows : l'installateur de KyberNow Station, tablette, téléphone et le reste : Link dans le navigateur, `/ecran`).
- Définition et fréquence : le site parle de « la définition et la fréquence de votre écran » et, depuis la refonte d'octobre 2026, cite des exemples (4K à 120 Hz, 5K, 6K, 240 Hz) en précisant que la seule limite est l'encodeur de la carte graphique. Les chiffres de latence sont ceux d'une mesure, présentés comme tels.
- « Réduire les animations » : le symbole reste en place, pas de grésillement ni de respiration, apparitions en simple fondu. « Réduire la transparence » : surfaces opaques, pas de halo.
- Sans WebGL : symbole allumé en SVG à la place de la scène.

## Mettre à jour

- Chiffres du trajet : `docs/prototype-A-results.md` (section `#trajet` de `technologie/index.html`, total dans `TOTAL` de `main.js`, et le hero de l'accueil).
- Vidéo de présentation : déposer `presentation.webm` et `presentation.mp4` dans `assets/video/` (sans piste son, 16:9) ; la durée « une minute trente » est écrite dans le titre de la section et dans `scripts/site-og/video-poster.html`.
- Liens de téléchargement : `releases/latest/download/<fichier>` sur GitHub, rien à changer à chaque version. Un lien vers un nouveau nom de fichier n'arrive sur `main` qu'une fois publiée la version qui contient ce fichier (voir [docs/releases.md](../docs/releases.md), « Anciens noms des installateurs »).
- `updates/appcast.xml` : flux des mises à jour de l'app Mac (Sparkle), toutes les versions, bêtas comprises. Écrit et commité par `scripts/release.sh` à chaque publication, reconstruit au besoin par `scripts/rebuild-appcast.sh` ; ne pas le modifier à la main (voir [docs/releases.md](../docs/releases.md), « Flux des mises à jour de l'app Mac »). Son adresse est compilée dans l'app : ne pas le déplacer.
- Three.js : remplacer les fichiers de `vendor/three/` par ceux de la même version sur unpkg (`three@<version>/build/three.module.min.js`, `examples/jsm/...`).
