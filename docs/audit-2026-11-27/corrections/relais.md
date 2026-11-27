# Corrections de l'audit du 27/11 : relais (`server/relay`)

Vague 1, branche `claude/fix-relais-0a07e5`. Rapport d'origine : `docs/audit-2026-11-27/relais.md`. Tout est dans `server/relay` (et son crate `tunnel/`), plus `docs/comptes.md` et `docs/comptes.en.md`. Les 42 constats sont traités : 39 en entier, 3 en partie (REL-09, REL-10 et REL-20 ont une moitié dans d'autres zones, voir plus bas).

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
| REL-11 | Plus d'ajout direct d'un compte existant : une invitation qu'il accepte ou refuse dans son espace (page « Invitations », bandeau sur « Ordinateurs », e-mail « Voir l'invitation ») ; la réponse est toujours « Invitation envoyée », qu'un compte existe ou non. Une personne qui a déjà un accès voit ses droits changer comme avant. |
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
| REL-29 | « jusqu'au JJ/MM » finit à minuit dans le fuseau du navigateur : petit script `assets/tz.js` qui remplit un champ caché `tz` (décalage borné à ±12 h) ; sans lui, UTC comme avant. La date affichée (`end_date`) reste la bonne dans tous les cas. |
| REL-30 | `claim_invitations` ajoute l'invitation aux droits déjà là (`add_person_rights`) au lieu de l'effacer. |
| REL-31 | `Agent::reserve` retire sa demande après le délai. |
| REL-32 | Nom d'inscription nettoyé (contrôle et bidi retirés) par le nettoyeur commun `clean_text`. |
| REL-33 | `grants_of_many` : les droits de tous les ordinateurs d'une page en une requête (équipe, « Personnes », « Ordinateurs ») ; membres et groupes d'une équipe en deux requêtes au lieu de deux par membre ; le menu ne relit plus deux fois les ordinateurs visibles. Le HTML se construit après avoir relâché la base. |
| REL-34 | Plus de verrou global par paquet : un drapeau « retiré » (`AtomicBool`) sur chaque pair, posé là où il quitte la table ; le plafond global en GCRA sans verrou (`AtomicRate`). |
| REL-35 | Repli TCP : l'enregistrement n'est plus recopié (la longueur s'écrit dans le tampon d'écriture), et la lecture réutilise un tampon (`read_record_into`). |
| REL-36 | `AttachRequest` hors de l'exécuteur (`spawn_blocking`), et 20 demandes aux comptes par minute et par Station. |
| REL-37 | Un seul gabarit de document (`pages::document`) pour les pages seules, la console et le tour d'accueil : titre échappé, scripts choisis (`page_scripts`), rafraîchissement sans script (`refresh_to`). |
| REL-38 | Un nettoyeur de noms commun (`clean_text`, contrôle et bidi retirés) ; le nom affiché (`Account::display_name`, `shown_name`) et « toute la famille / l'équipe » (`Org::everyone`) écrits une fois. |
| REL-39 | Les fonctions réservées aux essais passent sous `#[cfg(test)]` (`is_ready`, `invite_expires_at`, `Store::memory`, `audit_of`, `Hasher::fast`, `Outbox::last_to`/`count_to`). |
| REL-40 | L'avis « Admin ouvert à distance » passe par le `Limiter` commun (une fois par heure), plus de `static` à part. |
| REL-41 | L'e-mail « Admin ouvert » dit pourquoi un gestionnaire d'équipe le reçoit ; la doc des comptes dit ce que l'e-mail dit (sans lieu : le relais ne situe pas les adresses). |
| REL-42 | README FR et EN : toutes les pages en POST, les deux scripts, la mise à jour qui n'écrase plus une image construite sur place ; commentaire du fil SMTP corrigé ; version 0.11.8 dans `Cargo.toml` (celle de `/v1/health`). |

## Ce qui reste, hors de cette zone

| Id | Ce qui manque, et où |
|---|---|
| REL-09 (apps) | Le cœur Rust et les apps doivent envoyer `code_challenge` (S256) puis `code_verifier` (`apps/core`, apps). Le relais est prêt et accepte toujours les apps sans défi. |
| REL-10 (jeton en POST ou dans le fragment) | Admin 0.11.7 lit `?token=` : il faut d'abord changer la réception chez Admin (`server/admin`). Côté relais, le jeton ne part plus que vers les adresses que la Station a données. |
| REL-20 (signature) | Signer les images (cosign, clé hors ligne) au moment de la publication (`scripts/release.sh`), puis mettre `RELAY_COSIGN_KEY` sur la machine réelle. `update.sh` vérifie déjà la signature quand la clé est donnée. |

Deux points restent en partie, par choix et dans la zone :
- REL-16 : yamux ne baisse pas sa limite de flux pendant une connexion (une Station en reçoit un par connexion relayée, jusqu'à 8 × 32). La lecture au fil de l'arrivée et `tcp_total` bornent déjà la mémoire.
- REL-35 (QUIC) : le paquet reçu de quinn n'est pas à nous seuls (`Bytes` partagé), donc le nouveau numéro de flux demande une copie ; la tâche par paquet de secours ne sert qu'aux rares paquets trop grands pour un datagramme (64 en route au plus).

## Compatibilité avec les apps et les Stations publiées (0.11.7)

- Protocole du tunnel inchangé : `MediaSender::close` est un ajout ; `read_msg` lit la même trame. Les apps n'ouvrent ni flux bidirectionnels en `kn-media/1` ni unidirectionnels en `kn-ctl/1` ; le repli QUIC des paquets (flux unidirectionnels en média) reste permis.
- Connexion de présence sans Station (`presence`) : elle se ferme d'elle-même en 5 s, bien avant le ménage (30 s).
- API des appareils : `code_verifier` n'est exigé que si l'app a donné un défi.
- Base : migration 10 (`grants.right_ends`, `device_codes.challenge`) ; une base migrée refuse un relais plus ancien, comme toujours.
- Cookies d'attente de l'ancienne forme acceptés ; anciens codes de secours acceptés ; codes e-mail en clair encore en cours (24 h) acceptés.
- Repli TCP : les enregistrements sur le fil sont les mêmes (`[longueur][datagramme]`).
- Invitations d'avant ce changement : celles en attente pour une adresse avec compte apparaissent dans « Invitations » ; celles déjà reprises restent des droits.

## Ce qui est vérifié, et comment

- `cargo test --workspace` dans `server/relay` : 233 essais passent (169 du relais, 23 d'intégration du tunnel, 34 + 7 du crate `tunnel`), contre 205 avant ; deux passes de suite sans échec à la fin ; `cargo build` sans avertissement. Une fois, en cours de travail, `un_bout_qui_ne_lit_pas_ses_reponses_est_coupe` a lu `Reset` au lieu de la fermeture attendue ; il est passé aux sept passes suivantes (seul et dans la suite) ; probablement instable sous charge, à surveiller.
- Essais ajoutés : noms hostiles dans les pages (et vérifié qu'il échoue sans le correctif), échéances par droit, délégation bornée et cascade, départ d'une équipe, invitation fusionnée, mot de passe redemandé, passkeys retirées à la réinitialisation, TOTP illisible, refus de démarrer sans clé, inscription sur une adresse prise, limite d'invitations, plafond d'équipes, essais de connexion par réseau et navigateur de confiance, adresses d'Admin, PKCE (vecteur de la RFC 7636), connexion relayée muette, trame annoncée longue et coupée, HSTS, secrets en HMAC, Administrateur qui ne sort pas un PC, jetons d'une famille, `atomic` imbriqué, nettoyeur bidi ; puis invitations acceptées ou refusées et même réponse avec ou sans compte, fin de jour selon le fuseau (New York, Paris, bornes), débit GCRA à plusieurs fils, enregistrements dans un tampon réutilisé. Les essais qui supposaient l'ajout direct acceptent maintenant l'invitation (`World::accept_invites`).
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
- Quand on vous ouvre un ordinateur ou qu'on vous invite dans une famille ou une équipe, vous acceptez ou refusez l'invitation dans votre espace, page « Invitations » : rien ne vous est ouvert sans votre accord.
- « Jusqu'au 15/12 » se termine à minuit chez vous, où que vous soyez.

EN:
- KyberNow Share is safer: a computer or team name can no longer change the pages, and someone you give “Administer” to can no longer give more than they have, or for longer. If you remove their access, what they shared goes with it.
- Each right keeps its own end date: an endless “Watch” link no longer extends a limited access.
- Leaving a team also removes access to its computers.
- To add a passkey or turn on two-factor authentication in a session opened a while ago, you are asked for your password again. A new password removes passkeys added in the last few days.
- Nobody can block your sign-in any more by trying wrong passwords from another network.
- Invitations are limited per account, to prevent mass sending.
- “Open Admin” only sends your access to the address your computer gave.
- When someone opens a computer to you or invites you to a family or team, you accept or decline the invitation in your space, “Invitations” page: nothing is opened to you without your consent.
- “Until 15/12” ends at midnight where you are.
