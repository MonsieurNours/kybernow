# Registre des activités de traitement (article 30 du RGPD)

> **Brouillon à faire relire par un juriste.** Ce registre est un document interne, tenu à jour et montré à la CNIL si elle le demande ; il n'est pas publié. Il décrit le service de comptes tel que cadré dans [comptes.md](../comptes.md) et le relais tel que décrit dans [relais.md](../relais.md). Les passages entre crochets attendent une information ou une décision. Version 0.1 du 23/11/2026.

## Identité

| | |
|---|---|
| Responsable du traitement | Matthieu Requenna, entrepreneur individuel, exerçant sous le nom Les Lignes Claires `[À VÉRIFIER au lancement]` |
| SIREN | 510 178 577 |
| Adresse | Bâtiment 1, appartement 3, 230 avenue du Maréchal Leclerc, 34070 Montpellier, France |
| Contact | contact@leslignesclaires.fr `[À DÉCIDER : adresse dédiée aux données]` |
| Délégué à la protection des données | Aucun (non obligatoire) `[À CONFIRMER]` |
| Représentant dans l'UE | Sans objet (établi en France) |

## Partie 1 : traitements dont Les Lignes Claires sont responsables

### T1. Comptes et authentification

| Rubrique | Contenu |
|---|---|
| Date de création, mise à jour | `[À COMPLÉTER : date de mise en service]` ; 23/11/2026 (brouillon) |
| Finalités | Créer et tenir les comptes KyberNow ; connexion par mot de passe, double authentification, passkeys ; vérification de l'adresse ; mot de passe oublié ; sessions et appareils connectés ; export et suppression du compte |
| Base légale | Exécution du contrat (6.1.b) |
| Personnes concernées | Titulaires d'un compte sur le relais public |
| Données | E-mail, nom ; empreinte Argon2id du mot de passe ; secret TOTP chiffré ; empreintes des codes de secours ; passkeys (identifiant, clé publique, compteur, nom, dates) ; sessions (empreinte du jeton, appareil, dates, 2FA faite) ; appareils (nom, sorte, dernière venue) ; navigateurs de confiance (empreinte, échéance) ; jetons d'e-mail (empreinte, usage, code, échéance) ; offre du compte ; `[À AJOUTER : version et date des conditions acceptées]` |
| Données sensibles | Aucune |
| Destinataires | Les Lignes Claires ; prestataire d'e-mails (adresse et contenu des e-mails de service) |
| Sous-traitants | `[À COMPLÉTER : prestataire d'e-mails]` ; `[À COMPLÉTER : sauvegardes externes, s'il y en a]` |
| Transferts hors UE | Aucun |
| Durées | Vie du compte ; sessions : vie de l'appareil `[À PRÉCISER]` ; navigateurs de confiance : 30 jours ; jetons d'e-mail : 24 h ou 1 h ; compte supprimé : effacé sous 30 jours, sauvegardes comprises ; compte non vérifié : `[À DÉCIDER : 7 jours]` ; compte inactif : `[À DÉCIDER : 3 ans, après avertissement]` |
| Sécurité | Argon2id ; TOTP chiffré ; jetons en empreinte ; cookies `__Host-`, `Secure`, `HttpOnly` ; CSRF ; limites d'essais (10 par adresse, 5 par compte en 15 minutes) ; réponses identiques qu'un compte existe ou non ; e-mails et jetons jamais journalisés ; HTTPS ; double authentification obligatoire pour les rôles qui administrent |

### T2. Ordinateurs, droits et accès à distance

| Rubrique | Contenu |
|---|---|
| Finalités | Rattacher des ordinateurs (Stations) à un compte ; donner des droits (Session, Prêt, Partage, Administrer) à des personnes ; délivrer des jetons d'accès signés ; montrer les ordinateurs du compte dans les apps ; avertir le propriétaire d'un nouvel appareil |
| Base légale | Exécution du contrat (6.1.b) ; avis de nouvel appareil : intérêt légitime (6.1.f, sécurité des ordinateurs) |
| Personnes concernées | Titulaires d'un compte ; personnes ajoutées par e-mail qui n'ont pas encore de compte |
| Données | Stations (identifiant, nom, propriétaire) ; droits (compte ou groupe, ordinateur, droits, échéance) ; e-mail des personnes ajoutées sans compte ; jetons d'accès (identifiant, compte, appareil, Station, droits, dates) |
| Destinataires | Les Lignes Claires ; le propriétaire de l'ordinateur (nom, e-mail, droits et accès des personnes à qui il l'a ouvert) ; la Station elle-même (liste locale des comptes et appareils entrés) ; prestataire d'e-mails |
| Transferts hors UE | Aucun |
| Durées | Stations : jusqu'au détachement ; droits : jusqu'au retrait ou à l'échéance ; personnes ajoutées sans compte : `[À DÉCIDER : 30 jours]` ; jetons : 5 minutes, identifiant gardé `[À PRÉCISER]` pour empêcher le rejeu |
| Sécurité | Jetons Ed25519 de 5 minutes, clé épinglée par la Station, `jti` jamais rejoué ; interrupteur, liste locale et blocage dans Admin ; avis à chaque nouvel appareil ; tunnel chiffré de bout en bout entre les apps et la Station |

### T3. Sécurité du service et prévention des abus

| Rubrique | Contenu |
|---|---|
| Finalités | Détecter et bloquer les intrusions et les abus ; montrer à chaque titulaire l'activité de son compte ; enquêter sur un incident |
| Base légale | Intérêt légitime (6.1.f) : sécurité des comptes, des ordinateurs et du service |
| Personnes concernées | Titulaires d'un compte ; toute personne qui se connecte au relais (y compris les invités d'un partage, sans compte) |
| Données | Journal de sécurité du compte (type d'événement, date, adresse IP) ; journaux techniques du relais (adresses IP, heures de connexion, identifiants de Station et de client, débits ; jamais les codes de partage ni les e-mails) |
| Destinataires | Les Lignes Claires ; autorités sur réquisition |
| Transferts hors UE | Aucun |
| Durées | Journal de sécurité : 12 mois ; journaux techniques : quelques jours, tournés par taille `[À PRÉCISER : durée maximale]` |
| Sécurité | Accès aux journaux réservé à l'éditeur ; `[À COMPLÉTER : où sont les journaux, chiffrement du disque]` |
| Note | `[À TRANCHER : application du décret n° 2021-1362 (conservation des données d'identification et de connexion)]` |

### T4. Relais : partage et accès hors de chez soi

| Rubrique | Contenu |
|---|---|
| Finalités | Faire passer la connexion entre les apps et les Stations hors du réseau local ; mettre en relation un invité et l'hôte d'un partage par un code |
| Base légale | Exécution du contrat avec le titulaire de la Station `[À VÉRIFIER : sans compte, le service est utilisé sans contrat signé ; intérêt légitime ?]` |
| Personnes concernées | Hôtes et invités d'un partage ; appareils appairés hors de chez eux |
| Données | Adresses IP publiques, identifiants de Station et de client, débits ; codes de partage en mémoire, le temps du partage (jamais journalisés) ; trafic des navigateurs déchiffré en transit, jamais gardé ; trafic des apps chiffré de bout en bout, illisible par le relais |
| Destinataires | Les Lignes Claires (aucune donnée gardée hors journaux techniques, voir T3) ; la Station reçoit l'adresse IP publique du client |
| Transferts hors UE | Aucun |
| Durées | En mémoire, le temps de la connexion ; codes d'une Station partie gardés 120 s |
| Sécurité | TLS 1.3 et QUIC ; tunnel de bout en bout avec Admin épinglé ; limites par adresse ; pages sans script, cookie de suivi ni ressource tierce |

### T5. Organisations (pour le compte des Lignes Claires)

| Rubrique | Contenu |
|---|---|
| Finalités | Tenir l'organisation comme client : sa création, son nom, ses Propriétaires, son offre, la relation avec elle |
| Base légale | Exécution du contrat (6.1.b) |
| Personnes concernées | Propriétaires et Administrateurs des organisations |
| Données | Nom de l'organisation, comptes Propriétaires, offre, dates ; `[plus tard : données de facturation]` |
| Destinataires | Les Lignes Claires ; `[plus tard : prestataire de paiement, expert-comptable]` |
| Durées | Vie de l'organisation ; `[plus tard : pièces comptables 10 ans]` |
| Note | Les membres, groupes, droits et journal d'une organisation sont traités pour elle : voir partie 2 |

### T6. E-mails de service

| Rubrique | Contenu |
|---|---|
| Finalités | Envoyer les e-mails nécessaires au service : vérification, mot de passe oublié, nouvel appareil, ajout à un ordinateur ou à une organisation, double authentification changée, avis importants (conditions, offre payante, suspension) |
| Base légale | Exécution du contrat (6.1.b) ; avis de sécurité : intérêt légitime (6.1.f) |
| Personnes concernées | Titulaires d'un compte ; personnes ajoutées par e-mail |
| Données | Adresse e-mail, nom, contenu du message (nom de l'ordinateur, de l'appareil, de la personne qui ajoute) |
| Sous-traitant | `[À COMPLÉTER : prestataire d'e-mails, pays, contrat article 28 signé le …]` |
| Transferts hors UE | Aucun `[À VÉRIFIER chez le prestataire]` |
| Durées | Chez le prestataire : `[À COMPLÉTER : durée de ses journaux d'envoi]` |
| Note | Aucun e-mail de prospection. Si une lettre d'information est créée un jour, elle demandera un consentement et une entrée à part |

### T7. Demandes et exercice des droits

| Rubrique | Contenu |
|---|---|
| Finalités | Répondre aux questions, au support et aux demandes d'exercice des droits ; prouver la réponse |
| Base légale | Obligation légale (6.1.c) pour les droits (articles 12 à 22) ; intérêt légitime pour le support |
| Personnes concernées | Toute personne qui nous écrit |
| Données | Adresse e-mail, message, réponse, date, demande et suite donnée |
| Destinataires | Les Lignes Claires ; l'organisation concernée quand la demande porte sur ses données |
| Durées | `[À DÉCIDER : 3 ans après le dernier échange ; demandes de droits : 5 ans pour la preuve]` |

### T8. Violations de données

| Rubrique | Contenu |
|---|---|
| Finalités | Tenir le registre des violations (article 33.5) ; notifier la CNIL, les personnes et les organisations |
| Base légale | Obligation légale (6.1.c) |
| Données | Description de la violation, personnes et données touchées, mesures prises, notifications faites |
| Durées | `[À DÉCIDER : 5 ans]` |

## Partie 2 : traitements faits pour le compte des organisations (article 30.2)

| Rubrique | Contenu |
|---|---|
| Sous-traitant | Les Lignes Claires (identité ci-dessus) |
| Responsables du traitement | Chaque organisation cliente `[liste tenue dans la base : nom, Propriétaires, date du contrat de sous-traitance accepté]` |
| Catégories de traitements | Gestion des membres, rôles et groupes de l'organisation ; droits sur les ordinateurs de l'organisation ; délivrance des jetons d'accès ; journal des accès et des changements de droits ; avis de nouvel appareil aux Administrateurs ; jetons d'enrôlement de flotte ; e-mails liés |
| Données | Voir l'annexe 1 du [contrat de sous-traitance](dpa.md) |
| Sous-traitants ultérieurs | `[À COMPLÉTER : prestataire d'e-mails ; sauvegardes externes]` |
| Transferts hors UE | Aucun |
| Sécurité | Voir l'annexe 2 du [contrat de sous-traitance](dpa.md) |

## Analyse d'impact (AIPD)

`[À TRANCHER avec le juriste]` Le service permet de prendre la main sur des ordinateurs à distance et tient, pour les organisations, un journal de qui a ouvert quel ordinateur, quand et d'où. Deux critères de la liste du CEPD peuvent être réunis (surveillance de personnes, possiblement de salariés ; usage innovant d'une technologie). Une AIPD semble prudente avant le lancement des organisations (lot 5), au moins pour le traitement T2 et la partie 2.

## Suivi

| Date | Changement | Par |
|---|---|---|
| 23/11/2026 | Brouillon initial | Les Lignes Claires |
