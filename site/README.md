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
| `/` | `/en/` | Présentation et téléchargements |
| `/guide/` | `/en/guide/` | Guide pas à pas (serveur, appareils : Mac, Apple TV, navigateur ; VPN, mises à jour, désinstallation) et FAQ |
| `/mentions-legales/` | `/en/legal/` | Éditeur, hébergement, propriété intellectuelle, confidentialité, crédits |

Chaque page déclare ses deux versions (`<link rel="alternate" hreflang>`). `assets/lang.js`, chargé dans `<head>`, envoie un visiteur dont le navigateur n'est pas en français vers la version anglaise (et l'inverse), sauf s'il a choisi une langue avec le sélecteur : ce choix est gardé dans `localStorage` (`kn-lang`). Les robots ne sont jamais redirigés. Les textes produits par `main.js` (trajet, simulation, profils) suivent `<html lang>`.

Les deux langues sont écrites à la main : une modification de texte se reporte dans les deux fichiers.

## Bêta

Tant que KyberNow n'est pas en 1.0, le site le dit partout : badge « Bêta » à côté du logo sur chaque page et en tête de l'accueil (classe `.beta`), encart « KyberNow est en bêta » dans la section Télécharger (`.notice-beta`, icône `flask`), badge après le numéro de version, question « Pourquoi bêta ? » de la FAQ. À la 1.0, retirer ces éléments dans les deux langues.

L'app Apple TV n'a pas de fichier à télécharger : sa carte mène à la section « Sur l'Apple TV » du guide (`/guide/#apple-tv`), qui résume [apps/tvos/SIDELOAD.md](../apps/tvos/SIDELOAD.md) (installation depuis un Mac avec Xcode). À reprendre quand l'app sera sur l'App Store.

## Publication

Hébergé sur GitHub Pages par le workflow `.github/workflows/pages.yml` : chaque push sur `main` qui touche `site/` publie le dossier tel quel.

Mise en route, une seule fois :
1. GitHub › Settings › Pages › Source : « GitHub Actions ».
2. Vérifier le domaine (Settings du compte › Pages › Add a domain › `kybernow.app`, enregistrement TXT chez le registrar) avant de toucher aux DNS, pour éviter qu'un autre compte ne le prenne.
3. DNS de `kybernow.app` : quatre enregistrements `A` (185.199.108.153, 185.199.109.153, 185.199.110.153, 185.199.111.153), les `AAAA` 2606:50c0:8000::153 à 8003::153, et `www` en `CNAME` vers `monsieurnours.github.io`.
4. Settings › Pages › Custom domain : `www.kybernow.app` (les enregistrements `A` de `kybernow.app` font rediriger le domaine nu vers `www`), puis « Enforce HTTPS » dès que le certificat est prêt (jusqu'à 24 h ; `.app` exige le HTTPS).

## Communauté

Serveur Discord KyberNow, invitation permanente : https://discord.gg/sVy2mu5WdT (ouvre #bienvenue). Liens dans la navigation, le menu mobile, le téléchargement, le guide et le pied de page.

## Contenu

| Fichier | Rôle |
|---|---|
| `index.html`, `guide/`, `mentions-legales/`, `en/` | Les pages (voir plus haut) |
| `assets/lang.js` | Choix de la langue (détection, préférence retenue) |
| `assets/doc.css` | Mise en page des pages de lecture (sommaire, étapes, FAQ, mentions) |
| `assets/kybernow.css` | Jetons de la charte v2 (copie de `brand/v2/tokens/kybernow.css`) |
| `assets/site.css` | Mise en page |
| `assets/main.js` | Apparitions au défilement, menu mobile, section épinglée du trajet (compteur, étapes), simulation QUIC / flux unique, profils, bouton principal selon le système du visiteur, version lue sur GitHub ; textes en français ou en anglais selon la page |
| `assets/scene.js` | Scène 3D (Three.js) : symbole « Relais » extrudé depuis la géométrie de `brand/v2/tools/geometry.json`, couture en néon (bloom), flux de particules ; un état par section (`data-scene`) |
| `assets/icons.svg` | Icônes Phosphor 2.1.1 (MIT, `assets/PHOSPHOR-LICENSE`) en sprite |
| `assets/img/` | Logo et symboles (copies de `brand/v2/`) |
| `fonts/` | Archivo et Martian Mono (OFL) |
| `vendor/three/` | Three.js r170 (MIT) : `three.module.min.js` et les quelques modules d'exemples utilisés (bloom, SVGLoader, RoomEnvironment) |

Rien n'est chargé depuis un CDN. Seule requête externe : l'API GitHub, pour afficher la dernière version et la taille des fichiers (le texte statique reste si elle échoue).

## Règles de la charte appliquées

- Néon allumé seulement dans les moments actifs : allumage d'une seconde à l'arrivée, flux de l'image, mesure du trajet, ré-allumage au téléchargement. Éteint dans les sections de lecture.
- Sur téléphone, le symbole 3D se range en haut de l'écran, plus petit (`MOBILE` dans `scene.js`), pour ne jamais passer sous le texte.
- Un seul bouton principal par vue ; au téléchargement, il suit le système du visiteur (Mac : l'app, Windows : l'installateur, tablette, téléphone et le reste : le client web `/ecran`).
- Aucune limite de définition ou de fréquence annoncée : le site parle de « la définition et la fréquence de votre écran ». Les seuls chiffres sont ceux d'une mesure, présentés comme tels.
- « Réduire les animations » : le symbole ne voyage plus, pas de grésillement ni de respiration, apparitions en simple fondu. « Réduire la transparence » : surfaces opaques, pas de halo.
- Sans WebGL : symbole allumé en SVG à la place de la scène.

## Mettre à jour

- Chiffres du trajet : `docs/prototype-A-results.md` (section `#trajet` de `index.html`, total dans `TOTAL` de `main.js`).
- Liens de téléchargement : `releases/latest/download/<fichier>` sur GitHub, rien à changer à chaque version.
- Three.js : remplacer les fichiers de `vendor/three/` par ceux de la même version sur unpkg (`three@<version>/build/three.module.min.js`, `examples/jsm/...`).
