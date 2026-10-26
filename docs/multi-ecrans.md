# Multi-écrans

[English](multi-ecrans.en.md) · Français

Une session KyberNow peut prendre tous les écrans de l'appareil : l'ordinateur crée un écran virtuel par écran de l'appareil, placés de la même façon, et chaque écran de l'appareil affiche le sien dans une fenêtre plein écran. Ce document est le contrat commun de la Station, du cœur Rust et des apps Mac, Windows et Linux. Les autres apps Link (iPhone, iPad, Apple TV, Android, téléviseurs LG, client web) restent sur un écran et n'ont rien à changer.

Le multi-écrans n'a pas d'écrans à lui : il reprend l'accueil, la connexion, la barre de session, les réglages et les statistiques de [link-ui.md](link-ui.md), qui donne les textes exacts (section « Plusieurs écrans » de la connexion, ligne « Écrans » des réglages). Les trois apps se comportent pareil et emploient les mêmes mots ; un texte nouveau entre d'abord dans `link-ui.md`, puis dans le catalogue de chaque app, à l'identique.

## Ce que voit l'utilisateur

- **Réglage par ordinateur** « Écrans » (rubrique Image, sous Définition) : Auto, Cet écran, Tous les écrans. **Auto** (par défaut) prend tous les écrans en profil Bureau et Max, un seul en Jeu. Changer ce réglage ne fait pas passer le profil en « Personnalisé ». Inactif, sur « Cet écran », avec une définition sur mesure ou sans écran virtuel.
- **Accueil** : la ligne de l'écran dit ce que la session prendra (« 3 écrans, à la taille de ceux de cet appareil »), avec le même calcul que la session.
- **Connexion** : l'écran de connexion sur l'écran principal, le même fond sans texte sur les autres, chaque écran passe à son image dès sa première image.
- **Les écrans pris** : tous les écrans actifs de l'appareil, quatre au plus ; des écrans en recopie comptent pour un. « Cet écran » est l'écran où se trouve la fenêtre de l'app, comme une session à un écran.
- **L'écran principal** de l'appareil (Mac : celui de la barre des menus ; Windows : l'écran principal ; Linux : l'écran 0 de SDL) devient l'écran principal de Windows : la barre des tâches et les jeux en plein écran y vont.
- **Définition** : chaque écran virtuel a la définition de l'écran de l'appareil en pixels, comme « Comme cet écran » ; « Au plus 4K » s'applique à chaque écran ; en H.264, 4096 pixels de large au plus par écran.
- **Fréquence** : l'écran principal garde la sienne ; les autres sont à 60 Hz (moins si leur écran ne va pas jusque-là). Un écran immobile ne coûte presque rien : Kyber n'envoie une image que quand l'écran change.
- **Fenêtres** : une fenêtre plein écran par écran de l'appareil. En souris du bureau, le pointeur passe d'un écran à l'autre comme sur l'appareil. En souris des jeux, il reste capturé dans la fenêtre active.
- **Barre de session** : une seule, sur l'écran où se trouve le pointeur. Ses commandes (fin de session, souris, Touches, Statistiques) valent pour toute la session, depuis n'importe quelle fenêtre. « Plein écran » n'est pas proposé : les fenêtres restent en plein écran (« Cet écran » pour travailler dans une fenêtre). La fin de session ferme toutes les fenêtres.
- **Écrans de l'appareil qui changent en session** (branché, débranché, disposition ou recopie changée) : la session se refait avec les nouveaux écrans, sans revenir à l'accueil ; la connexion reprend à sa première étape sur l'écran principal.
- **Écran que l'ordinateur n'a pas pu créer** : sa fenêtre garde le fond de la connexion avec une seule ligne, « L'ordinateur n'a pas pu créer cet écran. » ; les autres écrans fonctionnent.
- **Station qui ne gère qu'un écran** (version antérieure) : la session se fait sur l'écran principal, sans message bloquant.

## Disposition des écrans

Le cœur calcule la place de chaque écran virtuel à partir de la disposition de l'appareil (une fonction commune aux trois apps, testée). Entrée : pour chaque écran de l'appareil, son cadre dans le repère logique de l'appareil (x, y, largeur, hauteur, y vers le bas ; l'app Mac convertit le repère de Cocoa), sa définition en pixels (après « Au plus 4K »), sa fréquence et s'il est principal. Sortie : la position en pixels de chaque écran sur le bureau de Windows.

- L'écran principal est en (0, 0).
- Les autres sont placés de proche en proche, en partant du principal : chacun se colle, du même côté (droite, gauche, dessus, dessous), à un voisin déjà placé avec lequel il partage un bord sur l'appareil ; son décalage le long de ce bord est celui de l'appareil, converti avec l'échelle pixels / unités logiques du voisin.
- Deux écrans placés partagent au moins un segment de bord (Windows l'exige) et ne se chevauchent jamais : un écran qui chevaucherait glisse le long du bord jusqu'à ne plus chevaucher.
- Un écran qui ne touche aucun voisin sur l'appareil (cas rare) se colle au plus proche, du côté de l'écart le plus grand.

Exemple (l'ordinateur d'essai de l'auteur) : XG32UQ 4K à 160 Hz au centre (principal), Studio Display 5K à droite, écran du MacBook Pro à gauche, plus bas.

## Échange avec Admin

`POST /api/stream/prepare` accepte un champ `screens` facultatif. Les champs `width`, `height` et `refresh` restent présents et décrivent l'écran principal : une Station qui ignore `screens` fait une session à un écran.

```json
{
  "width": 3840, "height": 2160, "refresh": 160,
  "screens": [
    { "width": 3840, "height": 2160, "refresh": 160, "x": 0, "y": 0, "primary": true },
    { "width": 5120, "height": 2880, "refresh": 60, "x": 3840, "y": 0 },
    { "width": 3456, "height": 2234, "refresh": 60, "x": -3456, "y": 400 }
  ],
  "audio_channels": 2, "virtual_display": true, "device": { "…": "…" }, "profile": "Bureau"
}
```

- De 1 à 4 écrans ; exactement un `primary`, en (0, 0) ; chaque mode valide comme aujourd'hui (7680 × 4320 au plus) ; rectangles sans chevauchement, tous reliés par des bords partagés. Sinon : `400`, sans rien toucher aux écrans.
- `screens` n'a d'effet qu'avec l'écran virtuel (`virtual_display` vrai et déclenchement actif) ; sans écran virtuel, la session reste sur les écrans de l'ordinateur, comme aujourd'hui.

Réponse : en plus des champs actuels (`applied` reste le mode de l'écran principal), `screens` donne, **dans l'ordre de la demande**, ce qui est en place : le mode et la position appliqués, ou `null` pour un écran que la Station n'a pas pu créer. L'écran principal est toujours créé, sinon la préparation échoue comme aujourd'hui.

```json
"screens": [
  { "width": 3840, "height": 2160, "refresh": 160, "x": 0, "y": 0, "primary": true },
  { "width": 5120, "height": 2880, "refresh": 60, "x": 3840, "y": 0 },
  null
]
```

Signal de vie, fin de session et reprise de main ne changent pas : une session, un appareil.

## Station

- **Écrans virtuels** : un écran SudoVDA par entrée de `screens`. Le premier garde l'identifiant actuel (`KyberNow-Display`), les suivants un identifiant dérivé, avec des numéros de série `KYBERNOW-0001` à `KYBERNOW-0004` : Windows retrouve les réglages de chaque écran (échelle) d'une session à l'autre.
- **Disposition** : seuls les écrans virtuels sont actifs, aux positions demandées, le principal en (0, 0) ; les écrans physiques s'éteignent et reviennent à la fin, comme aujourd'hui. Le signal de vie du pilote et les filets de sécurité couvrent tous les écrans.
- **Mode changé en session** (`follow` / `restore`) : s'applique à chaque écran.
- **Admin** : la Vue d'ensemble montre le nombre d'écrans et leurs modes ; le fil d'événements dit combien d'écrans la session a pris.
- **Correctif Kyber** : le débit d'une session à plusieurs écrans se répartit selon les pixels par seconde de chaque écran (largeur × hauteur × fréquence), au lieu de parts égales ; parts égales si un mode est inconnu.

## Cœur Rust

- `PrepareRequest` porte `screens` et `Prepared` les écrans appliqués ; sans `screens` dans la réponse, la session repart sur un écran.
- **Écrans de Kyber** : chaque écran appliqué est retrouvé dans la liste de Kyber par sa position et sa taille ; le cœur attend qu'ils soient tous annoncés (même délai qu'aujourd'hui), puis diffuse ceux qui sont là. Une entrée vidéo par écran dans la session de Kyber, l'écran principal en premier.
- **Flux** : chaque flux vidéo porte l'index de l'écran de l'appareil auquel il va ; l'app reçoit les paquets de chaque flux séparément.
- **Entrées** : une position de souris absolue est donnée avec l'index de l'écran (pixels de cet écran) et part avec l'identifiant d'écran de Kyber correspondant. Le pointeur annoncé par l'ordinateur arrive avec son écran : l'app ne le dessine que dans la fenêtre de cet écran. Clavier, souris relative et manettes ne changent pas.
- **Débit** : le débit du profil se calcule sur la somme des pixels par seconde des écrans, avec les mêmes plafonds qu'aujourd'hui.
- **Reprise après une perte** : une image clé par flux, jamais avant les premières données de ce flux (une demande trop tôt laisse kyavserver muet) ; relancer le flux relance tous les écrans.
- **Mesures** : latence et images par seconde de l'écran principal ; Statistiques donne en plus le nombre d'écrans.
- **API des apps** : la session à un écran garde exactement son API (les apps qui ne gèrent pas le multi-écrans ne changent pas). Le multi-écrans est demandé par l'app, qui donne au cœur la liste de ses écrans ; le pont C (`apps/apple`) et le pont JNI (`apps/android/jni`) compilent toujours.

## Apps

- **Mac** : une fenêtre plein écran par écran (`NSScreen.screens`, recopies regroupées), pointeur capturé dans la fenêtre active en souris des jeux, barre des menus et Dock masqués.
- **Windows** : une fenêtre plein écran sans bordure par écran (`EnumDisplayMonitors`), un seul appareil Direct3D 11 et une chaîne d'échange par fenêtre ; le crochet clavier reste unique.
- **Linux** : une fenêtre SDL par écran (`SDL_GetNumVideoDisplays`, `SDL_GetDisplayBounds`), en mode Bureau ; le mode Salon et le Steam Deck restent sur un écran.

## Hors du multi-écrans

- Diffuser plusieurs écrans physiques de l'ordinateur sans écran virtuel.
- Étaler un écran de l'ordinateur sur plusieurs écrans de l'appareil.
- Le client web (Kyber n'envoie qu'un flux vidéo en WebSocket), l'écran externe des iPhone et iPad, Android et les téléviseurs.

## Essai de bout en bout

Sur le Mac de l'auteur (XG32UQ 4K à 160 Hz principal, Studio Display 5K à 60 Hz, MacBook Pro à 120 Hz, recopie coupée) vers l'ordinateur d'essai (RTX 3080 Ti) : profil Bureau, Écrans en Auto → trois écrans virtuels placés comme ceux du Mac, le pointeur passe de l'un à l'autre, une fenêtre glissée d'un écran à l'autre suit ; un jeu en plein écran s'ouvre sur le XG32UQ à 160 Hz ; profil Jeu → un seul écran ; fin de session → écrans physiques de l'ordinateur revenus ; débrancher le Studio Display en session → la session se refait sur deux écrans.
