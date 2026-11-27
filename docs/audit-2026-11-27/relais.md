# Audit du 2026-11-27 : relais KyberNow Share (`server/relay`)

Ce rapport porte sur `server/relay` : le relais public `share.kybernow.app`, avec ses comptes, ses équipes, ses invitations, ses liens d'accès, Admin à distance et le crate `tunnel/`. L'audit s'appuie sur l'état de `main` au commit `1fb289c6` (0.11.7). C'est une analyse statique seulement : rien n'a été compilé, lancé ni modifié en dehors de ce fichier, et personne ne s'est connecté à la machine réelle.

Quatre relectures ont été menées en parallèle : authentification, droits, tunnel, puis pages, e-mails et déploiement. **Chaque constat gardé ici a été relu dans le code.** Quand le comportement dépend d'un autre composant (Admin, les apps, quinn), le constat est noté « plausible ».

L'audit précédent (`docs/archives/audit-2026-10-20.md`) est antérieur au relais : il n'y a donc aucun recoupement.

**Hors du relais : le prêt du PC.** Les codes de prêt, les comptes d'amis et les créneaux vivent sur la Station (Admin, page « Prêt »). Le relais ne connaît que le droit `Right::Lend` et des textes. Les courses entre créneaux, l'heure d'été et un emprunteur qui reste après la fin de son créneau sont donc à vérifier dans l'audit de la Station Windows.

## Synthèse

| Id | Catégorie | Constat | Lieu | Effort | Confiance |
|---|---|---|---|---|---|
| REL-01 | sécurité critique | Nom d'ordinateur ou d'équipe injecté sans échappement dans `<title>` | `console_pages.rs:1011`, `:2016` | S | vérifié |
| REL-02 | sécurité | Droit Admin délégué non borné : il dépasse la durée et les droits de celui qui le donne, et survit à son retrait | `web/link_pages.rs:42-50`, `web/console.rs:432-565` | M | vérifié |
| REL-03 | sécurité | Accepter un lien fusionne les échéances : un lien « Regarder » sans fin rend un Admin limité permanent | `links.rs:173-186` | M | vérifié |
| REL-04 | sécurité | Retirer quelqu'un d'une équipe ne retire pas ses droits individuels sur les PC de l'équipe | `orgs.rs:437-443` | S | vérifié |
| REL-05 | sécurité | Inscription : un tiers peut prendre un compte pas encore vérifié (cookie d'attente plus « changer d'adresse ») | `web.rs:752-759`, `:825-846` | S | vérifié |
| REL-06 | sécurité | Passkey et double authentification ajoutées sans redemander le mot de passe ; la réinitialisation du mot de passe ne retire ni les passkeys ni la double authentification | `passkeys.rs:474-478`, `web.rs:966-971`, `:943-954` | S | vérifié |
| REL-07 | sécurité | Double authentification ouverte (« fail-open ») si le secret ne se déchiffre plus | `web.rs:672-679` | S | vérifié |
| REL-08 | sécurité | N'importe qui peut bloquer la connexion par mot de passe d'un compte (5 échecs par compte, toutes adresses confondues) | `mod.rs:217`, `web.rs:595` | S/M | vérifié |
| REL-09 | sécurité | Code de retour vers l'app (`kybernow://compte`) sans PKCE | `devices.rs:485-509` | M | plausible |
| REL-10 | sécurité | « Ouvrir Admin » envoie un jeton vers n'importe quelle adresse locale, même en `http` | `web/admin_login.rs:42-73`, `:167` | S | vérifié (côté relais) |
| REL-11 | sécurité | Ajout direct dans une équipe sans consentement, et la réponse révèle si un compte existe | `web/org_pages.rs:219-241`, `web/console.rs:446-475` | M | vérifié |
| REL-12 | sécurité | Un Administrateur d'équipe (pas Propriétaire) peut sortir un PC de l'équipe | `moves.rs:159-164` | S | vérifié |
| REL-13 | sécurité | Aucune limite d'invitations par expéditeur (spam et hameçonnage par nos e-mails) | `mod.rs:222`, `web/org_pages.rs:207-257` | S/M | vérifié |
| REL-14 | sécurité | QUIC : chaque connexion peut immobiliser environ 16 Mio de mémoire du relais | `tunnel/src/quic.rs:103-115` | S | plausible |
| REL-15 | sécurité | Quelques adresses occupent toutes les places d'une Station avec des flux `admin_tls` muets | `hub.rs:99-102`, `:787-790` | M | vérifié (côté relais) |
| REL-16 | sécurité | Repli TCP : pas de plafond global, et 64 Kio alloués par flux sur la seule foi de la longueur annoncée | `tcp.rs:57`, `tunnel/src/frame.rs:47-52` | S/M | vérifié |
| REL-17 | sécurité | Corps HTTP lu sans délai, et une seule file de 8 places pour tout `/compte` et l'API | `web.rs:261`, `:269`, `:329`, `mod.rs:287` | S | vérifié |
| REL-18 | sécurité | Limites par adresse calculées sur l'adresse IPv6 exacte | `limits.rs:153`, `quic.rs:81`, `hub.rs:382-397` | S | vérifié (latent) |
| REL-19 | sécurité | Client sans Station jamais expiré, aucune limite sur les messages de contrôle | `hub.rs:652`, `:742-789`, `:856` | S | vérifié |
| REL-20 | sécurité | Mise à jour nocturne de l'image `:latest` par `docker.sock`, sans signature | `docker-compose.yml:94-111`, `update.sh:19-20` | M | vérifié |
| REL-21 | sécurité | Pas d'en-tête HSTS | `web.rs:64-75` | S | vérifié |
| REL-22 | sécurité | Secrets courts mal gardés en base (codes de secours en SHA-256 sans sel, codes e-mail en clair) | `web.rs:1018-1026`, `store.rs:36` | S | vérifié |
| REL-23 | sécurité | Chemin de la requête renvoyé sans échappement (« Formulaire expiré ») | `web.rs:408-414` | S | vérifié (durcissement) |
| REL-24 | bug | « Dernier Propriétaire » vérifié puis écrit hors transaction : course possible | `web/org_pages.rs:303-306`, `:349-352` | S | plausible |
| REL-25 | bug | `detach_station` : six requêtes sans transaction | `stations.rs:331-347` | S | vérifié |
| REL-26 | bug | Enrôlement : jeton compté dans une transaction, rattachement dans d'autres | `orgs.rs:760-768` | S | vérifié |
| REL-27 | bug | Jetons d'enrôlement pour une famille possibles par un POST direct | `web/org_pages.rs:445-509` | S | vérifié |
| REL-28 | bug | La mise à jour nocturne remplace une image construite depuis les sources | `docker-compose.yml:16-20`, `update.sh:19-20` | S | vérifié |
| REL-29 | bug | Échéance « jusqu'au JJ/MM » fixée à minuit UTC | `web/console.rs:79` | S | vérifié |
| REL-30 | bug | Une invitation sur un PC où le compte a déjà des droits est effacée au lieu d'être fusionnée | `people.rs:318-324` | S | vérifié |
| REL-31 | bug | `Agent::reserve` (Station) laisse sa demande en attente après le délai | `tunnel/src/station.rs:383-393` | S | vérifié |
| REL-32 | bug | Nom donné à l'inscription non nettoyé (caractères de contrôle, bidi) | `web.rs:716` | S | vérifié |
| REL-33 | optimisation | Verrou global de la base tenu pendant toute la construction des pages, avec des requêtes N+1 | `web/org_pages.rs:59-126`, `web/console.rs:30` | M | vérifié |
| REL-34 | optimisation | Verrous globaux pris à chaque datagramme relayé | `hub.rs:907`, `:922`, `:934` | S | vérifié |
| REL-35 | optimisation | Allocation et copie par paquet relayé | `tunnel/src/transport.rs:490`, `:506`, `:522`, `frame.rs:93`, `:122` | M | vérifié |
| REL-36 | optimisation | `AttachRequest` traité sur l'exécuteur async, sans limite de débit | `hub.rs:498`, `accounts/stations.rs:221-229` | S | vérifié |
| REL-37 | factorisation | Gabarit HTML copié trois fois, détection des passkeys copiée quatre fois (à l'origine de REL-01) | `pages.rs:355`, `console_pages.rs:229`, `:2290` | M | vérifié |
| REL-38 | factorisation | Trois ou quatre nettoyeurs de noms identiques | `devices.rs:236`, `stations.rs:351`, `passkeys.rs:534` | S | vérifié |
| REL-39 | code mort | Fonctions de production utilisées par les essais seulement | `certs.rs:70`, `people.rs:144`, `store.rs:299`, `:768`, `password.rs:85`, `mail.rs:50-57` | S | vérifié |
| REL-40 | factorisation | Deux limites d'e-mails parallèles (`Limiter.mail_to` et le `LAST` statique) | `stations.rs:393` | S | vérifié |
| REL-41 | doc | L'e-mail « Admin ouvert » ne dit pas ce que la doc annonce | `mail.rs:356`, `:369`, `docs/comptes.md:170`, `:199` | S | vérifié |
| REL-42 | doc | README et commentaires périmés : `compose build`, routes POST, version 0.8.0, pool SMTP | `README.md`, `Cargo.toml:11`, `mail.rs:62` | S | vérifié |

Décompte : 1 sécurité critique, 22 sécurité, 9 bugs, 4 optimisations, 3 factorisations, 1 code mort, 2 doc (42 constats).

---

## Sécurité

### REL-01 — Nom d'ordinateur ou d'équipe injecté dans `<title>` (sécurité critique)

- **Lieu.** `src/accounts/console_pages.rs:1011` appelle `frame(v, &p.shell, …, &p.name, …)`, et `:2016` appelle `frame(…, &o.name, …)`. `frame` (`:229`) écrit `title` tel quel dans `<title>{title} · KyberNow</title>` (`:248`).
- **Pourquoi c'est un bug.** Les autres appelants échappent avant l'appel (par exemple `:1082`, `esc(&m.name)`). La même fonction échappe d'ailleurs le nom dans le corps de la page.
- **Ce qui laisse passer l'attaque.** `stations::clean_name` (`stations.rs:351`), repris par les organisations (`orgs.rs:295`), ne retire que les caractères de contrôle : `<` et `"` restent.
- **Scénario.** Le propriétaire d'une famille ou d'une équipe la renomme `</title><meta http-equiv="refresh" content="0;url=https://piege.example">`. Chaque membre qui ouvre la page de l'équipe est envoyé sur le site piège, ce qui fait un hameçonnage parfait, parti de `share.kybernow.app`. Une Station peut faire de même avec son propre nom, vu par tous ceux à qui le PC est ouvert. La CSP (`script-src 'self'`) bloque les scripts en ligne, mais ni `meta refresh`, ni un `<form>` injecté, ni une fausse page.
- **Correctif.** `frame` et `page_with` reçoivent toujours du texte brut et l'échappent elles-mêmes ; retirer alors l'échappement fait par les appelants. Ajouter un essai qui affiche chaque page de la console avec des noms contenant `<"'&`. Voir aussi REL-37.
- **Effort** S. **Confiance** vérifié.

### REL-02 — Droit Admin délégué sans borne

- **Lieu.** `web/link_pages.rs:42-50` (création d'un lien), `web/console.rs:432-470` (`add`) et `:530-565` (`open_to`). `people.rs:426` : `can_manage` est vrai dès que la personne a `Right::Admin`.
- **Problème.** Celui qui a reçu Admin peut créer des liens, ajouter des personnes ou ouvrir la Station à une équipe qu'il possède. Rien ne borne l'échéance de ce qu'il donne à sa propre échéance, ni les droits donnés à ses propres droits. Et ce qu'il a donné ne tombe pas quand ses propres droits tombent.
- **Scénario.** Le propriétaire donne Admin à X jusqu'à vendredi. X crée un lien Admin sans fin et l'accepte avec un second compte : seul le créateur du lien est refusé (`link_pages.rs:161`). Samedi, X n'a plus rien, mais son second compte garde Admin pour toujours. Retirer X n'y change rien.
- **Correctif.** Pour qui n'est ni propriétaire ni gestionnaire de l'équipe, borner `until` à sa propre échéance et les droits aux siens. Mieux : se servir de `invited_by` (déjà en base) pour retirer en cascade ce qu'une personne a donné quand on la retire.
- **Effort** M. **Confiance** vérifié.

### REL-03 — Les échéances fusionnent à l'acceptation d'un lien

- **Lieu.** `links.rs:173-186` (`add_person_rights`).
- **Problème.** Un seul enregistrement `grants` porte l'union des droits et l'échéance la plus lointaine ; « sans échéance » l'emporte toujours.
- **Scénario.** Léa a Admin jusqu'au 31/12. Le propriétaire lui envoie ensuite un lien « Regarder » sans échéance. Après acceptation, son Admin n'a plus de fin.
- **Correctif.** Ne prolonger l'échéance que si les nouveaux droits couvrent tous les anciens. Sinon, garder une échéance par droit (une ligne par droit, ou une colonne d'échéance par droit).
- **Effort** M. **Confiance** vérifié.

### REL-04 — Retirer un membre laisse ses droits individuels

- **Lieu.** `orgs.rs:437-443` (`remove_member`) : la fonction n'efface que `group_members` et `org_members`.
- **Scénario.** Un salarié a reçu Session ou Admin sur un PC de l'équipe, en direct ou par un lien. On le retire de l'équipe : il garde l'accès au PC. Les droits de groupe sont bien liés à l'appartenance ; les droits par Station ne le sont pas.
- **Correctif.** Dans la même transaction, `DELETE FROM grants WHERE account = ? AND station IN (SELECT id FROM stations WHERE org = ?)`. À défaut, le dire dans la boîte de confirmation du retrait.
- **Effort** S. **Confiance** vérifié.

### REL-05 — Prise d'un compte pas encore vérifié

- **Lieu.** `web.rs:752-759` : une inscription sur une adresse déjà prise pose quand même `set_pending(&email)`. Ensuite `web.rs:825-846` (`action=changer`) appelle `set_email` sur le compte non vérifié qui porte l'adresse en attente.
- **Scénario.**
  1. Une victime s'inscrit et ne vérifie pas tout de suite.
  2. L'attaquant s'inscrit avec l'adresse de la victime et reçoit le cookie d'attente de cette adresse.
  3. Il choisit « changer d'adresse » vers `attaquant@x`. Le compte de la victime change d'adresse, et le code de vérification part chez l'attaquant.
  4. L'attaquant vérifie : `verified()` lui ouvre une session (`web.rs:771-773`), et la victime a perdu son compte.

  Le gain est limité, puisqu'un compte non vérifié n'a encore rien, mais l'attaque coûte une inscription et peut bloquer quelqu'un qui s'inscrit.
- **Correctif.** Ne pas poser le cookie d'attente quand l'adresse est prise, ou signer dans le cookie l'identifiant du compte créé par ce navigateur, et n'autoriser « changer » que pour ce compte-là.
- **Effort** S. **Confiance** vérifié.

### REL-06 — Passkey et double authentification ajoutées sans revérification ; la réinitialisation ne les retire pas

- **Lieu.**
  - `passkeys.rs:474-478` (`add`) et `web.rs:966-971` (`totp_setup`) ne demandent qu'une session complète. À l'inverse, `totp_off` demande le mot de passe (`web.rs:1064`).
  - La réinitialisation du mot de passe (`web.rs:943-954`) ferme les sessions, les appareils et les navigateurs de confiance, mais pas `passkeys` ni `totp`.
- **Scénario.** Un cookie de session est volé (poste partagé, logiciel espion). L'attaquant ajoute sa passkey. La victime réinitialise son mot de passe : l'attaquant se reconnecte quand même par passkey, et la double authentification est comptée comme faite. Variante : l'attaquant active la double authentification avec son propre secret, et la victime ne peut plus se connecter, même après une réinitialisation (`needs_code`, `web.rs:624`).
- **Correctif.** Demander le mot de passe (ou une passkey existante) avant d'ajouter une passkey ou d'activer la double authentification. Après une réinitialisation, retirer les passkeys ajoutées depuis moins de N jours, ou les montrer en évidence avec un bouton « Retirer ».
- **Effort** S. **Confiance** vérifié.

### REL-07 — Double authentification ouverte si le secret ne se déchiffre plus

- **Lieu.** `web.rs:672-679` : `match totp.and_then(|t| acc.keys.open(…))`. Si le déchiffrement échoue, on tombe dans `None => true` (« plus de double authentification »). Or `mod.rs:133-146` recrée sans bruit `secret.key` s'il manque.
- **Scénario.** On restaure `data/` sans `secret.key`, ou on change la clé : chaque compte protégé se connecte alors avec le seul mot de passe, et rien n'est journalisé.
- **Correctif.** Distinguer « pas de ligne TOTP activée » (laisser passer) de « ligne présente mais illisible » (refuser et écrire une erreur). Refuser de démarrer si la base contient des TOTP activés alors que la clé vient d'être créée.
- **Effort** S. **Confiance** vérifié.

### REL-08 — Blocage ciblé de la connexion par mot de passe

- **Lieu.** `mod.rs:217` (`sign_in_account`, 5 échecs par 15 min, rendus à raison d'un toutes les 3 min) ; `web.rs:595`, qui pose la limite avant même de vérifier le mot de passe.
- **Scénario.** Un script envoie un mauvais mot de passe toutes les 3 minutes pour `victime@x`. La victime reçoit « Trop d'essais » indéfiniment, même avec le bon mot de passe. Le même seau sert aussi à couper la double authentification et à supprimer le compte (`web.rs:1056`, `data.rs:40`). Les passkeys échappent au blocage (seau par adresse).
- **Correctif.** Ne pas compter le seau par compte pour un navigateur qui présente un `__Host-kn_confiance` valide de ce compte. Indexer le seau sur (compte, réseau /24), avec un plafond global par compte beaucoup plus haut. Ou remplacer le refus par des délais croissants.
- **Effort** S/M. **Confiance** vérifié.

### REL-09 — Pas de PKCE sur le retour vers l'app

- **Lieu.** `devices.rs:485-509` : `POST /v1/devices/token` échange le code de retour (5 min, usage unique) sans rien d'autre. `web/device_pages.rs:141-150` envoie ce code dans `kybernow://compte?code=…`.
- **Scénario.** Sur Windows, Linux ou Android, une autre app enregistre le schéma `kybernow://`, intercepte le retour et échange le code avant l'app légitime. Elle obtient une session d'appareil sur le compte. Sur macOS et iOS, `ASWebAuthenticationSession` limite ce risque. Le paramètre `etat` protège contre le CSRF, pas contre l'interception.
- **Correctif.** PKCE S256 : l'app envoie `code_challenge` à `/compte/connexion?appareil=…`, le relais le garde dans `device_codes`, puis l'app présente `code_verifier` à `/v1/devices/token`. Cela touche le cœur Rust et les apps.
- **Effort** M. **Confiance** plausible (dépend des plateformes).

### REL-10 — « Ouvrir Admin » vers une adresse locale quelconque

- **Lieu.** `web/admin_login.rs:42-73` (`local_origin`) accepte `http://`, n'importe quelle IP privée et n'importe quel nom sans point ou en `.local`, `.lan` ou `.home.arpa`. Le `nonce` vient de la requête (`:90`), et le jeton part dans l'URL (`:167`, `?token=`).
- **Scénario.**
  1. Un attaquant sur le même réseau ouvre lui-même le parcours sur Admin pour obtenir un nonce.
  2. Il envoie à la victime un lien `…/compte/ouvrir-admin?station=<id>&nonce=<le sien>&retour=http://192.168.1.66`.
  3. La victime confirme : la page montre bien l'hôte, mais une adresse IP ne lui dit rien.
  4. Le jeton signé (2 min) part chez l'attaquant, qui le présente à Admin avec son nonce.
- **Correctif.** Que la Station publie ses origines locales (le relais connaît déjà `lan` par `stations.admin(...)`) et n'accepter qu'elles, en `https`. Passer le jeton en POST, ou dans le fragment de l'URL.
- **Effort** S. **Confiance** vérifié côté relais. Le lien entre le nonce et le navigateur, côté Admin, n'a pas été relu.

### REL-11 — Ajout sans consentement et révélation des comptes

- **Lieu.** `web/org_pages.rs:219-241` : une adresse vérifiée est ajoutée tout de suite (`set_member`). La réponse est alors `MemberAdded` au lieu de `InviteSent`, et `AlreadyMember(name)` donne le nom. Même logique dans `web/console.rs:446-475` (`PersonAdded` contre `InviteSent`).
- **Scénario.** N'importe qui crée une équipe (il suffit d'activer la double authentification) et teste des adresses : il apprend qui a un compte et sous quel nom, et voit nom et adresse dans l'onglet Membres. La personne visée se retrouve dans une équipe qu'elle n'a pas choisie.
- **Correctif.** Toujours créer une invitation que la personne accepte ; toujours répondre « Invitation envoyée ».
- **Effort** M. **Confiance** vérifié.

### REL-12 — Un Administrateur d'équipe sort un PC de l'équipe

- **Lieu.** `moves.rs:159-164` (`may_move`) accepte tout rôle qui `manages()`. Même chose pour `detach_by_account` (`stations.rs:516`).
- **Scénario.** Un Administrateur déplace un PC de l'équipe « vers moi » avec `garder=0`. Les Propriétaires perdent le PC et tous les accès sont effacés. Cela contredit la règle selon laquelle « un Administrateur ne touche pas aux Propriétaires ».
- **Correctif.** Exiger `Role::Owner` pour sortir un PC d'une équipe, ou au minimum prévenir les Propriétaires par e-mail et journal.
- **Effort** S. **Confiance** vérifié dans le code ; l'intention n'est pas écrite dans `docs/comptes.md`.

### REL-13 — Invitations sans limite par expéditeur

- **Lieu.** La seule limite porte sur le destinataire : 3 e-mails par 15 min (`mod.rs:222`, `may_mail`). Les invitations d'équipe (`web/org_pages.rs:207-257`) et le partage d'un PC (`web/console.rs:489-501`) n'ont aucune limite par compte ni par équipe, et le nombre d'équipes par compte n'est pas borné.
- **Scénario.** Un compte vérifié crée l'équipe « Votre identifiant Apple est bloqué — allez sur … » et invite 10 000 adresses. Chacune reçoit « {nom} vous invite dans {équipe} » depuis notre domaine : le compte SendGrid et la réputation de `kybernow.app` sont en jeu.
- **Correctif.** Un seau par compte pour les invitations sortantes (par exemple 20 par heure et 100 par jour), avec une ligne au journal quand il est plein ; un plafond d'équipes par compte.
- **Effort** S/M. **Confiance** vérifié.

### REL-14 — QUIC : 16 Mio immobilisés par connexion

- **Lieu.** `tunnel/src/quic.rs:103-115` : tant que l'ALPN n'est pas connu, chaque connexion entrante reçoit 256 flux bidirectionnels, 4 Mio par flux et 16 Mio par connexion. Sur `kn-media/1`, le relais n'accepte jamais de flux bidirectionnel (`hub.rs:881`).
- **Scénario.** Un client ouvre des connexions média avec un jeton quelconque, puis remplit des flux bidirectionnels que personne ne lit. quinn garde les données jusqu'à la fenêtre de la connexion. Avec 16 connexions par IPv4, cela fait environ 256 Mio par adresse ; une vingtaine d'adresses suffisent pour plusieurs Gio. De plus, chaque nouvelle connexion média remplace l'émetteur (`hub.rs:920`) sans fermer l'ancienne.
- **Correctif.** Dès que l'ALPN est connu : `set_max_concurrent_bi_streams(0)` en média et `set_max_concurrent_uni_streams(0)` en contrôle. Fermer l'ancienne connexion média d'un client quand une nouvelle la remplace.
- **Effort** S. **Confiance** plausible (dépend de la mise en mémoire de quinn pour les flux non acceptés).

### REL-15 — Places d'une Station tenues par des flux muets

- **Lieu.** `hub.rs:99-102` : un client est occupé dès que `streams > 0`, et un client occupé n'est jamais évincé (`admit`, `hub.rs:154-170`). `hub.rs:787-790` : un flux `admin_tls` s'ouvre pour tout client, et la copie (`copy_bidirectional`) n'a pas de délai d'inactivité.
- **Scénario.** Quatre adresses, avec deux clients chacune et un flux `admin_tls` silencieux par client, remplissent les 8 places (`clients_per_station`). Les vrais utilisateurs lisent alors « trop de clients pour cette Station ». Il suffit de connaître l'identifiant de la Station, qui figure dans les liens de partage.
- **Correctif.** Mettre un délai d'inactivité sur les flux relayés. Ne compter un client comme occupé qu'une fois un flux de session accepté par la Station. Plafonner les flux par client.
- **Effort** M. **Confiance** vérifié côté relais ; Admin ferme peut-être les connexions muettes, ce qui n'a pas été vérifié.

### REL-16 — Repli TCP : pas de plafond global, allocation avant lecture

- **Lieu.** `tcp.rs:57` ne limite que `tcp_per_ip` (32) : il n'y a pas d'équivalent à `quic_total`. yamux accepte 256 flux par connexion. `tunnel/src/frame.rs:47-52` fait `vec![0; len]` (jusqu'à 64 Kio) sur la seule longueur annoncée, puis attend 10 s le corps.
- **Scénario.** 32 connexions × 256 flux × 64 Kio, soit environ 512 Mio par adresse, renouvelables toutes les 10 s.
- **Correctif.** Ajouter `tcp_total`, moins de flux avant le `hello`, et une lecture du corps par morceaux.
- **Effort** S/M. **Confiance** vérifié.

### REL-17 — Corps HTTP sans délai, une seule file pour les comptes

- **Lieu.** `web.rs:261` et `:329` : `Limited::new(…).collect().await` n'a pas de délai ; seul `header_read_timeout` est réglé (`web.rs:136`). `web.rs:269` : `gate.acquire().await`, sans délai, sur un sémaphore de 8 places (`mod.rs:287`) partagé par toutes les pages `/compte` et toute l'API des appareils, y compris `/v1/access`, nécessaire pour ouvrir une session.
- **Scénario.** Des connexions envoient leur corps un octet à la fois (32 par adresse, pas de plafond global). Ou bien des essais de connexion venus de nombreuses adresses gardent Argon2 occupé (19 Mio et environ 50 ms chacun). Les pages et `/v1/access` attendent alors en file, et plus personne n'ouvre de session distante.
- **Correctif.** `tokio::time::timeout` (10 s) autour de la lecture du corps. Un plafond global de connexions web. Le sémaphore réservé à Argon2, avec un délai d'attente qui rend 503.
- **Effort** S. **Confiance** vérifié (code) ; l'effet réel dépend de la charge.

### REL-18 — Limites par adresse IPv6 exacte

- **Lieu.** `ConnCounter` (`limits.rs:153`), la limite de poignées de main QUIC (`quic.rs:81`), `station_hellos` et `stations_per_ip` (`hub.rs:382-397`) utilisent l'adresse complète. L'admission des sièges, elle, passe par `ip_key` en /64.
- **Scénario.** Un hôte IPv6 dispose d'un /64 entier, donc de limites pratiquement infinies. Le risque est latent, puisque `docker-compose.yml` écoute sur `0.0.0.0` par défaut, mais il devient réel dès qu'on écoute sur `[::]`.
- **Correctif.** Passer `ip_key()` partout.
- **Effort** S. **Confiance** vérifié.

### REL-19 — Clients sans Station, messages sans limite

- **Lieu.** Les clients sans Station échappent à l'admission et au ménage des clients inactifs (`hub.rs:652`, `:856`). `status` répond en ligne ou hors ligne pour 16 Stations par message (`hub.rs:761`), sans limite de débit. `open_flow` et `open` sont relayés à la Station sans limite (`hub.rs:742`, `:789`).
- **Scénario.** Sonder en continu quelles Stations sont en ligne, ou inonder l'Admin d'une Station de demandes refusées.
- **Correctif.** Un seau par connexion pour les messages de contrôle et l'ouverture de flux, et un délai d'inactivité pour un client sans Station.
- **Effort** S. **Confiance** vérifié.

### REL-20 — Mise à jour nocturne sans signature, avec `docker.sock`

- **Lieu.** Le service `updater` (`docker-compose.yml:94-111`) monte `/var/run/docker.sock`, ce qui équivaut à root sur la machine. `update.sh:19-20` fait `pull` puis `up` de `ghcr.io/monsieurnours/kybernow-relay:latest`, sans empreinte ni signature.
- **Scénario.** Un jeton GitHub qui fuit, ou un compte compromis, et la nuit suivante le relais public exécute le code de l'attaquant, avec la base des comptes et la clé du certificat. Le conteneur du relais lui-même est bien durci : non-root, `read_only`, `cap_drop: ALL`.
- **Correctif.** Signer les images (cosign, clé hors ligne) et vérifier la signature dans `update.sh` avant `up`, ou épingler une version ou une empreinte et ne monter de version qu'à la main.
- **Effort** M. **Confiance** vérifié.

### REL-21 — Pas de HSTS

- **Lieu.** `web.rs:64-75` (`html()`) : aucun `strict-transport-security`, nulle part dans `src/`.
- **Scénario.** Sur un Wi-Fi hostile, la première visite à `share.kybernow.app` en `http://` peut être détournée.
- **Correctif.** `max-age=31536000` sur les pages du relais. Ne mettre `includeSubDomains` qu'après avoir décidé pour les sous-domaines des Stations (toujours servis en TLS : a priori sans risque).
- **Effort** S. **Confiance** vérifié.

### REL-22 — Secrets courts mal gardés en base

- **Lieu.** `web.rs:1018-1026` : les codes de secours font 40 bits et sont gardés en `token_hash`, un SHA-256 sans sel. `store.rs:36` : `email_tokens.code` est en clair.
- **Scénario.** Seulement en cas de fuite de la base : les codes de secours se retrouvent en minutes sur un GPU, et les codes e-mail en cours se lisent directement.
- **Correctif.** Un HMAC avec la clé du relais (`Keys`) à la place du SHA-256 ; le code e-mail gardé en HMAC.
- **Effort** S. **Confiance** vérifié.

### REL-23 — Chemin non échappé sur « Formulaire expiré »

- **Lieu.** `web.rs:408-414` passe `path` à `pages::message`, qui l'écrit dans `href="…"` sans `esc`.
- **Scénario.** Peu exploitable, puisque les navigateurs encodent `"`, mais c'est le seul endroit où une donnée de la requête entre dans une page sans échappement.
- **Correctif.** `esc(path)`.
- **Effort** S. **Confiance** vérifié (durcissement).

---

## Bugs

### REL-24 — Course sur le « dernier Propriétaire »

- **Lieu.** `web/org_pages.rs:303-306` (`set_role`) et `:349-352` (`leave`) : `owners()` puis `set_member` ou `remove_member`, chacun sous son propre verrou. Huit requêtes de comptes peuvent tourner à la fois.
- **Scénario.** Deux Propriétaires se rétrogradent ou partent en même temps : chacun voit deux Propriétaires, et l'équipe n'en a plus aucun. Rien dans l'interface ne permet de rattraper.
- **Correctif.** Vérifier et écrire dans une seule requête ou une seule transaction (`… WHERE (SELECT COUNT(*) …) > 1`).
- **Effort** S. **Confiance** plausible.

### REL-25 — `detach_station` non atomique

- **Lieu.** `stations.rs:331-347` : six `DELETE` sans transaction, et pas de clé étrangère de `grants` ni de `access_links` vers `stations`.
- **Scénario.** Une erreur d'E/S après `DELETE FROM stations` laisse des droits orphelins. Si la même machine est rattachée plus tard par quelqu'un d'autre (`attach_to` ne nettoie que s'il trouve une ligne existante), les anciens invités retrouvent l'accès.
- **Correctif.** `unchecked_transaction`, comme `attach_to`.
- **Effort** S. **Confiance** vérifié.

### REL-26 — Enrôlement en plusieurs transactions

- **Lieu.** `orgs.rs:760-768` : `used + 1` est validé, puis `attach_station_to_org`, `grant_group` et `set_station_group_member` passent hors de la transaction.
- **Scénario.** Un échec consomme une place du jeton pour rien, ou laisse une Station rattachée sans les droits de son groupe.
- **Correctif.** Une seule transaction.
- **Effort** S. **Confiance** vérifié.

### REL-27 — Jetons d'enrôlement pour une famille

- **Lieu.** `web/org_pages.rs:445-509` : ni `deploy` ni `token_create` ne regardent `org.kind`. Seule l'interface cache le bouton pour une famille.
- **Correctif.** Refuser quand `Kind::Family`.
- **Effort** S. **Confiance** vérifié.

### REL-28 — La mise à jour remplace une image construite localement

- **Lieu.** `docker-compose.yml:16-20` : `build:` et `image: …:latest` se combinent, donc `docker compose build relay` étiquette la construction locale comme l'image publiée. À 4 h, `update.sh:19-20` fait `pull` puis `up --no-build` et la remplace.
- **Correctif.** Une étiquette locale distincte (`kybernow-relay:local`), ou `update.sh` qui ne fait rien quand l'image a été construite sur place. Le dire dans README.md et README.en.md (« Depuis les sources »).
- **Effort** S. **Confiance** vérifié.

### REL-29 — Échéance à minuit UTC

- **Lieu.** `web/console.rs:79` : `end = (days + 1) * 86_400`, c'est-à-dire minuit UTC du jour choisi.
- **Scénario.** « Jusqu'au 15/12 » se termine à 1 h (heure d'hiver) ou à 2 h (heure d'été) du matin en France, ce qui passe. Pour un utilisateur à New York, l'accès se coupe le 15 à 19 h. Le comportement n'est pas documenté.
- **Correctif.** Retenir le fuseau de la page (champ caché rempli par le navigateur) ou du compte, et calculer la fin du jour local.
- **Effort** S. **Confiance** vérifié.

### REL-30 — Invitation effacée au lieu d'être fusionnée

- **Lieu.** `people.rs:318-324` (`claim_invitations`) : le `UPDATE … NOT EXISTS` saute les Stations où le compte a déjà des droits, puis le `DELETE` efface l'invitation.
- **Scénario.** Léa a Regarder. On l'invite par e-mail avec Session avant qu'elle ne vérifie sa nouvelle adresse. À la vérification, Session est perdu sans message.
- **Correctif.** Passer par `add_person_rights`, une fois REL-03 corrigé.
- **Effort** S. **Confiance** vérifié.

### REL-31 — `Agent::reserve` garde sa demande après le délai

- **Lieu.** `tunnel/src/station.rs:383-393` : `pending.insert(req, tx)`, puis `timeout(5 s)`. En cas de délai dépassé, l'entrée n'est jamais retirée, alors que `attach_code` et `enroll` la retirent.
- **Effet.** Petite fuite côté Station, à chaque réservation de code restée sans réponse.
- **Correctif.** `pending.remove(&req)` sur `Err(_)`.
- **Effort** S. **Confiance** vérifié.

### REL-32 — Nom d'inscription non nettoyé

- **Lieu.** `web.rs:716` : `trim().chars().take(80)`, sans `clean_name`. Les caractères de contrôle et bidi passent, puis se retrouvent dans « Bienvenue, {nom} », dans les e-mails d'invitation et dans les listes de membres.
- **Correctif.** Le nettoyeur commun (REL-38).
- **Effort** S. **Confiance** vérifié.

---

## Optimisations

### REL-33 — Verrou de la base tenu pendant la construction des pages

- **Lieu.**
  - `web/org_pages.rs:59-126` garde `acc.db()` pendant toute la vue et appelle `grants_of` pour chaque Station (un jeton d'enrôlement en permet jusqu'à 10 000).
  - `members()` et `groups()` font une requête par membre ou par groupe.
  - `shell()` (`web/console.rs:30`) refait `visible_stations`, puis `standing` (3 à 4 requêtes) et `grants_of` pour chaque Station, à chaque page de la console.
- **Effet.** Une grosse équipe ralentit chaque page, et toute la base attend pendant ce temps, y compris `/v1/access` (`devices.rs:573`).
- **Correctif.** Requêtes groupées (`… WHERE station IN …`), `shell` calculé une fois par requête, et le verrou relâché avant de produire le HTML.
- **Effort** M. **Confiance** vérifié.

### REL-34 — Verrous globaux à chaque datagramme

- **Lieu.** `hub.rs:907` et `:922` : `is_current_station` et `is_current_client` verrouillent la table globale et hachent l'identifiant pour chaque paquet. Avec `global_cap_mbps > 0`, `capped()` (`hub.rs:934`) prend en plus un `Mutex<Bucket>` global par paquet.
- **Effet.** À plusieurs centaines de milliers de paquets par seconde, ces verrous sérialisent le relais.
- **Correctif.** Un `AtomicBool` « remplacé » posé sur chaque pair par le code qui le retire ; un seau atomique, ou un budget par fil, pour le plafond global.
- **Effort** S. **Confiance** vérifié.

### REL-35 — Allocation et copie par paquet

- **Lieu.**
  - `tunnel/src/transport.rs:490` et `frame.rs:93` : chaque datagramme est réencodé dans un nouveau tampon.
  - En repli TCP, un tampon par enregistrement (`frame.rs:122`) et une copie de plus dans `encode_record` (`transport.rs:522`).
  - En repli QUIC, une tâche par paquet (`transport.rs:506`, `:565`).
- **Correctif.** Réécrire le préfixe sur place quand la longueur du numéro de flux ne change pas ; un tampon de lecture réutilisé, découpé avec `split_to`.
- **Effort** M. **Confiance** vérifié.

### REL-36 — `AttachRequest` sur l'exécuteur

- **Lieu.** `hub.rs:498` appelle `attach_request` directement : parcours de toute la table des codes en attente, parfois avec un tri (`accounts/stations.rs:221-229`). `AttachStatus` et `Enroll`, eux, passent par `spawn_blocking`. Une Station peut déclencher cet appel sans limite.
- **Correctif.** `spawn_blocking` et un seau par Station.
- **Effort** S. **Confiance** vérifié.

---

## Factorisation et code mort

### REL-37 — Gabarit HTML copié trois fois

- **Lieu.** Le gabarit complet existe trois fois : `pages.rs:355`, `console_pages.rs:229` et la visite d'accueil (`console_pages.rs` vers `:2290`). La détection de `passkey.js` existe quatre fois (`console_pages.rs:235`, `:2300`, `pages.rs:362`, `:1531`). Le `replacen` du `meta refresh` est en double (`pages.rs:1112`, `console_pages.rs:2425`).
- **Pourquoi c'est important.** REL-01 vient de là : chaque copie a sa propre convention d'échappement.
- **Correctif.** Un seul `page_shell(title: &str, …)` qui échappe lui-même.
- **Effort** M. **Confiance** vérifié.

### REL-38 — Nettoyeurs de noms en double

- **Lieu.** `devices.rs:236` (`clean_device_name`) est identique à `stations.rs:351` (`clean_name`). `passkeys.rs:534` en a une troisième variante, et l'inscription n'en utilise aucune (REL-32). Sont aussi recopiés : « {équipe} (toute la famille / l'équipe) » (`org_pages.rs:94-95`, `console.rs:373-374`) et le nom de l'auteur d'une action (`org_pages.rs:255`, `console.rs:500`, alors que `link_pages.rs:112` a déjà `display_name`).
- **Correctif.** Un module `names` commun.
- **Effort** S. **Confiance** vérifié.

### REL-39 — Code de production réservé aux essais

- **Lieu.** Ces fonctions ne servent qu'aux essais : `certs.rs:70` (`is_ready`), `people.rs:144` (`invite_expires_at`), `store.rs:299` (`Store::memory`), `store.rs:768` (`audit_of`), `password.rs:85` (`Hasher::fast`) et `mail.rs:50-57` (`Outbox::last_to`, `count_to`). La recherche a porté sur tout le dépôt, `third_party` et `target` exclus.
- **Correctif.** Les placer sous `#[cfg(test)]`, ou derrière une fonctionnalité `test-util` si les essais d'intégration en ont besoin.
- **Effort** S. **Confiance** vérifié.

Aucune route, table ou colonne morte : toutes les routes `/v1/*` et `/compte/*` sont appelées par les apps ou générées par le relais, et chaque table est lue et écrite. Aucun TODO, FIXME ou XXX (les « XXXX-XXXX » sont le format des codes de téléviseur).

### REL-40 — Deux limites d'e-mails

- **Lieu.** `Limiter.mail_to` (`mod.rs:222`) et un `LAST` statique dans `stations.rs:393`, pour l'avis « Admin ouvert à distance ».
- **Correctif.** Ne garder que `Limiter`, avec une clé dédiée.
- **Effort** S. **Confiance** vérifié.

---

## Documentation

### REL-41 — E-mail « Admin ouvert »

- **Lieu.** `docs/comptes.md:170` et `:199`, ainsi que la version anglaise, annoncent « …le {date}, depuis {lieu} ». Le code (`mail.rs:356`) dit « …par KyberNow Share », sans lieu. Le pied « parce que vous êtes propriétaire » (`mail.rs:369`) part aussi aux gestionnaires d'équipe (`stations.rs:385`).
- **Correctif.** Aligner la doc, ou ajouter le lieu, et adapter la raison pour les gestionnaires.
- **Effort** S. **Confiance** vérifié.

### REL-42 — README et commentaires périmés

- `README.md`, section « Depuis les sources » : rien n'avertit que l'`updater` remplace l'image construite (REL-28).
- Le README, section « Comptes », ne liste en POST que la connexion et la suppression, alors que toute la console (équipes, liens, groupes) l'est aussi.
- `Cargo.toml:11` : la version `0.8.0` est celle que renvoie `/v1/health`, alors que l'image publiée suit KyberNow (0.11.7).
- `mail.rs:62` parle d'« un fil qui garde la connexion », alors que la fonctionnalité `pool` de lettre n'est pas activée : chaque e-mail ouvre sa connexion SMTP. Sans gravité aux volumes actuels.
- **Effort** S. **Confiance** vérifié.

Les routes, les limites et les variables `KN_RELAY_*` des docs (`relais.md`, `comptes.md`, `invitations.md`, `kybernow-share.md`, `pret.md`) correspondent au code, et chaque document a sa version `.en.md`.

## Essais

Ce qui existe :
- `tests/relais.rs` : 23 essais d'intégration du tunnel (QUIC et repli TCP) ;
- `src/accounts/web/tests*.rs` : environ 46 parcours de pages ;
- environ 100 essais unitaires.

Ce qui manque, par ordre d'utilité :
1. Un essai de noms hostiles (`<"'&`) sur toutes les pages de la console (REL-01).
2. Des essais de délégation : échéance et droits de qui donne, puis retrait (REL-02 à REL-04).
3. Des limites d'e-mails par expéditeur (REL-13).
4. `org_pages.rs`, `move_pages.rs` et `data.rs`, couverts seulement indirectement.
5. Le fil SMTP et `update.sh` / `lego.sh`.

## Les 10 priorités

1. **REL-01** : échapper dans `frame` et `page_with`, avec un essai de noms hostiles. Une heure de travail, et c'est la seule injection trouvée.
2. **REL-02, REL-03, REL-04** : borner la délégation, ne plus fusionner les échéances, retirer les droits au départ d'une équipe. C'est le cœur du modèle de droits.
3. **REL-06 et REL-07** : demander le mot de passe pour ajouter une passkey ou activer la double authentification ; nettoyer après une réinitialisation ; plus de double authentification ouverte.
4. **REL-05** : cookie d'attente lié au compte créé par ce navigateur.
5. **REL-13** : limite d'invitations par expéditeur, avant que quelqu'un ne s'en serve pour du spam.
6. **REL-14, REL-16, REL-17** : bornes mémoire de QUIC et du repli TCP, délai sur les corps HTTP, sémaphore réservé à Argon2.
7. **REL-15 et REL-19** : délai d'inactivité sur les flux relayés, places comptées après acceptation, seau par connexion de contrôle.
8. **REL-08** : connexion qui ne se laisse plus bloquer depuis l'extérieur.
9. **REL-20 et REL-28** : images signées ou épinglées ; ne plus écraser une construction locale.
10. **REL-10 et REL-09** : origines d'Admin publiées par la Station ; PKCE sur le retour vers l'app.

## Ce qui est sain

- **Cookies et CSRF.**
  - Cookies `__Host-` (`Secure`, `HttpOnly`, `SameSite=Lax`) : une Station sur `<id>.share.kybernow.app` ne peut pas les poser ni les écraser.
  - Jeton CSRF signé lié au cookie, et `Sec-Fetch-Site` qui refuse `same-site` (donc les pages des Stations).
  - Aucun en-tête CORS, et l'API ne marche qu'avec un jeton porteur.
- **Jetons.**
  - 256 bits tirés de `SystemRandom`, gardés en empreinte, comparés en temps constant (`ct_eq`).
  - Nouveau jeton à chaque étape (pas de fixation de session), expiration côté serveur.
  - Une réinitialisation ferme sessions, appareils et navigateurs de confiance (sauf REL-06).
- **Mots de passe.** Argon2id aux réglages OWASP, empreinte factice quand le compte n'existe pas.
- **TOTP.** Anti-rejeu (`last_step` atomique), secret en AES-GCM lié au compte.
- **WebAuthn.**
  - Vérifie le type, l'origine exacte, `crossOrigin`, le défi signé à usage unique lié au navigateur, `rpIdHash`, les drapeaux UP et UV et le compteur.
  - Analyseur CBOR borné (profondeur 16, longueurs bornées par l'entrée).
- **Droits.**
  - Aucune IDOR : chaque route d'équipe passe par `org_of` et `managed!` avec un filtre SQL par équipe ; chaque route de Station passe par `managed_station` et `can_manage`.
  - Liens d'accès à usage unique, dans une transaction atomique, en 160 bits gardés en empreinte.
  - Jetons d'accès de 120 s, signés avec exactement les droits accordés.
- **Tunnel.**
  - Une Station ne peut pas être usurpée : son identifiant est le hachage de sa clé, et TLS 1.3 prouve la possession de la clé privée.
  - `admin_fp` est signé et vérifié de bout en bout par les apps.
  - Aucun pivot SSRF : l'app ne donne qu'un `host_id` et un port, et Admin applique une liste fermée de ports.
- **Trames et délais.**
  - Trames bornées à 64 Kio.
  - Délais de 10 s sur le ClientHello, la poignée de main, le `hello` et les en-têtes de flux ; 30 s de silence en QUIC, 35 s en TCP.
  - Files bornées et aucun verrou tenu à travers un `.await`.
- **Pages.** CSP stricte (`script-src 'self'`, `frame-ancestors 'none'`), `nosniff`, `Referrer-Policy: no-referrer`, `esc` appliqué partout ailleurs.
- **Redirections.** Retour d'app fixé à `kybernow://compte`, `next_ok` contre les redirections ouvertes.
- **SQL.** Uniquement par paramètres. Migrations ordonnées, une transaction chacune, refus d'une base plus récente, `foreign_keys=ON`.
- **E-mails.** Envoyés dans leur propre fil ; en-têtes protégés par lettre ; corps HTML échappés ; aucun secret ni aucune adresse dans les journaux.
- **Déploiement.**
  - Image distroless non-root, conteneur `read_only`, `cap_drop: ALL`.
  - Métriques sur 127.0.0.1, sans identifiant ni adresse dans les étiquettes.
  - `lego.sh` en `set -eu`, avec une clé lisible par le seul relais.
  - Aucun secret dans l'historique git.
