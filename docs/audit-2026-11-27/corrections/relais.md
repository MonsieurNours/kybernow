# Corrections de l'audit du 27/11 : relais (`server/relay`)

Vague 1, branche `claude/fix-relais-0a07e5`. Rapport d'origine : `docs/audit-2026-11-27/relais.md`. Tout est dans `server/relay` (et son crate `tunnel/`), plus `docs/comptes.md` et `docs/comptes.en.md`.

## Constats corrigés

| Id | Correction |
|---|---|
| REL-01 | `frame` et `page_with` échappent eux-mêmes le `<title>` (`title_text` : balises retirées, entités décodées, tout réechappé) et le chemin du pied de page ; essai de noms hostiles `<"'&` sur dix pages de la console. |
| REL-02 | Délégation bornée : qui a reçu Administrer ne donne que ses droits, jusqu'à leur échéance (ajout, lien, acceptation du lien) ; « ouvrir à une équipe » seulement avec des droits sans échéance. Retirer son droit (ou lui ôter Administrer) retire en cascade ce qu'il a donné et révoque ses liens en attente (`drop_delegated`, par `invited_by`). |
| REL-03 | Une échéance par droit (migration 10, colonne `grants.right_ends`) : l'union garde l'échéance de chaque droit ; `expires_at` reste la plus lointaine, pour les requêtes qui ne lisent qu'elle. |
| REL-04 | `remove_member` efface les droits individuels du membre sur les ordinateurs de l'équipe (et, pour un simple membre, ce qu'il y a délégué). |
| REL-05 | Le cookie d'attente porte l'identifiant du compte créé par ce navigateur (0 si l'adresse était prise) ; « changer d'adresse » et la session après vérification ne valent que pour lui. Les cookies d'avant (sans identifiant) restent lus. |
| REL-06 | Ajouter une passkey ou activer la double authentification : mot de passe redemandé si la session a plus de 15 min (champ affiché seulement alors) ; la réinitialisation retire les passkeys des 7 derniers jours. |
| REL-07 | Secret TOTP illisible : refus (et `error` au journal) au lieu d'ouvrir ; refus de démarrer si la clé manque et qu'un compte a la double authentification. |
| REL-08 | Essais de mot de passe par (compte, réseau /24 ou /48) à 5, plafond de 30 par compte ; un navigateur de confiance du compte n'est pas compté. |
| REL-09 | Partie relais : PKCE S256 facultatif (`code_challenge` à `/compte/connexion?appareil=…`, gardé dans `device_codes.challenge` ; `code_verifier` exigé par `/v1/devices/token` quand un défi a été donné). Les apps publiées, sans défi, passent comme avant. |
| REL-10 | « Ouvrir Admin » : une adresse IP n'est acceptée que parmi les adresses d'Admin données par la Station (`admin_lan`), ou en boucle locale ; un nom du réseau seulement dans leur schéma. Adresses inconnues (Station hors ligne) : l'ancienne règle. |
| REL-12 | Déplacer ou détacher un ordinateur d'équipe : Propriétaire seulement (le bouton suit). |
| REL-13 | 20 invitations par heure et 100 par jour par compte (équipes, ordinateurs, renvois), ligne `warn` au journal ; 20 familles ou équipes au plus par propriétaire. |
| REL-14 | `kn-media/1` : plus de flux bidirectionnels (limite 0, et ceux déjà ouverts refusés) ; `kn-ctl/1` : plus de flux unidirectionnels ; l'ancienne connexion des paquets est fermée quand une nouvelle la remplace (`MediaSender::close`, ajout au crate du tunnel). |
| REL-15 | Une connexion `admin_tls` ne tient plus la place d'un client et se coupe après 5 min sans un octet ; 32 connexions relayées au plus par client. |
| REL-16 | `tcp_total` (4096, réglable) ; `read_msg` lit le corps au fil de l'arrivée au lieu d'allouer la longueur annoncée. |
| REL-17 | Délai de 10 s sur la lecture des corps (408) et sur l'attente de la file des comptes (503) ; l'API des appareils (`/v1/access`…) a sa propre file, hors d'Argon2. |
| REL-18 | Limites par adresse en /64 pour IPv6 partout (`ConnCounter`, poignées de main QUIC, connexions de Station). |
| REL-19 | Seau de messages de contrôle par client (64 d'un coup, 32/s ; `ping`/`pong` exclus) ; un client sans Station inactif est renvoyé comme les autres (une question `status` compte comme activité). |
| REL-20 | Partie dépôt : `update.sh` ne remplace pas une image épinglée par son empreinte, vérifie la signature cosign quand `RELAY_COSIGN_KEY` est donnée (sinon l'ancienne image reprend son étiquette) ; README FR et EN. |
| REL-21 | HSTS `max-age=31536000` (sans sous-domaines) sur toutes les réponses du relais. |
| REL-22 | Codes de secours et codes e-mail en HMAC avec une clé dérivée de celle du relais ; les anciens codes de secours (SHA-256) valent encore. |
| REL-23 | Le lien de « Formulaire expiré » (et tout lien de `pages::message`) est échappé. |
| REL-24 | « Dernier Propriétaire » : compte et écriture sous le même verrou de la base. |
| REL-25 | `detach_station` tout ou rien (`SAVEPOINT`). |
| REL-26 | Enrôlement tout ou rien, rattachement compris (`atomic`, qui s'imbrique ; `attach_to` y passe aussi). |
| REL-27 | Jetons d'enrôlement refusés pour une famille, même par un POST direct. |
| REL-28 | Une image construite sur place (aucune empreinte de registre) n'est jamais remplacée par la mise à jour de la nuit ; le README conseille `RELAY_IMAGE=kybernow-relay:local`. |
| REL-30 | `claim_invitations` ajoute l'invitation aux droits déjà là (`add_person_rights`) au lieu de l'effacer. |
| REL-31 | `Agent::reserve` retire sa demande après le délai. |
| REL-32 | Nom d'inscription nettoyé (contrôle et bidi retirés) par le nettoyeur commun `clean_text`. |
| REL-38 (en partie) | `clean_text` sert aux ordinateurs, appareils, équipes, passkeys et à l'inscription (les copies sont devenues des appels). |

## Constats laissés

| Id | Raison |
|---|---|
| REL-09 (apps) | Le cœur Rust et les apps doivent envoyer `code_challenge` puis `code_verifier` : hors de cette zone. Le relais est prêt et reste compatible. |
| REL-10 (jeton en POST ou dans le fragment) | Admin (0.11.7) lit `?token=` : changer le transport du jeton demande Admin d'abord (`server/admin`, hors zone). |
| REL-11 | Ajout direct sans consentement et réponse qui révèle un compte : changer l'ajout en invitation à accepter est un choix de produit (et touche les textes communs) ; effort M. À trancher par l'auteur. |
| REL-20 (signature) | La signature des images (cosign, clé hors ligne) se fait à la publication (`scripts/release.sh`, hors zone) ; tant qu'elle n'existe pas, `RELAY_COSIGN_KEY` reste vide. Rien n'a été touché sur la machine réelle. |
| REL-29 | Fin du jour dans le fuseau de la personne : demande le fuseau du navigateur (script, ou réglage du compte) ; bug sans enjeu de sécurité, laissé à la vague 2. |
| REL-33 à REL-36 | Optimisations (verrou des pages, verrous par datagramme, copies par paquet, `AttachRequest` sur l'exécuteur) : hors du périmètre « sécurité puis bugs S ». |
| REL-37, REL-39, REL-40 | Factorisation et code mort : vague 2. |
| REL-41, REL-42 (en partie) | Doc de l'e-mail « Admin ouvert », version 0.8.0 du `Cargo.toml` : la version suit la publication (coordinatrice). Le README dit maintenant que la mise à jour n'écrase plus une image construite sur place. |
| REL-16 (moins de flux avant `hello`) | yamux ne change pas sa limite de flux en cours de connexion ; la lecture au fil de l'arrivée et `tcp_total` bornent déjà la mémoire. |

## Compatibilité avec les apps et les Stations publiées (0.11.7)

- Protocole du tunnel inchangé : `MediaSender::close` est un ajout ; `read_msg` lit la même trame. Les apps n'ouvrent ni flux bidirectionnels en `kn-media/1` ni unidirectionnels en `kn-ctl/1` ; le repli QUIC des paquets (flux unidirectionnels en média) reste permis.
- Connexion de présence sans Station (`presence`) : elle se ferme d'elle-même en 5 s, bien avant le ménage (30 s).
- API des appareils : `code_verifier` n'est exigé que si l'app a donné un défi.
- Base : migration 10 (`grants.right_ends`, `device_codes.challenge`) ; une base migrée refuse un relais plus ancien, comme toujours.
- Cookies d'attente de l'ancienne forme acceptés ; anciens codes de secours acceptés ; codes e-mail en clair encore en cours (24 h) acceptés.

## Ce qui est vérifié, et comment

- `cargo test --workspace` dans `server/relay` : 230 essais passent (167 du relais, 23 d'intégration du tunnel, 33 + 7 du crate `tunnel`), contre 205 avant ; deux passes de suite sans échec. Une fois, en cours de travail, `un_bout_qui_ne_lit_pas_ses_reponses_est_coupe` a lu `Reset` au lieu de la fermeture attendue ; il est passé aux sept passes suivantes (seul et dans la suite) ; probablement instable sous charge, à surveiller.
- Essais ajoutés : noms hostiles dans les pages (et vérifié qu'il échoue sans le correctif), échéances par droit, délégation bornée et cascade, départ d'une équipe, invitation fusionnée, mot de passe redemandé, passkeys retirées à la réinitialisation, TOTP illisible, refus de démarrer sans clé, inscription sur une adresse prise, limite d'invitations, plafond d'équipes, essais de connexion par réseau et navigateur de confiance, adresses d'Admin, PKCE (vecteur de la RFC 7636), connexion relayée muette, trame annoncée longue et coupée, HSTS, secrets en HMAC, Administrateur qui ne sort pas un PC, jetons d'une famille, `atomic` imbriqué, nettoyeur bidi.
- `sh -n update.sh` ; `update.sh` n'a pas été exécuté (pas de Docker ni de machine réelle).
- Rien n'a été déployé ni essayé sur `share.kybernow.app`.

## Notes de version

FR :
- KyberNow Share est plus sûr : un nom d'ordinateur ou d'équipe ne peut plus modifier les pages, et une personne à qui vous donnez « Administrer » ne peut plus donner plus que ce qu'elle a, ni plus longtemps. Si vous lui retirez l'accès, ce qu'elle a partagé part avec.
- Chaque droit garde sa propre date de fin : un lien « Regarder » sans fin ne prolonge plus un accès limité.
- Quitter une équipe retire aussi l'accès à ses ordinateurs.
- Pour ajouter une passkey ou activer la double authentification sur une session ouverte depuis un moment, votre mot de passe vous est redemandé. Un nouveau mot de passe retire les passkeys ajoutées ces derniers jours.
- Personne ne peut plus bloquer votre connexion en essayant de mauvais mots de passe depuis un autre réseau.
- Les invitations sont limitées par compte, pour éviter les envois en masse.
- « Ouvrir Admin » n'envoie votre accès qu'à l'adresse que votre ordinateur a donnée.

EN:
- KyberNow Share is safer: a computer or team name can no longer change the pages, and someone you give “Administer” to can no longer give more than they have, or for longer. If you remove their access, what they shared goes with it.
- Each right keeps its own end date: an endless “Watch” link no longer extends a limited access.
- Leaving a team also removes access to its computers.
- To add a passkey or turn on two-factor authentication in a session opened a while ago, you are asked for your password again. A new password removes passkeys added in the last few days.
- Nobody can block your sign-in any more by trying wrong passwords from another network.
- Invitations are limited per account, to prevent mass sending.
- “Open Admin” only sends your access to the address your computer gave.
