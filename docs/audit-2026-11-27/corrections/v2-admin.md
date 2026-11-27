# Vague 2 — Admin à distance et Admin des Stations Mac

Branche `claude/v2-admin-516a6b`, d'après [vague-2.md](../vague-2.md) (phase 2a). Zone : `server/admin`, `server/relay` (avec `tunnel/`), `server/web` (interface d'Admin), `docs/relais*`, `docs/comptes*`, README du relais et du serveur. Rien poussé, rien fusionné.

## Fait

| Id | Correctif |
|---|---|
| STWIN-17 | **Admin à distance chiffré de bout en bout jusqu'à la Station.** Conception dans [docs/relais.md](../../relais.md) (FR et EN), « Navigateur chiffré de bout en bout (0.11.8) ». Le navigateur ouvre la Station par `https://<id>.station.share.kybernow.app` : le relais lit le `ClientHello` sans répondre et fait passer le TLS tel quel (flux `browser_tls`) ; la Station tient sa clé et son certificat, obtenu et renouvelé par Admin auprès de Let's Encrypt (ACME, défi TLS-ALPN-01 relayé comme un navigateur). |
| STWIN-17 (nom) | Zone à part, `station.<domaine>` : le certificat générique `*.share.kybernow.app` du relais, gardé pour les Stations 0.11.7, ne la couvre pas (une étiquette seulement). Le relais n'a donc aucune clé valable pour le nom de bout en bout. |
| STWIN-17 (pas de retour) | Une Station qui a été de bout en bout avec son relais (`[relay] e2e`) refuse ensuite « Admin à distance » en clair (`403 remote_admin_e2e`), ferme les sessions ouvertes en clair, et se suspend (`lost`) si le relais cesse d'annoncer la zone. Une session ne vaut que sur le chemin où elle s'est ouverte. Rallumer « Admin à distance » sur place lève la marque. |
| STWIN-17 (journaux publics) | Une fois par jour, `crt.sh` : un certificat pour le nom de la Station qu'elle n'a pas obtenu suspend « Admin à distance » (`foreign`). |
| STWIN-17 (interface) | Sous l'interrupteur d'Admin : chiffré de bout en bout, en préparation, pas prêt, relais qui ne le permet pas, suspendu (deux cas). La phrase « Le relais voit ce qui passe » ne s'affiche plus que si c'est vrai. Page `/privacy` du relais à jour (chiffrement du navigateur, journaux publics). |
| REL-10 (reste) | Vers l'origine de bout en bout (Station ≥ 0.11.8), le jeton d'« Ouvrir Admin » part dans le fragment (`#token=`), jamais envoyé au serveur ; `?token=` reste pour l'ancienne origine (Admin 0.11.7). |
| STMAC-06 | `launchctl` vise `gui/<uid>` du compte qui fait tourner Admin (`mac::launchd_domain`), plus le propriétaire de `/dev/console` ; une seule fonction pour `mac.rs`, `update/mac.rs` et `station.rs` (la partie launchd de STCOM-08). |
| STMAC-07 | `launchctl` qui ne se lance pas est une erreur (`mac::launchctl` rend un `Result`) : l'applicateur ne remplace pas l'app si l'arrêt n'est pas sûr, relance la Station sur échec, note chaque échec au journal ; le redémarrage depuis Admin aussi. |
| STMAC-08 | Agent des écrans (Mac **et** Linux, même code) : `display/agent_pipe.rs`, lecture par un fil et un canal, réponse attendue 30 s (5 s pour la veille), agent muet arrêté puis relancé ; fin attendue 5 s au plus puis arrêt ; un écran n'est oublié qu'une fois retiré (ou parti avec l'agent). |
| hors liste | Essais d'Admin compilables sous Windows (`invites_api.rs`, `discovery.rs`, aides de `accounts.rs` gardées `not(windows)` comme les autres) ; `preferred_among` gardé Mac : `cargo check` Mac, Linux et Windows sans avertissement. |

Compatibilité : Station 0.11.7 face au relais 0.11.8, et Station 0.11.8 face au relais 0.11.7, marchent comme aujourd'hui (champs `browser_tls`, `tls_ready` et message `tls_zone` ignorés par qui ne les connaît pas ; le relais n'envoie un flux `browser_tls` qu'à une Station qui l'a annoncé). Tableau dans docs/relais.md.

## Laissé, et pourquoi

- **Jetons signés par le relais** (l'autre moitié de STWIN-17) : le relais signe toujours le jeton `via: "relay"` qui ouvre une session à distance. Il ne lit plus Admin, mais un relais compromis pourrait forger un jeton et ouvrir lui-même Admin par le chemin chiffré. Le fermer demande une preuve que la Station vérifie elle-même (passkey enregistrée sur place auprès de la Station, sur `<id>.station…`) : une étape de plus pour l'hôte, **à décider par l'auteur**. Écrit dans docs/relais.md, « Plus tard » et « Ce que le relais voit encore ».
- **Déploiement sur la machine du relais** : à faire par l'auteur, pas à pas dans docs/relais.md, « Déploiement sur le relais public » (DNS `*.station.share`, CAA, image 0.11.8, vérifications, retour en arrière par `KN_RELAY_TLS_ZONE=off`).
- **Limites de Let's Encrypt** (50 nouveaux certificats par semaine pour `kybernow.app`) : les Stations attendent et réessaient, le navigateur passe par l'ancien nom en attendant. Quand le nombre de Stations s'en approche : demande de hausse à Let's Encrypt, ou `station.share.kybernow.app` à la Public Suffix List (jamais `share.kybernow.app`, qui porte les passkeys). Décision d'exploitation.
- **Page « injoignable » sur le nom de bout en bout** : une Station absente ferme la connexion (le relais n'a pas de certificat pour ce nom) ; la console et `/j/` disent déjà « hors ligne » avant d'y mener.
- **STCOM-06 et STCOM-08 entiers** (factorisation de `display/mac.rs` et `display/linux.rs`, étiquettes et `home()` en plusieurs copies) : seule la partie qui portait STMAC-06 à 08 est faite ; le reste relève de la session « Stations » de la phase 2b.
- **Cœur des apps** : pas compilé (il demande les arbres de Kyber). Les changements du crate du tunnel sont des ajouts ; aucun `match` exhaustif ni littéral de `apps/core` ne touche les types changés (relu).

## Vérifié, et comment

- **Relais** (`cargo test --workspace --all-features`, `cargo clippy` sans avertissement) : 172 + 25 + 34 + 7 + 2 essais. Nouveaux : `tcp::lecture_du_client_hello` (vrai `ClientHello` rustls, octets rejoués à l'identique), `rejoue_avant_le_reste`, `config::zone_de_bout_en_bout` (443 seulement, `off`, jamais le domaine), `web::aiguillage_par_le_nom`, `navigateur_de_bout_en_bout` (relais réel sur la boucle locale, fausse Station avec sa propre autorité : la négociation n'aboutit qu'avec l'autorité de la Station, donc le relais n'a rien déchiffré ; défi `acme-tls/1` passé à la Station ; liens `/j/` vers le nouveau nom une fois `tls_ready`), `bout_en_bout_seulement_si_annonce` (Station 0.11.7, port public ≠ 443), et « Ouvrir Admin » de bout en bout dans `lot45` (retour accepté, jeton dans le fragment, autre Station refusée).
- **Admin** (`cargo test`, 405 essais ; `cargo clippy` sans nouvel avertissement) : `acme::empreinte_jwk_rfc7638`, `acme::certificat_du_defi` (extension acmeIdentifier critique), `relay_tls::certificat_obtenu_par_acme` (**tout le parcours ACME contre une fausse autorité** qui vérifie chaque signature JWS, valide le défi par une vraie négociation TLS-ALPN-01 contre le serveur `browser_tls` d'Admin et signe la clé de la CSR ; certificat gardé, relu, servi pour son nom seul), `journaux_publics`, `lecture_d_un_certificat`, `veille_du_nom`, `relay::admin_a_distance_de_bout_en_bout` (chemin de chaque session, marque, suspension, rallumage, changement de relais), `relay::de_bout_en_bout_avec_un_relais_simule` (relais simulé qui annonce la zone : `tls_ready` dit au relais, marque posée ; zone plus annoncée : `lost`), `display::agent_pipe::reponse_delai_et_relance` (faux agent : réponse, refus, agent muet arrêté dans le délai, relance, fin bornée).
- `cargo check --all-targets` d'Admin pour `aarch64-apple-darwin`, `aarch64-unknown-linux-gnu` (C de ring avec le sysroot du NDK) et `x86_64-pc-windows-gnu` : sans erreur ni avertissement.
- Interface d'Admin (`server/web`) : `npm run check` (svelte-check, catalogues fr/en alignés) et `vitest` (49 essais).

## À essayer sur appareil

- Une Station Windows et une Station Mac 0.11.8 face au relais public mis à jour (après le DNS) : certificat obtenu (`GET /api/relay` → `remote_tls.state = "ready"`), « Ouvrir Admin » depuis la console mène à `<id>.station.share.kybernow.app` et le navigateur montre le certificat de la Station ; un invité par `/j/<code>` aussi. Avant la production : `KYBERNOW_ACME_DIRECTORY=https://acme-staging-v02.api.letsencrypt.org/directory`.
- Station 0.11.8 face au relais 0.11.7 (avant sa mise à jour) : « Admin à distance » comme aujourd'hui, état « ce relais ne permet pas… ».
- Mac : changement rapide d'utilisateur pendant une session puis « Redémarrer » et une mise à jour (STMAC-06) ; agent des écrans bloqué (sortie de veille) sans que la page Écrans d'Admin se fige (STMAC-08).

## Notes de version

- FR : « Admin à distance » est maintenant chiffré de bout en bout jusqu'à votre ordinateur : le relais KyberNow Share ne voit plus ce qui passe. Votre ordinateur obtient son propre certificat, et surveille qu'aucun autre n'est émis à son nom. Station Mac : redémarrage et mises à jour visent toujours la bonne session, et un écran virtuel bloqué ne fige plus Admin.
- EN: “Remote Admin” is now encrypted end to end all the way to your computer: the KyberNow Share relay no longer sees what goes through. Your computer gets its own certificate, and watches that no other one is issued in its name. Mac Station: restarts and updates always target the right session, and a stuck virtual display no longer freezes Admin.
