# Brief : réécriture du guide de KyberNow

## Le but
Le guide actuel est une seule longue page : `site/guide/index.html` (douze chapitres, ids `prerequis serveur appareil session playstation partage pret compte distance relais mises-a-jour desinstaller faq`). Dans la refonte, il devient :
- un **accueil du guide** (`/guide/`), sommaire par tâches ;
- **un article par tuto**, chacun avec son URL, son titre et sa description pour les moteurs de recherche ;
- une **FAQ à part** (`/guide/faq/`), riche en questions-réponses.

Les fichiers que vous écrivez serviront directement à construire le site. Ils doivent être exacts, complets et prêts à publier.

## À qui on parle
Quelqu'un qui installe, les mains sur le clavier ou la télécommande : précis, concret, dans l'ordre des écrans. Grand public par défaut. Les articles qui concernent surtout les équipes (comptes, familles et équipes, déploiement, relais à soi) gardent la même voix, un cran plus technique.

## Le ton
Lisez `docs/ton-du-site.md` en entier (section « Le guide » comprise) : ses règles priment. Vouvoiement, typographie française (espace fine insécable `&#8239;` ou U+202F avant ; : ! ? et dans « », espace insécable entre nombre et unité, virgule décimale), majuscule en début de titre seulement, pas de tiret cadratin.
Ton de référence voulu par l'auteur : celui de Playruo (playruo.com) : direct, sûr de lui, titres courts et concrets, jargon assumé puis expliqué, chiffres précis, une pointe de légèreté. Sans « seamless », sans superlatifs vides.
L'auteur rejette tout ce qui « fait IA » : antithèses « pas X, mais Y », deux-points à effet, titres-slogans, triades automatiques, « fluide », « sans effort », « en toute simplicité », phrases qui décrivent la page, paragraphes qui répètent le titre. Pas de surexplication : une langue accessible mais d'un bon niveau.
Exemples de textes validés de la refonte : « Les mêmes apps font les deux. Choisissez l'usage qui vous ressemble. » ; « Vous jouez avec votre manette sur l'écran le plus proche, en 4K et en HDR. L'image met moins de 10 ms à arriver. » ; « Copiez l'image sur une carte microSD et branchez le Pi à la télé. Il démarre directement sur KyberNow Link. »

## Les faits
- Partez du texte actuel du guide pour le contenu, et vérifiez-le dans la doc du dépôt (`docs/*.md`, en particulier `link-ui.md` pour les vrais libellés des écrans et des boutons, `comptes.md`, `kybernow-share.md`, `pret.md`, `playstation.md`, `relais.md`, `hdr.md`, `pyrowave.md`, `stylet.md`, `station-macos.md`, `station-linux.md`, `multi-ecrans.md`, `webcam.md`, `micro.md`, `enregistrement.md`, `invitations.md`, `profil-auto.md`, `debit-automatique.md`).
- **Les libellés d'interface (boutons, menus, réglages, messages) se recopient tels qu'ils sont dans les apps** (`link-ui.md` et le guide actuel), entre « » quand ils sont cités.
- Présentez tout comme disponible et fonctionnel : la Station existe pour Windows, Mac et Linux ; les apps Apple TV et iPhone/iPad sont sur l'App Store ; les comptes KyberNow, les familles et équipes, le SSO et OAuth, les liens d'accès, « Regarder », l'enregistrement, le micro et la webcam vers la Station existent. Ne dites jamais qu'une fonction n'a pas été essayée, ni qu'elle est nouvelle. Gardez les vraies limites (débit de PyroWave, Pi 5 seul pour PyroWave, son en stéréo dans le navigateur, 5.1 au plus sur LG…).
- N'inventez aucun chiffre, aucun prérequis, aucun nom de fichier. Si un détail manque (par exemple le nom exact d'un paquet Linux), écrivez une phrase qui reste vraie sans lui.
- Message agnostique dans les accroches (« votre ordinateur sur tous vos appareils ») ; les plateformes réelles vont dans les articles d'installation.

## Le format de chaque article
Un fichier Markdown par article, dans `docs/refonte-site-2026-11/guide/`, nommé `<slug>.md` :

```
---
slug: installer-station-windows
titre: Installer KyberNow Station sur Windows        # le H1, court
title: Installer KyberNow Station sur Windows · KyberNow   # balise <title>, 60 caractères au plus
description: …                                       # meta description, 150 caractères au plus, phrase complète
rubrique: Installer                                  # rubrique du sommaire
univers: commun                                      # commun, jeu ou pro
duree: 5 min                                         # temps de lecture ou de mise en place, si utile
connexes: [connecter-apple-tv, hors-de-chez-vous]    # 2 à 4 slugs d'articles liés
---

Chapeau : deux phrases au plus, ce que l'article permet de faire.

## Avant de commencer        (si utile : prérequis en liste courte)

## …                         (étapes numérotées 1. 2. 3. dans l'ordre des écrans ; un intertitre par écran ou par tâche)

> Bon à savoir : …           (encadrés rares, seulement s'ils servent)

## Si ça ne marche pas        (si utile : symptôme → solution, en phrases courtes)
```
Liens internes : `[texte](/guide/<slug>/)`. Pas d'image (les captures seront ajoutées à la construction), mais vous pouvez indiquer `<!-- capture : … -->` là où une capture aiderait.

La FAQ (`faq.md`) : même en-tête, puis des `## Question ?` suivis d'une réponse de deux à quatre phrases, groupées par thèmes avec `# Thème` (ou des sections claires). Reprenez les questions de l'actuelle FAQ, réécrites, et ajoutez celles qu'un nouveau venu pose vraiment (prix, sécurité, comparaison, bêta, compte), sans rien inventer.

## Le découpage (slugs imposés)
Rubrique **Commencer** : `ce-qu-il-vous-faut`
Rubrique **Installer la Station** : `installer-station-windows`, `installer-station-mac`, `installer-station-linux`
Rubrique **Connecter un appareil** : `connecter-un-appareil` (le principe : l'ordinateur apparaît, le code dans Admin, les autres façons d'ajouter), `link-mac`, `link-windows`, `link-linux-steam-deck`, `apple-tv`, `iphone-ipad`, `android`, `televiseur-lg`, `raspberry-pi`, `navigateur`
Rubrique **Pendant la session** : `profils-et-qualite` (profils, format vidéo et PyroWave, débit, HDR), `clavier-et-raccourcis` (mode immersif, raccourcis par app, commandes à la télé et au Steam Deck), `stylet-micro-webcam`, `plusieurs-ecrans-et-ordinateurs`
Rubrique **Consoles** : `playstation`
Rubrique **Partager** : `partager` (KyberNow Share : ouvrir, rejoindre, droits des invités, image de chaque invité, invités loin de chez vous), `preter`
Rubrique **Comptes et équipes** : `compte-kybernow` (créer, se connecter, ouvrir un ordinateur à quelqu'un, Regarder, liens d'accès, réglages qui suivent, déplacer un ordinateur, sécurité), `familles-et-equipes` (organisations, rôles, groupes de personnes et d'ordinateurs, journal, SSO et OAuth, déploiement par jeton d'inscription, Admin à distance)
Rubrique **Hors de chez vous** : `hors-de-chez-vous` (relais, ce qui y passe, VPN), `installer-votre-relais`
Rubrique **Entretien** : `mettre-a-jour`, `desinstaller`
Et `faq`.

## Ce que vous rendez
Les fichiers de votre lot, puis un bilan de dix lignes au plus : articles écrits, points de faits incertains que l'auteur doit vérifier (avec la source consultée), et libellés d'interface que vous n'avez pas pu confirmer.
