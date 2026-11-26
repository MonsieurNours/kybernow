# Banc de mesure

[English](banc-de-mesure.en.md) · Français

Des outils de développement, hors des apps, pour chiffrer une version de KyberNow et la comparer à une autre : avant et après un chantier d'optimisation, avec les mêmes instruments et les mêmes conditions. Tout est dans [tools/banc](../tools/banc) :

| Outil | Ce qu'il mesure | Où il tourne |
|---|---|---|
| `banc.py ecran` (+ `sonde.html`, `lecteur.swift`) | la latence de l'écran de la Station à l'écran de l'appareil | sonde : un navigateur sur la Station ; lecteur : le Mac client |
| `banc bride` | rien : il joue un réseau (débit, retard, gigue, pertes) entre l'appareil et la Station | n'importe quelle machine, sans droits administrateur |
| `banc echo` + `banc enregistre` | un vrai lien (Wi-Fi, 4G, relais), relevé en trace rejouable | les deux bouts du lien |
| `banc rejeu-debit`, `banc.py debit` | le débit automatique des apps rejoué sur des traces, sans réseau | partout, en quelques secondes |
| `banc.py rapport` | une page HTML qui compare des mesures | partout |

Les temps de chaque étape mesurés par la session elle-même (`kyclient --metrics`) restent l'affaire de [tools/latency](../tools/latency) ; `analyze_metrics.py --json` les donne au rapport.

## Construire

```bash
cd tools/banc && cargo build --release
```

`banc` est un crate à part (son propre espace de travail), qui se construit sans `third_party`. Le lecteur Swift est construit par `banc.py` au premier usage (`swiftc`, outils de Xcode). Python 3.9 ou plus, sans paquet à installer.

## Latence de l'écran à l'écran

Le temps entre l'instant où la Station montre une image et l'instant où l'appareil montre la même. C'est ce que l'on ressent ; les statistiques de la session n'en donnent qu'une partie (ni la composition de l'écran de la Station, ni celle de l'appareil).

**Comment** :

1. **La sonde** (`sonde.html`), ouverte en plein écran sur la Station dans n'importe quel navigateur, dessine en haut à gauche une bande de 16 blocs : deux repères (jaune, bleu), un compteur d'images de 11 bits en code de Gray, un bit de parité, et les repères à l'envers. Elle change à chaque image de l'écran et note l'instant de chaque image (début du rendu, horloge de la page).
2. Elle envoie ces instants au banc par HTTP, avec l'écart entre son horloge et celle du banc : 25 allers-retours toutes les 5 s, le plus court gardé. La précision est la moitié de cet aller-retour (± 0,5 ms sur la boucle locale, ± 1 à 2 ms en Wi-Fi), affichée avec chaque mesure.
3. **Le lecteur** (`lecteur.swift`), sur le Mac client, cherche la bande sur ses écrans (dans la fenêtre de KyberNow Link), puis capture cet écran par ScreenCaptureKit aussi vite qu'il se compose, jusqu'à 240 images par seconde. Pour chaque capture, il lit le compteur et note l'instant où l'écran l'a montrée.
4. Le banc apparie : la latence d'une image est l'instant de sa première apparition sur le Mac moins l'instant où la sonde l'a dessinée. Le code de Gray fait qu'une image déchirée entre deux (parité fausse) se lit quand même comme l'une des deux : elle est comptée à part. Une lecture que ni sa voisine d'avant ni celle d'après ne suit de près (0 à 16 images) est une erreur de lecture, rejetée.

**Ce qu'un chiffre comprend** : le rendu du navigateur et la composition de l'écran de la Station, la capture, l'encodage, le réseau, le décodage, la mise en attente et l'affichage dans l'app, la composition de l'écran du Mac et sa capture. **Ce qu'il laisse** : le temps de réponse de la dalle du Mac (constant pour un écran donné).

**Le plancher** : la sonde ouverte sur le Mac même, sans KyberNow entre les deux, mesure le retard propre de l'instrument (rendu du navigateur, composition, capture). On ne compare que des mesures au même plancher, et on le retranche par la pensée. Une mesure n'est précise qu'à un rafraîchissement près de l'écran de la Station.

**Mesurer** :

```bash
python3 tools/banc/banc.py ecran --plancher --etiquette plancher        # la sonde s'ouvre ici même
python3 tools/banc/banc.py ecran --etiquette main --note "HEVC Jeu, câble, 4K 60" --duree 30
```

Pour la seconde, le banc affiche l'adresse de la sonde (`http://<adresse du Mac>:8765/sonde`) : l'ouvrir en plein écran sur la Station (F11) ; dès qu'elle se manifeste, mettre la session KyberNow Link au premier plan, en plein écran ou fenêtre assez grande pour qu'un bloc garde au moins 8 pixels (`?bloc=60` sur l'adresse de la sonde pour des blocs plus grands). Le lecteur part 3 s plus tard (`--delai`) et capture `--duree` secondes. Le Mac doit autoriser l'enregistrement de l'écran à l'app qui lance le banc (Réglages Système › Confidentialité et sécurité › Enregistrement de l'écran). Le pare-feu de la Station doit laisser son navigateur joindre le Mac sur le port 8765 (`--port`).

Chaque mesure va dans `tools/banc/mesures/<date>-<étiquette>/` (hors du dépôt) : `resultat.json` (statistiques et latences), et les données brutes (`sonde.json`, `lectures.jsonl`) que `banc.py analyse <dossier>` recalcule.

**Résultat** : médiane, p95, p99, moyenne, maximum ; images de la Station arrivées à l'écran par seconde ; part d'images déchirées ; lectures rejetées ; précision de l'horloge.

Le lecteur est pour Mac seulement. Sur un autre appareil (Apple TV, télé, téléphone), filmer les deux écrans à 240 images par seconde reste la voie ([prototype A](archives/prototype-A.md), §3.2).

## La bride : un réseau joué

`banc bride` est un relais : l'app se connecte à lui comme à la Station, il fait suivre l'UDP (l'image, le son, les entrées de Kyber) en jouant une trace dans chaque sens, et le TCP (Admin) sans le toucher. Il tourne en espace utilisateur : ni `sudo`, ni pare-feu à modifier, sur Mac, Linux ou Windows.

Chaque sens est un lien : pertes au hasard, puis débit limité derrière une file d'attente bornée (`file_ms`, 100 ms par défaut : un paquet qui attendrait plus est jeté, comme dans une box), puis retard fixe et gigue (un retard tiré au hasard pour chaque paquet, sans les désordonner, sauf `--desordre`). La **descente** va de la Station à l'appareil, la **montée** de l'appareil à la Station.

```bash
tools/banc/target/release/banc profils                    # les profils intégrés
tools/banc/target/release/banc trace chute-20 --rtt 20    # ce qu'un profil joue
tools/banc/target/release/banc bride --vers 192.168.1.100 --udp 8080 --tcp 8080,37462 \
    --profil 4g --rtt 20 --journal bride.jsonl
```

- `--udp` : le port de Kyber (8080 par défaut ; le journal de l'app l'écrit, « console : … port Some(…) ») ; `--tcp` : le même et celui d'Admin. `40241:8080` écoute sur 40241 et joint 8080, pour une Station sur la même machine.
- Puis, dans l'app, ajouter l'ordinateur par l'adresse de la machine qui fait tourner la bride. La bride écoute sur toutes les adresses (`--ecoute` pour en choisir une).
- **Station Mac ou Linux** : Admin peut donner à l'app une adresse « préférée » pour l'image (journal de l'app : « image par l'adresse préférée de l'ordinateur »), qui contourne la bride. Dans ce cas, passer par `tools/latency/run_client.sh <adresse de la bride>`, qui joint Kyber directement. Station Windows : pas d'adresse préférée.
- `--boucle` rejoue la trace sans fin ; sinon son dernier palier reste. `--duree S` arrête la bride. Chaque seconde, elle écrit ce qui est passé et ce qui s'est perdu (au hasard, ou par la file) dans chaque sens, sur l'écran et dans `--journal`.
- La précision est de l'ordre de la demi-milliseconde (l'émission attend par tranches d'une milliseconde, puis en tournant : macOS réveille tard les longues attentes).

| Profil | Ce qu'il joue |
|---|---|
| `propre` | rien (le retard de `--rtt` seul) |
| `capacite-20` | descente à 20 Mbit/s |
| `chute-20` | descente à 100 Mbit/s, 20 Mbit/s de 30 à 60 s, puis 100 Mbit/s |
| `capacite-3-1` | descente à 3 Mbit/s, 1 Mbit/s à partir de 60 s |
| `pertes-2` | 2 % de pertes en descente, 5 % à partir de 60 s |
| `rafales` | toute la descente perdue 50 ms toutes les 2 s |
| `gigue-15` | descente retardée de 0 à 15 ms au hasard, paquet par paquet |
| `wifi` | Wi-Fi chargé : gigue de 4 ms, 0,2 % de pertes, pic de 60 ms pendant 300 ms toutes les 7 s |
| `4g` | réseau mobile : 15 à 40 Mbit/s changeant chaque seconde (toujours la même suite), +25 ms, gigue 8 ms, 0,5 % de pertes |
| `retour-coupe` | toute la montée perdue pendant 1 s à 30 s : les accusés de l'appareil s'arrêtent |

Les profils durent 2 minutes ; `--rtt` ajoute un aller-retour de base, moitié dans chaque sens. Conditions de référence : `--rtt 10` sur le réseau local, `--rtt 40` pour le relais.

### Format d'une trace

JSON Lines : une ligne d'en-tête facultative, puis un palier par ligne, qui tient jusqu'au suivant ; un sens absent n'est pas bridé.

```json
{"banc":"trace","nom":"salon","source":"banc enregistre …","duree_ms":60000}
{"t_ms":0,"descente":{"debit_mbps":0,"retard_ms":3.1,"gigue_ms":2.4,"pertes_pct":0,"file_ms":100},"montee":{"retard_ms":3.1}}
{"t_ms":200,"descente":{"debit_mbps":38.5,"retard_ms":3.1,"gigue_ms":0,"pertes_pct":0,"file_ms":100}}
```

`debit_mbps` 0 : sans limite.

## Relever un vrai lien

Pour rejouer chez soi le Wi-Fi du salon, une 4G ou le chemin par le relais : `banc echo` tourne à l'autre bout, `banc enregistre` lui demande un flux régulier vers l'appareil, comme l'image d'une session, et en tire une trace.

```bash
banc echo                                              # à l'autre bout (UDP 7979)
banc enregistre --echo 192.168.1.100:7979 --debit 40 --duree 120 --nom salon --sortie salon.jsonl
```

- Toutes les `--pas` ms (200 par défaut) : le débit reçu, le retard de la file (l'arrivée moins l'envoi, rapportée à son minimum : sans horloge commune), sa dispersion (9ᵉ moins 1ᵉʳ décile, qui devient la gigue) et les paquets perdus. Le retard de base est la moitié de l'aller-retour le plus court de sondes envoyées toutes les 20 ms.
- Un pas où la file monte (plus de 10 ms) ou qui perd plus de 1 % est **saturé** : le lien ne passe pas plus, son débit reçu devient la capacité du palier, et la file vient de là (la bride la refera). Sinon, le lien n'est pas limité à ce débit : `--debit` doit dépasser ce que l'on veut savoir (40 à 80 Mbit/s pour un Wi-Fi, 20 à 40 pour une 4G).
- Les sondes perdues comptent en pertes de montée (au-delà de 20 % d'un pas, et pas quand la descente sature : la file de descente les a jetées au retour).
- Le relevé charge le lien : ne pas le faire tourner sur la connexion de quelqu'un sans le dire.

**Essai** (26/11/2026, boucle locale du Mac) : la bride jouant `chute-20` avec `--rtt 20`, un relevé à 50 Mbit/s retrouve « sans limite, +10 ms » jusqu'à 29,5 s, « 20 Mbit/s, +10 ms » de 30 à 59,5 s, puis sans limite, avec l'aller-retour des sondes à 20 ms (120 ms file pleine).

## Rejouer le débit automatique sans réseau

`banc rejeu-debit` fait passer une trace dans un modèle simple du lien (une file devant le débit, les pertes, le retard), seconde après seconde, et donne chaque seconde au vrai calcul des apps, [apps/core/src/adaptive.rs](../apps/core/src/adaptive.rs), repris tel quel. Deux minutes de trace se rejouent en quelques millisecondes : de quoi régler les seuils avant d'essayer sur la bride et en vrai.

- Le modèle avance par pas de 10 ms ; l'aller-retour d'une seconde est pris à un instant qui change d'une seconde à l'autre (Kyber en donne un toutes les quelques secondes) ; un trou d'image est compté quand un pas perd plus de 10 % de ses paquets (au-delà de la correction d'erreurs).
- Il ne modèle ni l'encodeur (le débit demandé est le débit envoyé), ni la montée (`retour-coupe` ne se juge que sur la bride).
- Résultat par trace : débit reçu moyen ; **utilisation** (part du débit utile disponible, capacité ou débit du profil, réellement reçue) ; secondes où la file dépasse 15 ms ; secondes avec des trous ; paquets perdus ; changements de débit ; aller-retour p50 et p95.

```bash
python3 tools/banc/banc.py debit main .                 # main contre l'arbre de travail, tous les profils
python3 tools/banc/banc.py debit main claude/opti-pertes-aa4ebe --trace pertes-2 --trace salon.jsonl
```

`banc.py debit` prend `apps/core/src/adaptive.rs` de chaque version git (« . » : l'arbre de travail), le construit avec le banc de la branche en cours (les règles du rejeu restent les mêmes), et écrit le rapport. Débit du profil : `--besoin` (60 Mbit/s) ; aller-retour de base : `--rtt` (10 ms).

**Quand adaptive.rs change** : le rejeu en reprend le fichier tel quel et remplace par de petits modules ce qu'il emprunte au cœur (`latency::Second` réduit à `rtt_ms`, `lost_packets`, `bytes`, `holes` ; `i18n`). Si le calcul se met à lire un autre champ de `Second`, ajouter ce champ à `tools/banc/src/main.rs` (et sa valeur à `simu::Mesure`) dans le même commit ; une version qui ne se construit plus avec le banc l'arrête avec un message.

## Le rapport

```bash
python3 tools/banc/banc.py rapport tools/banc/references/2026-11-26-main/plancher.json mesures/…-avant mesures/…-apres -o rapport.html
python3 tools/latency/analyze_metrics.py metrics.json --json etapes-apres.json --etiquette après
```

Une page autonome (sans dépendance, claire et sombre), avec la première mesure pour référence : pour l'écran à l'écran, un tableau (médiane, p95, p99, moyenne, maximum, images par seconde, images déchirées) avec l'écart de chaque version, en vert quand il améliore, et la répartition des latences ; pour les étapes de la session (`analyze_metrics.py --json`), médiane et p95 de chaque étape ; pour le débit automatique, un tableau par trace et la courbe du débit demandé par chaque version sur la capacité du lien.

## Mesure de référence : main au 26/11/2026

Dans [tools/banc/references/2026-11-26-main](../tools/banc/references/2026-11-26-main) : `plancher.json` et `debit.json` (main 36c25c24, KyberNow 0.11.4). La mesure de l'écran à l'écran face à une Station est à faire par l'auteur (commandes ci-dessus) : elle demande la sonde ouverte sur la Station et la session au premier plan.

**Plancher** (Mac de l'auteur, écran 5K à 60 Hz, navigateur par défaut) : médiane 29,2 ms, p95 30,5, p99 30,9 ; 59,8 images par seconde ; aucune déchirée ; précision ± 0,5 ms.

**Débit automatique** (profil à 60 Mbit/s, `--rtt 10`) :

| Trace | Reçu (Mbit/s) | Utilisation | File > 15 ms | Trous | Perdus | Changements | Aller-retour p95 |
|---|---|---|---|---|---|---|---|
| propre | 60,0 | 100 % | 0 s | 0 s | 0 | 0 | 10 ms |
| capacite-20 | 17,6 | 88 % | 21 s | 3 s | 7 458 | 32 | 110 ms |
| chute-20 | 30,6 | 61 % | 9 s | 3 s | 7 410 | 25 | 37 ms |
| capacite-3-1 | 2,0 | 100 % | 120 s | 120 s | 52 961 | 7 | 110 ms |
| pertes-2 | 24,9 | 41 % | 0 s | 0 s | 7 963 | 7 | 10 ms |
| rafales | 58,5 | 97 % | 0 s | 60 s | 18 750 | 0 | 10 ms |
| gigue-15 | 60,0 | 100 % | 0 s | 0 s | 0 | 0 | 25 ms |
| wifi | 59,9 | 100 % | 0 s | 0 s | 1 575 | 0 | 22 ms |
| 4g | 16,8 | 68 % | 18 s | 10 s | 8 067 | 33 | 235 ms |
| retour-coupe | 60,0 | 100 % | 0 s | 0 s | 0 | 0 | 10 ms |

Ce que le rejeu montre de main, à confirmer sur la bride :

- **Pertes sans saturation** (`pertes-2`) : 2 % de pertes au hasard font baisser le débit d'un cran (42 Mbit/s), 5 % jusqu'au plancher (5 Mbit/s), alors que baisser n'y change rien ; 41 % du débit utile seulement.
- **Plancher au-dessus du lien** (`capacite-3-1`) : le plancher de 5 Mbit/s dépasse un lien de 3 puis 1 Mbit/s ; la file reste pleine toute la session.
- **Capacité fixe** (`capacite-20`, `4g`) : le débit oscille (32 et 33 changements en 2 minutes) en dents de scie autour de la capacité, avec des secondes de file pleine à chaque remontée.
- **Rafales** : aucun changement de débit, à raison, mais 50 ms perdues d'un coup dépassent la correction d'erreurs à chaque rafale (60 secondes avec des trous).
