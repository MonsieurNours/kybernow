# Plusieurs ordinateurs à la fois

[English](plusieurs-ordinateurs.en.md) · Français

Une app KyberNow Link peut tenir des sessions sur plusieurs ordinateurs en même temps, une fenêtre par ordinateur : garder un œil sur le PC du bureau pendant une partie sur celui du salon, ou travailler sur deux machines depuis le même Mac. Ce document est le contrat commun du cœur Rust et des apps Mac, Windows et Linux (interface de bureau). Les autres apps Link (iPhone, iPad, Apple TV, Android, téléviseurs LG, client web), le Steam Deck, le salon et KyberNow Link OS restent sur un ordinateur à la fois. La Station ne change pas : chaque ordinateur voit une session ordinaire, d'un appareil.

Ce chantier n'a pas d'écrans à lui : il reprend l'accueil, la connexion, la barre de session, le menu de l'icône et les notifications de [link-ui.md](link-ui.md), qui donne les textes exacts (section « Plusieurs ordinateurs »). Les trois apps se comportent pareil et emploient les mêmes mots ; un texte nouveau entre d'abord dans `link-ui.md`, puis dans les catalogues de chaque app.

## Ce que voit l'utilisateur

- **Ouvrir un autre ordinateur pendant une session**, de trois façons, partout les mêmes :
  - **la barre de session** : un menu « Ordinateurs » liste vos autres ordinateurs, avec leur état ; un ordinateur en ligne s'ouvre d'un clic, un ordinateur hors ligne se réveille comme depuis l'accueil (seul moyen sous Linux, qui n'a pas d'icône près de l'horloge) ;
  - **le menu de l'icône près de l'horloge** (Mac, Windows) : « Se connecter » reste actif sur les autres ordinateurs ;
  - **l'accueil** (Mac, Windows), rouvert par « Ouvrir KyberNow Link » : « Se connecter » reste actif sur les autres ordinateurs.
- **Une fenêtre par ordinateur**, titrée de son nom. La première session s'ouvre comme aujourd'hui (plein écran, ou tous les écrans avec le multi-écrans). Chaque session suivante s'ouvre **dans une fenêtre**, par-dessus, sur l'écran de la fenêtre active, centrée, aux deux tiers de cet écran en 16:9 ; elle prend un seul écran (« Cet écran »), dont la taille est celle de la fenêtre, comme toute session en fenêtre. Sa barre propose « Plein écran » : elle couvre alors son écran, la première session gardant les siens.
- **La session active** est celle de la fenêtre au premier plan. Elle seule reçoit le clavier, la souris, les manettes, le presse-papiers et le son. Les autres gardent leur image à l'écran, pour garder un œil, mais leur ordinateur ne reçoit rien, et on ne les entend pas. Une fenêtre entièrement cachée ou réduite allège son image, comme aujourd'hui.
- **Le son** : « Garder le son » dans la barre (le bouton du haut-parleur) laisse une session audible même quand elle n'est pas active. Coupé par défaut, valable pour la session en cours.
- **Changer d'ordinateur** : un clic sur sa fenêtre, ou le raccourci **« Ordinateur suivant »**, ⌃⌥⌘O sur Mac, Ctrl+Alt+Maj+O sur Windows et Linux, qui passe à la fenêtre de l'ordinateur suivant (dans l'ordre d'ouverture), même en plein écran, où ⌘Tab et Alt+Tab vont à l'ordinateur. Le menu de l'icône et le menu « Ordinateurs » de la barre ont aussi « Afficher » pour chaque session.
- **Le même ordinateur deux fois**, c'est impossible : « Se connecter » sur un ordinateur déjà en session fait passer sa fenêtre devant ; l'accueil montre alors « Afficher la session » à la place de « Se connecter ».
- **Quatre ordinateurs au plus** en même temps. Au-delà, « Se connecter » est inactif, avec la raison.
- **Fin d'une session** : « Se déconnecter » ou la fermeture de sa fenêtre ne termine que la sienne. La dernière fermée ramène l'accueil, comme aujourd'hui. Quitter l'app les termine toutes. Une session qui s'arrête sans « Se déconnecter » (un autre appareil, Admin, une erreur) alors que sa fenêtre n'était pas active donne la notification habituelle, même si l'app est au premier plan.
- **Le reste ne change pas** : la dernière session, les réglages, les images, le profil et la souris restent ceux de chaque ordinateur ; les statistiques et ⌃⌥⌘S (Ctrl+Alt+Maj+S) sont celles de la fenêtre active.

## Cœur Rust

- **Plusieurs `Session` dans un processus** : c'est déjà possible (aucun état global hors la langue et le journal). Les lignes du journal portent le nom de l'ordinateur.
- **Un registre des sessions** (`focus`), une seule source pour « qui est active » :
  - `register(session) → id`, `unregister(id)`, `activate(id)`, `set_keep_sound(id, bool)` ;
  - à chaque changement, il applique à toutes les sessions : entrées actives ou non, son coupé ou non (sauf « Garder le son »), presse-papiers, manettes ;
  - les apps lui disent seulement quelle fenêtre passe au premier plan.
- **`InputPipeline::set_active(bool)`**. Inactive : touches et boutons relâchés, manettes au repos (axes au centre, gâchettes relâchées), souris relative coupée, presse-papiers en pause (rien lu sur l'appareil, rien écrit dessus), vibrations ignorées ; en filet, le traitement des entrées de kynput coupé. Active : le contenu du presse-papiers de l'appareil est renvoyé à l'ordinateur.
- **`AudioEngine::set_muted(bool)`**, avec un fondu court (une vingtaine de millisecondes) pour ne pas claquer. Le son continue d'être décodé et vidé : il revient sans décalage.
- **Les manettes : un seul lecteur par app**, dans le cœur, repris de celui de l'app Linux (`apps/linux/src/device/pads.rs`, SDL). SDL ne s'ouvre qu'une fois par processus : chaque session ne peut plus lire les manettes elle-même.
  - Les manettes sont branchées sur chaque session (la Station garde sa manette virtuelle, un jeu ne la perd pas) ; boutons et axes ne vont qu'à la session active ; à la désactivation, l'état au repos part une fois.
  - Les vibrations ne reviennent à la manette que depuis la session active.
  - Mac et Windows passent à `gamepads_from_app` ; libkynput y est construite sans son lecteur SDL, comme sur Linux et Android. Une app avec une seule session se comporte comme avant.
- **Écrans** : une session ouverte pendant qu'une autre tourne prend « Cet écran » en fenêtre, quel que soit le réglage « Écrans » (deux sessions ne prennent jamais les mêmes écrans). Seule la première peut prendre tous les écrans.
- **Pont C** : `kn_focus_*`, `kn_input_set_active`, `kn_audio_set_muted`. Rien côté JNI (Android reste sur un ordinateur).
- **Limite** : quatre sessions par app.

## Apps

- **Mac** : un gestionnaire de sessions remplace la session unique ; chaque session a sa propre fenêtre, et l'accueil ne sert plus de fenêtre de session : il reste disponible pendant les sessions (« Ouvrir KyberNow Link », le Dock), et « Réglages » et « Ajouter un ordinateur » du menu de l'icône y deviennent actifs, comme sur Windows. Les fenêtres des sessions suivantes doivent pouvoir passer par-dessus une session en plein écran. Les commandes du menu Session et les raccourcis vont à la session active. Le code commun à l'Apple TV et à l'iPhone (`KyberKit`) garde leur comportement.
- **Windows** : les sessions dans un registre (fenêtre → session) à la place de l'état global ; le crochet du clavier et Raw Input suivent la fenêtre active ; une seule fenêtre de barre, qui sert la session de la fenêtre où elle s'ouvre ; les fonds de connexion seulement pour la première session ; `--connect`, la jump list et les notifications ouvrent une session de plus au lieu d'être refusés.
- **Linux** (interface de bureau) : une liste de sessions à la place de la session unique ; les événements portent l'identifiant de leur session ; chaque fenêtre appartient à une session ; le son est coupé par session (plus de `MUTED` global) ; les manettes passent par le lecteur du cœur. Le Steam Deck, le salon et Link OS gardent une session.

## Hors de ce chantier

- Plusieurs appareils sur un même ordinateur (spectateur, 2e joueur) : c'est KyberNow Share, sur la feuille de route.
- Deux sessions sur le même ordinateur depuis une même app : impossible (la Station ne tient qu'un appareil à la fois).

## Essai de bout en bout

Deux Stations (par exemple PC-DE-MATTHIEU et PC-ATELIER), sur Mac puis sur Windows et Linux :

1. Session sur le premier en plein écran (puis avec tous les écrans) ; depuis la barre, « Ordinateurs » › le second : sa fenêtre s'ouvre par-dessus, à son nom, et devient active.
2. Taper, cliquer, brancher une manette : seul l'ordinateur de la fenêtre active réagit ; ⌃⌥⌘O (Ctrl+Alt+Maj+O) passe à l'autre, et la manette suit sans que le jeu la perde.
3. Son : seule la session active s'entend ; « Garder le son » sur l'autre la laisse audible.
4. Copier un texte sur l'un, passer à l'autre, coller : le texte ne passe que par l'appareil, jamais d'un ordinateur à l'autre en arrière-plan.
5. « Se connecter » sur un ordinateur déjà en session : sa fenêtre passe devant. Cinquième ordinateur : refusé avec la raison.
6. Fermer la fenêtre du second : la première session continue. Quitter l'app : les deux se terminent, les écrans des deux ordinateurs reviennent.
