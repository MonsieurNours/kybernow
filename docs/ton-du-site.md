# Rédaction du site : le ton de KyberNow

[English](ton-du-site.en.md) · Français

Décidé avec l'auteur le 24/10/2026. Ces règles s'appliquent à tous les textes visibles du site (www.kybernow.app) : pages, titres, boutons, légendes, textes alternatifs, `aria-label`, balises `<title>` et `meta description`, données structurées, `llms.txt`. Les écrans d'apps recréés en HTML (classe `.sc`, `aria-hidden`) reprennent les vrais textes des apps : on n'y touche pas.

## À qui on parle

- **L'accueil** parle au grand public : quelqu'un qui a un PC Windows et veut l'utiliser ailleurs (jouer sur la télé du salon, reprendre son bureau depuis le canapé, depuis l'hôtel), qui n'a jamais entendu parler de Moonlight et ne sait pas ce qu'est QUIC. Il veut savoir ce que ça fait pour lui, sur quels appareils, ce qu'il lui faut, si c'est gratuit et sûr.
- **La page Technologie** parle aux curieux et aux passionnés (ceux qui connaissent Moonlight, Sunshine, Parsec) : les chiffres, le trajet d'une image, QUIC, Kyber, la comparaison. Même voix, plus de détails, toujours expliqués.
- **Le guide** parle à quelqu'un qui installe, pas à pas, les mains sur le clavier : précis, concret, dans l'ordre des écrans, sans jargon inutile.

On **vouvoie** partout. Le visiteur est souvent le sujet de la phrase : « Vous jouez sur la télé », plutôt que « Le jeu s'affiche sur le téléviseur ».

## La voix : simple et chaleureuse

- Des phrases courtes et complètes, avec un sujet et un verbe. Une idée par phrase.
- Des mots de tous les jours : « la télé » plutôt que « le téléviseur » dans les textes d'accroche (le guide et les fiches techniques peuvent dire « téléviseur »), « votre PC » ou « votre ordinateur », « l'image », « le son », « la manette ».
- Du concret : des appareils, des pièces de la maison, des moments (« le soir, sur le canapé », « à l'hôtel », « dans le train »). Un exemple vaut mieux qu'un adjectif.
- Chaleureux veut dire attentif, pas familier : pas d'argot, pas d'emoji, pas de points d'exclamation à la chaîne, pas de blagues forcées.
- Honnête : KyberNow est en bêta, certaines apps s'installent encore à la main (Apple TV, iPhone, téléviseurs LG), certaines n'ont pas été essayées sur du vrai matériel. On le dit simplement, sans s'excuser et sans le cacher.

## À ne plus jamais écrire

Ces tournures font « texte généré » et l'auteur les refuse. Exemples relevés sur le site actuel :

| Tournure | Exemple à bannir | Pourquoi |
|---|---|---|
| Antithèse « X, pas Y » | « Mesuré, pas promis. » ; « Pas une démonstration : … » | Formule creuse, ton de slogan |
| Phrase pivot « Ce qui change, c'est… » | « Ce qui change, c'est la façon de le conduire » | Construction artificielle, mot impropre (« conduire ») |
| Titre-slogan elliptique | « Tout, tout de suite. » ; « Pensé jusqu'au dernier pixel. » ; « Convaincu ? Trois étapes. » | Ne dit rien de concret |
| Personnification | « L'ordinateur se tait, l'appareil écoute. » | Effet de style gratuit |
| Rythme en trois temps systématique | « Gratuit, libre, sans compte. » en titre ; « Sans abonnement, sans machine louée : la vôtre. » | Martèlement publicitaire |
| Deux-points à effet | « … : la vôtre. » ; « Chez KyberNow, c'est le réglage par défaut : installez, connectez, rien d'autre. » | Chute artificielle |
| Mode d'emploi de la page | « Lecture automatique au défilement · sans son » ; « Faites défiler : de la capture à l'affichage. » ; « La vidéo démarre d'elle-même quand… » | L'utilisateur s'en moque |
| Superlatif invérifiable | « Aucune autre solution ne réunit tout cela » ; « nouvelle génération » | Promesse creuse |
| Jargon non expliqué sur l'accueil | QUIC, TLS 1.3, Opus, 4:4:4, 4:2:0, HEVC, encodeur, FEC, A/B, HDMI-CEC, « flasher », VRR, Adaptive-Sync, « souris relative » | Le grand public décroche |
| Mots creux | « expérience », « fluide », « fluidité », « solution », « écosystème », « révolutionnaire », « puissant », « ultime », « seamless », « sans compromis » | Vides de sens |
| Étiquettes en cascade | « Sur vos appareils », « Prêt à l'emploi », « Déjà entre vos mains » au-dessus de chaque titre | Décor, pas information |

Autres réflexes à éviter : commencer une phrase par « Et » ou « Ou », les points de suspension, les questions rhétoriques en titre, les fragments sans verbe en cascade (« Clavier, souris, manette. Tout. »), les mots en anglais quand le français existe (« streaming » est admis dans les textes pour passionnés, « jeu à distance » ailleurs).

## Ce qu'on écrit à la place

- **Titres** : ils disent ce que le visiteur obtient ou fait. « Jouez sur la télé du salon », « Ce qu'il vous faut pour commencer », « Installer KyberNow en trois étapes ». Une phrase normale, pas un slogan. Le gras du titre (`<strong>`) met en valeur la partie qui compte, il ne crée pas d'effet de chute.
- **Chiffres** : on les garde (la latence de 9,4 ms, la 4K à 120 Hz, le son 7.1…), mais chacun est dit en clair la première fois. Exemple : « 9,4 millisecondes entre votre PC et votre écran : moins d'un centième de seconde, mesuré sur un réseau local ». On ne répète pas le même chiffre dans chaque section.
- **Jargon** : sur l'accueil, on dit l'effet, pas la technique (« l'image et vos frappes sont chiffrées » plutôt que « TLS 1.3 » ; « si le Wi-Fi décroche, l'app se reconnecte toute seule » plutôt que « FEC »). Le nom technique peut suivre entre parenthèses sur la page Technologie, jamais seul.
- **Noms des apps** : on garde les trois noms, KyberNow Station (sur le PC à partager), KyberNow Admin (les réglages, dans le navigateur) et KyberNow Link (sur vos appareils), et KyberNow Link OS (l'image pour Raspberry Pi). On les présente une fois clairement, puis on peut dire « la Station », « Admin », « l'app Link ».
- **Boutons** : un verbe et un objet. « Télécharger », « Voir la vidéo », « Installer sur l'Apple TV », « Lire le guide ».
- **Textes alternatifs et `aria-label`** : décrire ce qu'on voit, en une phrase utile (« Une télé affiche l'accueil de KyberNow Link ; une personne tient une manette »). Pas de mode d'emploi.

## Ce que KyberNow apporte (les messages à faire passer)

Dans cet ordre d'importance pour le grand public, toujours avec des faits vérifiés dans le code et la doc (PROJECT.md, ROADMAP.md, README des apps) : rien d'« essayé » qui ne l'a pas été, aucune mesure d'exemple présentée comme une mesure.

1. Votre PC sur tous vos écrans : la télé, le Mac, la tablette, le téléphone, le Steam Deck, un navigateur. Pour jouer comme pour travailler.
2. Réactif comme un écran branché : très faible latence grâce au moteur Kyber, image à la définition et à la fréquence de chaque écran.
3. Ça tient le coup : si le réseau flanche, l'image se répare et l'app se reconnecte toute seule (dix essais de reconnexion, une seconde d'écart, sur toutes les apps).
4. La manette en fait beaucoup, sans rien ajouter : l'app choisie par appareil s'ouvre au début de la session (Steam Big Picture, Playnite…), le PC voit la manette comme une manette Xbox, les apps de télé et du Steam Deck se parcourent entièrement à la manette (qui peut aussi servir de souris), l'écran à la bonne taille est créé tout seul. Ce sont des irritants connus du jeu à distance, réglés sans modules à installer. Ne pas écrire « tout à la manette dans toutes les apps » : l'app Windows et le navigateur n'ont pas de navigation à la manette, Mac et iPhone seulement le menu de session.
5. Gratuit, libre, sans compte ni abonnement, rien ne passe par un serveur tiers, tout est chiffré.
6. En bêta : on l'utilise déjà au quotidien, on le dit franchement.

## Conventions typographiques (FR)

- Espace fine insécable avant ; : ! ? (`&#8239;` dans le HTML), guillemets français « » avec espaces fines insécables (`«&#8239;` et `&#8239;»`), espace insécable (`&nbsp;`) entre un nombre et son unité (9,4&nbsp;ms, 4K à 120&nbsp;Hz, 77,3&nbsp;Mo).
- Virgule décimale en français (9,4 ms), point en anglais (9.4 ms).
- Majuscule seulement au début des titres et aux noms propres (pas de Title Case en français).
- Pas de tiret cadratin pour faire une pause dans une phrase : une virgule, deux points, ou une nouvelle phrase.

## L'anglais

L'anglais n'est pas une traduction mot à mot : même voix, mêmes faits, écrits comme un rédacteur anglophone les écrirait (anglais britannique, déjà employé sur le site : « colour », « favourite »…). Mêmes interdits (pas de « not X, but Y », pas de slogans elliptiques, pas de « seamless », « next-gen », « effortless »). Le titre et la description de l'accueil gardent « Sunshine and Moonlight alternative » pour le référencement.

## Les images

Les photos de l'accueil montrent des moments de vie réels, avec de vraies captures de l'app sur les écrans : des personnes de 30 à 50 ans, femmes et hommes, de dos ou de trois quarts, dans une lumière chaude du soir, souvent à plusieurs. Elles sont générées et assemblées par `scripts/site-scenes/` (voir son README). Les écrans d'apps recréés en HTML (`.sc`) restent pour montrer l'interface de près.
