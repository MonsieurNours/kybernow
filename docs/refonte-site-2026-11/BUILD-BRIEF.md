# Construire le site : cahier commun

La refonte du site KyberNow est validée en maquettes (Claude Design) : `docs/refonte-site-2026-11/design/project/*.dc.html` (pages sombres ; `*Clair` et `*Mobile` sont des copies). On la construit maintenant dans `site/`, le vrai site : **statique, sans npm ni étape de compilation**, servi tel quel (GitHub Pages). Un serveur local le sert : `http://localhost:5200/` (configuration `site` de `.claude/launch.json`, déjà lancée).

**Ne commitez rien, ne poussez rien** : un push sur `main` publie le site. Ne touchez qu'aux fichiers de votre lot.

## Règles de fond
- Textes : ceux des maquettes, **mot pour mot** (validés par l'auteur après trois passes de rédacteurs). Les maquettes calculent certains textes dans leur script (`renderVals()` : scènes des héros, appareils, usages, étapes, droits…) : reprenez-les dans le HTML (contenu indexable) ou dans le script de page, sans les changer.
- Ton et typographie : `docs/ton-du-site.md` (FR et EN). Anglais britannique, écrit comme un rédacteur anglophone l'écrirait, même voix (direct, concret, sans « seamless », « effortless », « next-gen », sans antithèse « not X, but Y »).
- Message agnostique : « votre ordinateur sur tous vos appareils » ; les plateformes réelles seulement là où l'on installe.
- Tout est présenté comme disponible (choix de l'auteur).

## Pages et adresses
| Univers (`data-u`) | Français | Anglais | Maquette |
|---|---|---|---|
| carrefour | `/` | `/en/` | `Main.dc.html` |
| jeu | `/jeu/` | `/en/play/` | `Jeu.dc.html` |
| pro | `/pro/` | `/en/pro/` | `Pro.dc.html` |
| commun | `/telecharger/` | `/en/download/` | `Telecharger.dc.html` |
| commun | `/guide/`, `/guide/<slug>/`, `/guide/faq/` | `/en/guide/…` | `Guide*.dc.html` (générés par `scripts/site-guide.py`) |
| commun | `/share/`, `/technologie/`, `/mentions-legales/` | `/en/share/`, `/en/technology/`, `/en/legal/` | pages existantes, à rhabiller |

Liens entre pages : remplacez les `Xxx.dc.html` des maquettes par ces adresses (avec `/en/…` dans la version anglaise). Les liens de téléchargement réels sont dans la maquette Télécharger (`https://github.com/MonsieurNours/kybernow/releases/latest/download/…`). Contact pro : `hello@kybernow.app` (affichée en texte sélectionnable, lien `mailto:`).

## Fichiers communs (déjà écrits, ne pas modifier sans le dire)
- `site/assets/kn/base.css` : jetons (sombre par défaut, `[data-theme="light"]`), univers (`data-u`), boutons (`.btn`, `.btn.sig`), en-tête (`.top`, `.logo`, `.nav`, `.tools`, `.thm`), menu mobile (`.burger`, `.sheet`, `.mi`), sections (`.sec`, `.sechead`, `.k`, `.disp`, `.lead`), pied de page (`.ft`), apparitions (`.rv`), îlots sombres en thème clair (`.dark-island` : à poser sur les écrans d'apps recréés), mode réduit des animations.
- `site/assets/kn/theme.js` (dans `<head>`), `site/assets/kn/base.js` (fin de `<body>`) : thème, menu, langue, classe `live` sur `<html>` (largeur ≥ 900 px, animations permises), `KN.pin(section, hauteurDeScène, rappel(p))` pour les sections épinglées (pose `--p` de 0 à 1 sur la section en mode live, l'enlève sinon), `KN.onView(el, rappel, seuil)`, redirection des anciennes ancres.
- Logos : `site/assets/img/logo/{logo,station,link,share,admin}-{spectre,glacier}{,-clair}.svg`. Sombre : `-spectre`/`-glacier` ; clair : `-spectre-clair`/`-glacier-clair`. Deux `<img>` côte à côte, classes `dk` et `lt` (base.css cache celui de l'autre thème).
- Icônes : `site/assets/icons.svg` (`<svg class="ic"><use href="/assets/icons.svg#discord-logo"/></svg>` ; liste : `grep -o 'id="[a-z-]*"' site/assets/icons.svg`).
- Images : `site/assets/img/scenes/`, `site/assets/img/pi/`, `site/assets/img/pro/` (photos de la page Travailler : `ipad`, `deportee`, `sculpt`, `equipe`, `jeustudio`, `creation`, en `.webp` 1536 et `-960.webp`).

## Par page
- `site/<chemin>/index.html` (FR) et `site/en/<chemin>/index.html` (EN).
- `site/assets/kn/<page>.css` et, si la page en a besoin, `site/assets/kn/<page>.js` (JavaScript ordinaire, sans dépendance, communs aux deux langues : les textes que le script affiche viennent du HTML ou d'attributs `data-`, ou d'un petit dictionnaire `{fr, en}` choisi par `KN.fr`).
- Reprenez le CSS de la maquette (bureau, mobile, thème clair) en remplaçant le `.pg`/`.pg.light`/`.live` des maquettes par `:root[data-theme="light"]` et `.live` sur `<html>` (`:root.live`), et en retirant tout ce que `base.css` fournit déjà. Les jetons ont les mêmes noms (`--ink0`, `--frost`, `--sig`, `--soft`, `--sline`, `--veil`…).
- Interactions (diaporamas, onglets, carrousels, compteurs, bascule 4:2:0, droits de la console, détection de l'appareil…) : en JavaScript ordinaire, avec un état de départ complet dans le HTML (la page doit être lisible et utile sans JavaScript).
- Accessibilité : vrais `<button>`, `aria-pressed`/`aria-selected`/`aria-expanded`, `alt` sur les photos, `aria-hidden` sur le décor.

## Modèle de `<head>` (FR ; adaptez titre, description, chemins)
```html
<!doctype html>
<html lang="fr" data-u="jeu" data-theme="dark">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>…</title>
<meta name="description" content="…">
<link rel="canonical" href="https://www.kybernow.app/jeu/">
<meta name="theme-color" content="#05070A">
<meta property="og:type" content="website">
<meta property="og:site_name" content="KyberNow">
<meta property="og:locale" content="fr_FR">
<meta property="og:locale:alternate" content="en_GB">
<meta property="og:title" content="…">
<meta property="og:description" content="…">
<meta property="og:url" content="https://www.kybernow.app/jeu/">
<meta property="og:image" content="https://www.kybernow.app/assets/og/kybernow-fr.jpg">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta name="twitter:card" content="summary_large_image">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="alternate" hreflang="fr" href="https://www.kybernow.app/jeu/">
<link rel="alternate" hreflang="en" href="https://www.kybernow.app/en/play/">
<link rel="alternate" hreflang="x-default" href="https://www.kybernow.app/en/play/">
<script src="/assets/lang.js"></script>
<script src="/assets/kn/theme.js"></script>
<link rel="preload" href="/fonts/Archivo-wdth-wght.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="/assets/kn/base.css">
<link rel="stylesheet" href="/assets/kn/jeu.css">
</head>
```
EN : `lang="en"`, `og:locale` `en_GB` et `og:locale:alternate` `fr_FR`, image `kybernow-en.jpg`.

## En-tête et menu (FR) — à copier tel quel, `aria-current="page"` sur l'entrée de la page
Logo : univers jeu/commun → `logo-spectre` ; pro → `logo-glacier` ; carrefour → les deux, le second avec la classe `g` (fondu).
```html
<a class="skip" href="#contenu">Aller au contenu</a>
<header class="top">
  <div class="wrap">
    <a class="logo" href="/" aria-label="KyberNow, version bêta, accueil">
      <img class="dk" src="/assets/img/logo/logo-spectre.svg" alt="KyberNow" width="150" height="21">
      <img class="lt" src="/assets/img/logo/logo-spectre-clair.svg" alt="" width="150" height="21" aria-hidden="true">
      <span class="beta" aria-hidden="true">Bêta</span>
    </a>
    <nav class="nav" aria-label="Sections">
      <a class="j" href="/jeu/">Jouer</a>
      <a class="p" href="/pro/">Travailler</a>
      <a href="/share/">Share</a>
      <a href="/technologie/">Technologie</a>
      <a href="/guide/">Guide</a>
      <a class="disc" href="https://discord.gg/sVy2mu5WdT" rel="noopener"><svg class="ic" aria-hidden="true"><use href="/assets/icons.svg#discord-logo"/></svg>Discord</a>
    </nav>
    <div class="tools">
      <button class="thm" type="button" aria-label="Passer en thème clair"><svg class="moon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/></svg><svg class="sun" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg></button>
      <a class="signin" href="https://share.kybernow.app/compte/connexion?lang=fr" rel="noopener"><span>Se connecter</span></a>
      <a class="lang" href="/en/play/" hreflang="en" lang="en" data-set-lang="en">EN</a>
      <a class="btn sig" href="/telecharger/">Télécharger</a>
      <button class="burger" type="button" aria-expanded="false" aria-controls="menu" aria-label="Ouvrir le menu"><span></span><span></span><span></span></button>
    </div>
  </div>
</header>
<div class="sheet" id="menu" role="dialog" aria-modal="true" aria-label="Menu">
  <nav aria-label="Menu principal">
    <a class="mi j" href="/jeu/" style="--i: 0">Jouer</a>
    <a class="mi p" href="/pro/" style="--i: 1">Travailler</a>
    <a class="mi" href="/share/" style="--i: 2">Share</a>
    <a class="mi" href="/technologie/" style="--i: 3">Technologie</a>
    <a class="mi" href="/guide/" style="--i: 4">Guide</a>
    <a class="mi" href="https://discord.gg/sVy2mu5WdT" rel="noopener" style="--i: 5">Discord</a>
  </nav>
  <div class="sheet-foot">
    <a href="https://share.kybernow.app/compte/connexion?lang=fr" rel="noopener">Se connecter</a>
    <a href="/en/play/" hreflang="en" lang="en" data-set-lang="en">English</a>
    <a class="btn sig" href="/telecharger/">Télécharger KyberNow</a>
  </div>
</div>
<main id="contenu">…</main>
```
EN : « Skip to content », « KyberNow, beta, home », Beta, Play (`/en/play/`), Work (`/en/pro/`), Share (`/en/share/`), Technology (`/en/technology/`), Guide (`/en/guide/`), Discord ; « Switch to light theme », « Sign in » (`…/compte/connexion?lang=en`), `FR` (vers la page française, `data-set-lang="fr"`), « Download » (`/en/download/`), « Open menu », « Main menu », « Français », « Download KyberNow ».

## Pied de page (FR)
```html
<footer class="ft">
  <div class="wrap">
    <div class="who">
      <img class="dk" src="/assets/img/logo/logo-spectre.svg" alt="KyberNow" width="132" height="18">
      <img class="lt" src="/assets/img/logo/logo-spectre-clair.svg" alt="" width="132" height="18" aria-hidden="true">
      <span>Un projet libre de <a href="https://www.leslignesclaires.fr" rel="noopener">Les Lignes Claires</a>.</span>
    </div>
    <nav aria-label="Pied de page">
      <a href="/jeu/">Jouer</a><a href="/pro/">Travailler</a><a href="/share/">Share</a><a href="/technologie/">Technologie</a><a href="/guide/">Guide</a><a href="/guide/faq/">FAQ</a><a href="/telecharger/">Télécharger</a><a href="https://discord.gg/sVy2mu5WdT" rel="noopener">Discord</a><a href="https://github.com/MonsieurNours/kybernow" rel="noopener">GitHub</a><a href="/mentions-legales/">Mentions légales</a>
    </nav>
  </div>
</footer>
<script src="/assets/kn/base.js" defer></script>
<script src="/assets/kn/jeu.js" defer></script>
</body>
</html>
```
EN : « A free and open project by Les Lignes Claires. » (avec le même lien), « Footer », Play, Work, Share, Technology, Guide, FAQ (`/en/guide/faq/`), Download, Discord, GitHub, Legal notice (`/en/legal/`).

## Vérifier
Serveur : `http://localhost:5200/…`. N'utilisez pas le panneau navigateur de l'app (partagé). Captures avec `node docs/refonte-site-2026-11/design/shim/cap.mjs '[{"url":"http://localhost:5200/jeu/","out":"docs/refonte-site-2026-11/design/captures/site/jeu-390.png","w":390,"h":844,"wait":2500}]'` (vraie largeur téléphone par le protocole de débogage de Chrome ; `w` 1440 pour le bureau ; `js` pour évaluer une expression, `scroll` pour défiler). Thème clair : ajoutez `"js":"document.documentElement.setAttribute('data-theme','light')"`. Vérifiez au moins : 1440 sombre et clair, 390 et 360 sombre et clair, aucun défilement horizontal, aucune erreur dans la console, les interactions, le mode live au défilement en 1440 × 900, `prefers-reduced-motion`.
