# Textes juridiques du service KyberNow (brouillons)

[English](README.en.md) · Français

Brouillons des textes juridiques du service de comptes KyberNow (relais public `share.kybernow.app`), rédigés à partir du cadrage ([comptes.md](../comptes.md)), de ce que fait le relais ([relais.md](../relais.md)) et des mentions légales actuelles du site. **Rien n'est en vigueur ni publié** : tout est à faire relire par un juriste avant la mise en ligne des comptes (lot 6 du cadrage). Version 0.1 du 23/11/2026.

## Les documents

| Document | Pour qui | Publié ? |
|---|---|---|
| [conditions.md](conditions.md) · [EN](conditions.en.md) | Conditions d'utilisation : comptes, relais public, organisations, usage acceptable, accès à distance, suspension, suppression, gratuité et offre payante, droit applicable | Oui, sur le relais (lien depuis « Créer un compte ») |
| [confidentialite.md](confidentialite.md) · [EN](confidentialite.en.md) | Politique de confidentialité RGPD du service | Oui, sur le relais ; elle remplace la page `/privacy` actuelle |
| [registre.md](registre.md) | Registre des traitements (article 30), en français | Non : document interne, montré à la CNIL sur demande |
| [dpa.md](dpa.md) · [EN](dpa.en.md) | Contrat de sous-traitance (article 28) pour les organisations clientes | Oui, accepté par un Propriétaire à la création d'une organisation |

Dans les textes, `[À COMPLÉTER]` attend une information, `[À DÉCIDER]` une décision, `[À VÉRIFIER]` une confirmation ; les notes « Pour le juriste » posent les questions juridiques et disparaissent des versions publiées.

## À faire relire en priorité

1. **Partage des rôles RGPD** : Les Lignes Claires responsables des comptes, sous-traitant des organisations pour leurs membres, droits et journal ; recoupement du journal de sécurité du compte et du journal de l'organisation.
2. **Responsabilité** : clauses limitatives face à un consommateur pour un service gratuit ; régime des contenus et services numériques (Code de la consommation, articles L. 224-25-1 et suivants) ; rédaction séparée pour les professionnels.
3. **Accès à distance** : la répartition des responsabilités (celui qui ouvre son ordinateur, celui qui y entre, l'organisation employeur) et le rappel des articles 323-1 et suivants du Code pénal.
4. **Durées** : adresse IP gardée 12 mois dans le journal contre quelques jours dans les journaux techniques ; application éventuelle du décret n° 2021-1362 (conservation des données d'identification), qui pourrait contredire l'effacement sous 30 jours.
5. **Analyse d'impact** (AIPD) avant le lancement des organisations, et désignation ou non d'un délégué à la protection des données.
6. **Délai de notification** au Responsable dans le contrat de sous-traitance (48 heures proposées).
7. **Âge minimum** et compte d'un mineur.
8. **Médiateur de la consommation** : obligatoire dès le service gratuit, ou à l'offre payante.

## À compléter ou à décider

**Identité** (reprise des mentions légales du site ; à confirmer au lancement, et partout à changer si une société remplace l'entreprise individuelle) :

- éditeur et responsable du traitement : Matthieu Requenna, entrepreneur individuel, exerçant sous le nom Les Lignes Claires ;
- SIREN 510 178 577, adresse à Montpellier ;
- adresse de contact dédiée au service et aux données (aujourd'hui contact@leslignesclaires.fr) ; adresse de signalement des failles (`security.txt`).

**Prestataires** :

- **prestataire d'e-mails** de l'UE : nom, adresse, pays des données, contrat de sous-traitance signé, durée de ses journaux d'envoi, absence de transfert hors UE chez lui ;
- **hébergement du relais** : aujourd'hui sur le matériel de l'éditeur, en France (page `/privacy` : « chez l'auteur de KyberNow ») ; à confirmer, avec le fournisseur d'accès, et à revoir si le relais passe chez un hébergeur ;
- **sauvegardes** : lieu, fréquence, chiffrement, durée de garde ; prestataire s'il est externe.

**Durées** :

- journaux techniques du relais : durée maximale en jours (aujourd'hui « quelques jours », tournés par taille) ;
- sessions d'appareil inactives ; cookie de session de la console ;
- comptes jamais vérifiés (7 jours proposés), comptes inactifs (3 ans proposés, après avertissement) ;
- personnes ajoutées par e-mail sans créer de compte (30 jours proposés) ;
- lignes du journal d'une organisation après la suppression du compte d'un membre ;
- échanges avec le support, demandes de droits, registre des violations.

**Conditions** :

- âge minimum (15 ans proposé) ;
- préavis avant l'offre payante (30 jours proposé), avant une fermeture de compte par nous (15 jours), avant l'arrêt du service (3 mois) ;
- plafond de responsabilité ; engagement de disponibilité pour l'offre payante ;
- reprise d'une organisation sans Propriétaire joignable ;
- tribunal compétent pour les professionnels (Montpellier proposé) ;
- médiateur de la consommation ;
- contrat de sous-traitance accepté en ligne, ou signé ; audit sur place quand le relais est chez l'éditeur.

## Ce que ces textes demandent au produit

À reporter dans le lot 1 et les suivants (rien n'est changé dans le code ici) :

- garder **la version et la date des conditions acceptées** par chaque compte, et par chaque organisation pour le contrat de sous-traitance (pas de champ prévu dans la base du lot 1) ;
- case « conditions et confidentialité » de la page d'inscription pointant vers ces textes ; versions précédentes consultables ;
- **export des données** du compte et, pour une organisation, de ses membres, droits et journal ;
- effacement **sous 30 jours, sauvegardes comprises** : prévoir la rotation des sauvegardes en conséquence ;
- effacement automatique des comptes non vérifiés, des personnes ajoutées sans compte, et (si décidé) des comptes inactifs avec e-mails d'avertissement ;
- e-mail aux Propriétaires d'une organisation pour un nouveau sous-traitant (30 jours avant) et pour une violation (48 heures) ;
- page `/privacy` du relais à remplacer par la politique de confidentialité ; mentions légales du site à mettre à jour (le relais tient des comptes ; la phrase « projet communautaire et non commercial » devra changer avant l'offre payante).
