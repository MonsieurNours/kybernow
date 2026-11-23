# KyberNow Link sur l'App Store

[English](app-store.en.md) · Français

KyberNow Link pour iPhone, iPad et Apple TV part sur l'App Store, **avec** le prêt de l'ordinateur et les consoles PlayStation (décision du 23/11/2026). Ce document donne ce qui est fait dans le dépôt, les réponses à donner dans App Store Connect, les notes pour l'App Review et ce qui reste à faire. Règles de référence : [App Review Guidelines](https://developer.apple.com/app-store/review/guidelines/), version du 16/10/2026.

## Périmètre

| App | App Store | Pourquoi |
|---|---|---|
| iPhone et iPad (`apps/ios`) | Oui | |
| Apple TV (`apps/tvos`) | Oui | |
| Mac (`apps/macos`) | Non, reste en Developer ID | Le Mac App Store exige le bac à sable (2.4.5 i), refuse Sparkle (2.4.5 iv) et l'ouverture à l'ouverture de session cochée sans accord (2.4.5 iii) |

Les trois apps ont le même identifiant (`fr.leslignesclaires.kybernow`) : une seule fiche App Store, en **achat universel** (iPhone, iPad, Apple TV). Gratuite, sans achat intégré.

## Ce qui est fait dans le dépôt

### Manifestes de confidentialité

Un `PrivacyInfo.xcprivacy` par app et par KyberKit (le cadre embarqué, cœur Rust compris), ajouté aux projets Xcode ; XcodeGen les reprend seul (ils sont dans les dossiers sources). Pas de pistage, aucune donnée collectée par le développeur, et les API à raison déclarée :

| API | Raison | Où |
|---|---|---|
| `UserDefaults` | CA92.1 : réglages de l'app | Apps et KyberKit |
| Dates des fichiers | C617.1 : fichiers du conteneur de l'app (images des ordinateurs) | Apps (`Platform.swift`), KyberKit (cœur Rust) |
| Temps depuis le démarrage | 35F9.1 : mesure de durées (`mach_absolute_time`, `systemUptime` : images, manettes) | Apps et KyberKit |
| Claviers actifs | 54BD.1 : disposition du clavier envoyée à l'ordinateur (`KeymapIOS.swift`) | KyberKit iOS |

Top Shelf n'appelle aucune de ces API : pas de manifeste. Toute nouvelle API de la [liste d'Apple](https://developer.apple.com/documentation/bundleresources/describing-use-of-required-reason-api) doit entrer dans le manifeste du même ensemble.

### Chiffrement

`ITSAppUsesNonExemptEncryption` vaut `true` sur iOS et tvOS : les apps chiffrent avec des algorithmes standard qui ne sont pas ceux du système d'Apple (TLS de rustls, QUIC de quinn, ring ; AES du transport des consoles PlayStation). Dans App Store Connect :

1. « Votre app utilise-t-elle le chiffrement ? » : **Oui**.
2. Type : **algorithmes standard, à la place ou en plus du chiffrement du système d'Apple** (pas d'algorithme propriétaire).
3. États-Unis : produit grand public (5D992), sans rapport annuel depuis la règle du BIS de mars 2021.
4. France : App Store Connect demande la **déclaration de fourniture d'un moyen de cryptologie** auprès de l'ANSSI quand l'app y est distribuée. À déposer avant la première soumission ; Apple donne ensuite un code de conformité, à mettre dans `ITSEncryptionExportComplianceCode` pour ne plus répondre à chaque version.

## App Store Connect

### Étiquettes de confidentialité

- **Pistage** : non.
- **Données collectées par le développeur** : le relais (`share.kybernow.app`) garde les adresses IP dans ses journaux quelques jours, pour le fonctionnement et la sécurité, sans lien avec une identité. Apple n'a pas de catégorie « adresse IP » : rien à déclarer tant qu'elle ne sert pas à situer l'utilisateur. Si le relais gardait davantage (comptes, statistiques d'usage), revoir les étiquettes.
- Tout le reste reste sur l'appareil ou dans l'iCloud de l'utilisateur, hors de portée du développeur (ordinateurs, mots de passe dans le Trousseau, identifiant et pseudo PSN) : rien à déclarer.
- **URL de la politique de confidentialité** : la section Confidentialité des mentions légales (`https://www.kybernow.app/mentions-legales/`, `/en/legal/`). Y ajouter les comptes PlayStation (identifiant et pseudo lus une fois chez Sony, gardés sur l'appareil et dans iCloud) avant la soumission.

### Classification par âge

L'app montre l'écran de l'ordinateur sans filtre (navigateur, jeux, tout contenu) : répondre « oui » à l'accès web sans restriction, ce qui donne la tranche la plus haute.

### Fiche

- Nommer les plateformes d'Apple et Windows ; ne pas citer Android, Google ni les autres plateformes de KyberNow (2.3.10).
- PlayStation : seulement en texte, comme compatibilité (« jouez à votre PS5 et votre PS4 »), avec la mention « PlayStation, PS5 et PS4 sont des marques de Sony Interactive Entertainment Inc. » ; aucun logo, aucune image de Sony, ni dans l'icône ni dans les captures.
- Captures sur de vrais ordinateurs, sans contenu de jeux protégés en vedette.
- Catégorie : Utilitaires, seconde catégorie : Productivité (l'app n'est pas un jeu).

## App Review

Le testeur n'a pas de Station : sans moyen de se connecter, la soumission est refusée (2.1). Il faut :

- **Une Station de démonstration** allumée pendant la review, avec « Accès hors de chez vous » actif, joignable par le relais public ;
- **Le partage pour parcours principal** : l'appairage ne passe jamais par le relais (premier contact sur le réseau seulement, [relais.md](relais.md)), le testeur ne peut donc pas ajouter l'ordinateur comme le sien. Il entre par « Rejoindre avec un code » sur un partage ouvert ; quelqu'un de l'équipe l'accepte dans Admin et lui donne le clavier, la souris et les manettes, aux heures données dans les notes. Il voit ainsi le bureau entier, le cas générique de 4.2.7. Pas de compte d'ami dans les notes : le prêt est décrit et montré dans la vidéo, pas proposé à l'essai ;
- **Une vidéo** sur un vrai iPhone et une vraie Apple TV : ajout, session, partage, prêt, console PlayStation (la console ne peut pas être prêtée au testeur : la vidéo la montre).

### Notes pour l'App Review (en anglais)

> KyberNow Link is a remote desktop client for the user's own Windows PC, running our free host software KyberNow Station (https://www.kybernow.app). It is a generic mirror of the host's screen (guideline 4.2.7): every app runs on the user's PC, nothing is executed or sold in the client. No account is created in the app; there is no in-app purchase.
>
> To test: tap "Join with a code" and enter **{code}**. Our demo PC is reachable over the internet through our relay and stays on during review; a team member accepts you and gives you the keyboard, mouse and gamepads, {hours} (Paris time). If you need another time slot, contact us at {email}.
>
> Lending (KyberNow Share): the owner of a PC can lend it to a trusted friend, from the host (KyberNow Admin). During a loan the friend sees the owner's own Windows session, restricted by the owner to the apps they chose; the owner creates and manages the friend's access on the host, and can end it at any time. It is shown in the attached video.
>
> PlayStation consoles: the app can also display the user's own PS5 or PS4 on their home network, like Sony's Remote Play. Signing in with Sony happens once, on Sony's own page, only to read the user's PSN account ID, which the console requires for pairing; it is stored on the device. A console cannot be provided for review: see the attached video, {link}.
>
> Video of the full flow on iPhone and Apple TV: {link}.

## Risques assumés

- **Prêt (4.2.7)** : pendant un prêt, l'ami voit un lanceur limité aux apps choisies par l'hôte ; un testeur peut y voir le « miroir d'un logiciel » soumis à 4.2.7 a, qui exige un ordinateur appartenant à l'utilisateur, sur son réseau local. Notre réponse : c'est la session Windows de l'hôte, restreinte par lui ; l'accès naît et se gère sur l'hôte (4.2.7 c) ; rien ne s'achète ni ne se parcourt comme un magasin (4.2.7 d).
- **PlayStation (5.2.1, 5.2.2)** : protocole de Sony recréé, connexion au compte Sony dans une vue web, nom des marques. D'autres apps de lecture à distance de PlayStation sont sur l'App Store, mais Apple peut demander une autorisation de Sony, et une plainte de Sony ferait retirer l'app.
- **Licences** : publier l'app sous AGPL est permis aux titulaires des droits. Le code des consoles est écrit d'après chiaki-ng (AGPL-3.0) : s'il en reprend l'expression et pas seulement le protocole, ses auteurs sont aussi titulaires, et la GPL est jugée incompatible avec les conditions de l'App Store (retrait de VLC en 2011). Même question pour toute contribution extérieure.
- **Parcours du testeur** : le testeur entre par le partage et voit le bureau entier ; le prêt n'est que décrit et montré. Il ne doit pas être caché pour autant (2.3.1, fonctions cachées) : notes et vidéo le présentent. Un testeur ou un réexamen plus tard peut encore l'essayer.
- **Plan de repli** : si Apple refuse l'une de ces fonctions, la masquer dans les versions App Store par une option de compilation (`APPSTORE`), sans toucher aux versions installées depuis un Mac.

## Reste à faire avant la soumission

- [ ] Fiche App Store (achat universel iPhone, iPad, Apple TV), textes FR et EN, captures.
- [ ] Station de démonstration, partage ouvert et permanence pour accepter le testeur, vidéo.
- [ ] Mentions légales : ajouter les comptes PlayStation à la section Confidentialité.
- [ ] Déclaration à l'ANSSI, puis `ITSEncryptionExportComplianceCode`.
- [ ] Licence de Kyber (`third_party/kyber-desktop`) : confirmer qu'elle permet la distribution sur l'App Store.
- [ ] Code des consoles : vérifier ce qui vient de chiaki-ng, ou obtenir l'accord de ses auteurs.
- [ ] Ajouter un fichier `LICENSE` à la racine du dépôt (seul `apps/Cargo.toml` donne `AGPL-3.0-or-later`), éventuellement avec une permission additionnelle pour les magasins d'apps (AGPL, section 7).
- [ ] Profils de distribution App Store (iOS, tvOS, Top Shelf), archive Release, TestFlight interne d'abord.
- [ ] Autorisation multicast (`com.apple.developer.networking.multicast`), facultative : réveil par diffusion.
