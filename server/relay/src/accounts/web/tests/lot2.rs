//! Les parcours du lot 2 (rattacher une Station, la détacher) et de la moitié
//! relais du lot 3 (connecter un appareil par la fenêtre du système ou par un
//! code, ordinateurs du compte, jetons d'accès, avis de nouvel appareil), sans
//! réseau : une fausse liaison vers les Stations remplace le hub.

use std::collections::HashSet;
use std::sync::{Mutex, Weak};

use kybernow_tunnel::access::{Use, Verifier};
use kybernow_tunnel::msg::Msg;
use serde_json::{Value, json};

use super::*;
use crate::accounts::devices::{ApiReply, ApiRequest};
use crate::accounts::stations::{Attest, StationLink};

/// Les Stations « connectées » et ce qu'on leur a envoyé.
#[derive(Default)]
struct FakeLink {
    online: Mutex<HashSet<String>>,
    sent: Mutex<Vec<(String, Msg)>>,
}

impl StationLink for FakeLink {
    fn online(&self, station: &str) -> bool {
        self.online.lock().unwrap().contains(station)
    }

    fn send(&self, station: &str, msg: Msg) -> bool {
        if !self.online(station) {
            return false;
        }
        self.sent.lock().unwrap().push((station.to_string(), msg));
        true
    }

    fn attest(&self, station: &str) -> Option<Attest> {
        self.online(station).then(|| Attest { station_key: "Y2xl".into(), admin_fp: format!("sha256:{station}"), sig: "c2ln".into() })
    }
}

impl FakeLink {
    fn last(&self) -> Option<(String, Msg)> {
        self.sent.lock().unwrap().last().cloned()
    }
}

fn linked(w: &World) -> Arc<FakeLink> {
    let link = Arc::new(FakeLink::default());
    let weak: Weak<dyn StationLink> = Arc::downgrade(&link) as Weak<dyn StationLink>;
    w.acc.stations.set_link(weak);
    link
}

fn station(n: u8) -> String {
    kybernow_tunnel::id::station_id(&[n; 32])
}

fn api(w: &World, post: bool, path: &str, bearer: Option<&str>, body: Value) -> ApiReply {
    w.acc.api(&ApiRequest { post, path: path.into(), bearer: bearer.map(str::to_string), body, ip: "192.0.2.10".parse().unwrap() })
}

/// Une personne connectée dans son navigateur.
fn signed_in(w: &World) -> Browser {
    w.acc.add_verified("ana@exemple.org", "Ana", PASSWORD).unwrap();
    let mut b = w.browser();
    assert_eq!(location(&b.sign_in("ana@exemple.org", PASSWORD)), "/compte?lang=fr");
    b
}

/// Rattache `st` au compte d'Ana, depuis un navigateur connecté.
fn attach(w: &World, b: &mut Browser, st: &str) {
    let (code, _, _) = w.acc.attach_request(st, "PC du salon");
    let page = b.get(&format!("/compte/rattacher?code={code}&lang=fr"));
    assert!(html(&page).contains("Rattacher PC du salon"));
    let r = b.post("/compte/rattacher?lang=fr", &[("code", &code), ("action", "rattacher")]);
    assert!(html(&r).contains("Ordinateur rattaché"));
}

#[test]
fn rattacher_une_station_puis_la_detacher() {
    let w = world(true);
    let link = linked(&w);
    let st = station(1);
    link.online.lock().unwrap().insert(st.clone());
    w.acc.add_verified("ana@exemple.org", "Ana", PASSWORD).unwrap();
    // Admin demande un code par sa connexion de contrôle : la page à ouvrir.
    let (code, url, expires) = w.acc.attach_request(&st, "PC du\nsalon");
    assert_eq!(url, format!("https://share.test/compte/rattacher?code={code}"));
    assert_eq!(expires, 15 * 60);
    // Pas connectée : la page est retenue, on se connecte, on y revient.
    let mut b = w.browser();
    let r = b.get(&format!("/compte/rattacher?code={code}&lang=fr"));
    assert_eq!(location(&r), "/compte/connexion?lang=fr");
    assert!(b.jar.contains_key("__Host-kn_suite"));
    let r = b.sign_in("ana@exemple.org", PASSWORD);
    assert_eq!(location(&r), format!("/compte/rattacher?code={code}&lang=fr"));
    assert!(!b.jar.contains_key("__Host-kn_suite"), "la suite ne sert qu'une fois");
    let page = b.get(&format!("/compte/rattacher?code={code}&lang=fr"));
    let text = html(&page);
    assert!(text.contains("Rattacher PC du salon") && text.contains("ana@exemple.org") && text.contains(r#"name="proprietaire" value="moi" checked"#));
    // Annuler : rien n'est rattaché, le code reste valable.
    let r = b.post("/compte/rattacher?lang=fr", &[("code", &code), ("action", "annuler")]);
    assert!(html(&r).contains("Rattachement annulé"));
    assert!(w.acc.db().station(&st).unwrap().is_none());
    // Confirmer : la Station reçoit `attached` (relais, compte, clés publiées).
    let r = b.post("/compte/rattacher?lang=fr", &[("code", &code), ("action", "rattacher")]);
    assert!(html(&r).contains("Ordinateur rattaché") && html(&r).contains("Admin l'affiche déjà"));
    let (to, msg) = link.last().expect("attached envoyé");
    assert_eq!(to, st);
    let Msg::Attached(a) = msg else { panic!("attached attendu : {msg:?}") };
    assert_eq!(a.relay, "share.test");
    assert_eq!(a.account, a.owner.id);
    assert!(a.account.starts_with("acc_") && a.account.len() == 20);
    assert_eq!((a.owner.kind.as_str(), a.owner.name.as_str(), a.owner.email.as_deref()), ("person", "Ana", Some("ana@exemple.org")));
    assert_eq!(a.keys, w.acc.stations.keys.key_set().keys);
    let row = w.acc.db().station(&st).unwrap().unwrap();
    assert_eq!(row.name, "PC du salon");
    assert!(w.acc.db().audit_of(row.account).unwrap().iter().any(|(k, _)| k == "station_rattachee"));
    // Le code a servi.
    assert_eq!(b.post("/compte/rattacher?lang=fr", &[("code", &code), ("action", "rattacher")]).status, 400);
    assert_eq!(b.get(&format!("/compte/rattacher?code={code}&lang=fr")).status, 400);
    // À la reconnexion, la Station demande où elle en est : `attached`, avec la demande.
    assert!(matches!(w.acc.attach_status(&st, 9), Msg::Attached(ref a) if a.req == Some(9)));
    // La page du compte la montre ; « Détacher » le dit à la Station.
    let page = b.get("/compte?lang=fr");
    assert!(html(&page).contains("PC du salon") && html(&page).contains("En ligne"));
    let r = b.post("/compte/ordinateurs/detacher?lang=fr", &[("id", &st)]);
    assert!(html(&r).contains("Ordinateur détaché"));
    assert!(matches!(link.last(), Some((_, Msg::Detached { req: None }))));
    assert_eq!(w.acc.attach_status(&st, 10), Msg::Detached { req: Some(10) });
    // Détachée dans Admin (`detach`) : effacée chez le relais aussi.
    attach(&w, &mut b, &st);
    w.acc.station_detached(&st);
    assert!(w.acc.db().station(&st).unwrap().is_none());
}

#[test]
fn rattacher_une_station_hors_ligne_et_codes_d_un_autre() {
    let w = world(true);
    let _link = linked(&w);
    let mut b = signed_in(&w);
    let st = station(2);
    let (code, _, _) = w.acc.attach_request(&st, "Mac");
    let r = b.post("/compte/rattacher?lang=fr", &[("code", &code), ("action", "rattacher")]);
    assert!(html(&r).contains("à sa prochaine connexion"), "hors ligne : elle le saura en se reconnectant");
    assert!(w.acc.db().station(&st).unwrap().is_some());
    // Un code inconnu, ou échu (15 min) : refusé.
    assert_eq!(b.get("/compte/rattacher?code=inconnu&lang=fr").status, 400);
    let (code, _, _) = w.acc.attach_request(&st, "Mac");
    w.advance(15 * 60);
    assert_eq!(b.get(&format!("/compte/rattacher?code={code}&lang=fr")).status, 400);
    // Un autre compte ne détache pas l'ordinateur d'Ana.
    w.acc.add_verified("bob@exemple.org", "Bob", PASSWORD).unwrap();
    let mut bob = w.browser_at("198.51.100.7");
    bob.sign_in("bob@exemple.org", PASSWORD);
    bob.post("/compte/ordinateurs/detacher?lang=fr", &[("id", &st)]);
    assert!(w.acc.db().station(&st).unwrap().is_some());
}

#[test]
fn connecter_une_app_par_la_fenetre_du_systeme() {
    let w = world(true);
    let link = linked(&w);
    w.acc.add_verified("ana@exemple.org", "Ana", PASSWORD).unwrap();
    let mut b = w.browser();
    // Un autre retour que le schéma des apps : refusé.
    assert_eq!(b.get("/compte/connexion?lang=fr&appareil=iPad&retour=https://ailleurs.example").status, 400);
    let page = b.get("/compte/connexion?lang=fr&appareil=iPad%20de%20L%C3%A9a&retour=kybernow://compte&etat=abc123");
    assert!(!html(&page).contains("Créer un compte"));
    let r = b.post("/compte/connexion?lang=fr&appareil=iPad%20de%20L%C3%A9a", &[("email", "ana@exemple.org"), ("password", PASSWORD)]);
    let next = location(&r).to_string();
    assert_eq!(next, "/compte/connecter-appareil?lang=fr&appareil=iPad%20de%20L%C3%A9a&etat=abc123");
    let consent = b.get(&next);
    assert!(html(&consent).contains("Connecter KyberNow Link sur iPad de Léa\u{202F}?"));
    assert!(html(&consent).contains(r#"name="etat" value="abc123""#));
    // Annuler : retour vers l'app, sans code.
    let r = b.post("/compte/connecter-appareil?lang=fr&appareil=iPad%20de%20L%C3%A9a", &[("etat", "abc123"), ("action", "annuler")]);
    assert!(html(&r).contains("kybernow://compte?erreur=annule&amp;etat=abc123"));
    let r = b.post("/compte/connecter-appareil?lang=fr&appareil=iPad%20de%20L%C3%A9a", &[("etat", "abc123"), ("action", "connecter")]);
    let page = html(&r);
    assert!(page.contains("<meta http-equiv=\"refresh\" content=\"0;url=kybernow://compte?code="));
    let code = between(page, "kybernow://compte?code=", "&amp;etat=abc123").to_string();
    // L'app échange le code, une seule fois, contre sa session d'appareil.
    let t = api(&w, true, "/v1/devices/token", None, json!({ "code": code }));
    assert_eq!(t.status, 200, "{:?}", t.body);
    assert_eq!(t.body["device"]["name"], "iPad de Léa");
    assert_eq!(t.body["account"]["email"], "ana@exemple.org");
    assert_eq!(t.body["relay"], "share.test");
    let token = t.body["token"].as_str().unwrap().to_string();
    assert_eq!(api(&w, true, "/v1/devices/token", None, json!({ "code": code })).status, 400, "code déjà servi");
    // Déjà connectée dans ce navigateur : l'app suivante va droit à la question.
    let r = b.get("/compte/connexion?lang=fr&appareil=Mac&retour=kybernow://compte");
    assert_eq!(location(&r), "/compte/connecter-appareil?lang=fr&appareil=Mac");

    // Les ordinateurs du compte, puis un jeton d'accès pour l'un d'eux.
    let st = station(3);
    link.online.lock().unwrap().insert(st.clone());
    assert_eq!(api(&w, false, "/v1/computers", Some(&token), Value::Null).body, json!({ "computers": [] }));
    assert_eq!(api(&w, true, "/v1/access", Some(&token), json!({ "station": st })).status, 404, "pas rattachée à ce compte");
    attach(&w, &mut b, &st);
    let list = api(&w, false, "/v1/computers", Some(&token), Value::Null);
    assert_eq!(list.body["computers"][0]["station"], st.as_str());
    assert_eq!(list.body["computers"][0]["rights"], json!(["session", "lend", "share", "admin"]));
    assert_eq!(list.body["computers"][0]["online"], true);
    // Connectée : l'empreinte d'Admin attestée, pour l'épingler dans l'app.
    assert_eq!(list.body["computers"][0]["admin_fp"], format!("sha256:{st}"));
    assert_eq!((list.body["computers"][0]["station_key"].as_str(), list.body["computers"][0]["sig"].as_str()), (Some("Y2xl"), Some("c2ln")));
    let mails = w.outbox.count_to("ana@exemple.org");
    let access = api(&w, true, "/v1/access", Some(&token), json!({ "station": st }));
    assert_eq!(access.status, 200);
    let jwt = access.body["token"].as_str().unwrap();
    let verifier = Verifier::new("share.test", &st, &w.acc.stations.keys.key_set().keys);
    let claims = verifier.verify(jwt, w.now() as u64, Use::Reusable).expect("jeton signé par la clé publiée");
    assert_eq!(claims.exp - claims.iat, 300);
    assert_eq!(claims.name.device, "iPad de Léa");
    assert_eq!(claims.device, t.body["device"]["id"].as_str().unwrap());
    assert_eq!(claims.account, t.body["account"]["id"].as_str().unwrap());
    // Garde-fou 4 : un e-mail au propriétaire la première fois, pas ensuite.
    assert_eq!(w.outbox.count_to("ana@exemple.org"), mails + 1);
    let notice = w.mail("ana@exemple.org");
    assert_eq!(notice.subject, "Nouvel appareil sur PC du salon");
    assert!(notice.text.contains("iPad de Léa vient d'ouvrir PC du salon pour la première fois"));
    api(&w, true, "/v1/access", Some(&token), json!({ "station": st }));
    assert_eq!(w.outbox.count_to("ana@exemple.org"), mails + 1);
    let account = w.acc.db().account_by_email("ana@exemple.org").unwrap().unwrap();
    assert_eq!(w.acc.db().audit_of(account.id).unwrap().iter().filter(|(k, _)| k == "nouvel_appareil").count(), 1);
    // Sans session d'appareil, ou avec une mauvaise : 401.
    assert_eq!(api(&w, false, "/v1/computers", None, Value::Null).status, 401);
    assert_eq!(api(&w, true, "/v1/access", Some("faux"), json!({ "station": st })).status, 401);
    assert_eq!(api(&w, true, "/v1/access", Some(&token), json!({ "station": "pas-une-station" })).status, 400);
    // La page du compte montre l'appareil ; « Déconnecter » ferme sa session.
    let page = b.get("/compte?lang=fr");
    assert!(html(&page).contains("Appareils connectés") && html(&page).contains("iPad de Léa"));
    let id = w.acc.db().devices_of(account.id).unwrap()[0].id.to_string();
    let r = b.post("/compte/appareils/deconnecter?lang=fr", &[("id", &id)]);
    assert!(html(&r).contains("Appareil déconnecté"));
    assert_eq!(api(&w, false, "/v1/computers", Some(&token), Value::Null).status, 401);
}

#[test]
fn connecter_un_televiseur_par_un_code() {
    let w = world(true);
    let mut b = signed_in(&w);
    let tv = b.get("/tv?lang=fr");
    assert!(html(&tv).contains("<form method=\"get\" action=\"/compte/appareil\">"));
    assert_eq!(api(&w, true, "/v1/devices/code", None, json!({})).status, 400, "sans nom");
    let c = api(&w, true, "/v1/devices/code", None, json!({ "name": "Apple TV du salon" }));
    assert_eq!(c.status, 200);
    let code = c.body["code"].as_str().unwrap().to_string();
    let poll = c.body["poll"].as_str().unwrap().to_string();
    assert_eq!((code.len(), c.body["expires"].as_i64(), c.body["interval"].as_u64()), (9, Some(600), Some(5)));
    assert_eq!(c.body["url"], "https://share.test/tv");
    assert_eq!(api(&w, true, "/v1/devices/poll", None, json!({ "poll": poll })).status, 202);
    // Tapé en minuscules, sans tiret : la question de la maquette.
    let typed = code.replace('-', " ").to_lowercase();
    let page = b.get(&format!("/compte/appareil?code={typed}&lang=fr"));
    assert!(html(&page).contains("Connecter Apple TV du salon\u{202F}?") && html(&page).contains("Ce n'est pas moi") && html(&page).contains(&code));
    let r = b.post("/compte/appareil?lang=fr", &[("code", &code), ("action", "connecter")]);
    assert!(html(&r).contains("Appareil connecté"));
    let done = api(&w, true, "/v1/devices/poll", None, json!({ "poll": poll }));
    assert_eq!(done.status, 200);
    assert_eq!(done.body["device"]["name"], "Apple TV du salon");
    assert!(done.body["token"].as_str().unwrap().len() >= 43);
    assert_eq!(api(&w, true, "/v1/devices/poll", None, json!({ "poll": poll })).status, 404, "une seule session par code");
    let account = w.acc.db().account_by_email("ana@exemple.org").unwrap().unwrap();
    assert_eq!(w.acc.db().devices_of(account.id).unwrap()[0].kind, "tv");
    // « Ce n'est pas moi » : refusé, l'appareil le sait.
    let c = api(&w, true, "/v1/devices/code", None, json!({ "name": "TV" }));
    let r = b.post("/compte/appareil?lang=fr", &[("code", c.body["code"].as_str().unwrap()), ("action", "refuser")]);
    assert!(html(&r).contains("Appareil refusé"));
    assert_eq!(api(&w, true, "/v1/devices/poll", None, json!({ "poll": c.body["poll"] })).status, 403);
    // Un code faux : redemandé ; un code échu : l'appareil le sait (410).
    assert!(html(&b.get("/compte/appareil?code=BCDF-GHJK&lang=fr")).contains("Code incorrect"));
    let c = api(&w, true, "/v1/devices/code", None, json!({ "name": "TV" }));
    w.advance(10 * 60);
    assert!(html(&b.get(&format!("/compte/appareil?code={}&lang=fr", c.body["code"].as_str().unwrap()))).contains("Code incorrect"));
    assert_eq!(api(&w, true, "/v1/devices/poll", None, json!({ "poll": c.body["poll"] })).status, 410);
    // Pas connectée : la page est retenue pour après la connexion.
    let mut other = w.browser_at("198.51.100.9");
    assert_eq!(location(&other.get("/compte/appareil?code=BCDF-GHJK&lang=fr")), "/compte/connexion?lang=fr");
    assert_eq!(location(&other.sign_in("ana@exemple.org", PASSWORD)), "/compte/appareil?code=BCDF-GHJK&lang=fr");
}

#[test]
fn les_codes_de_televiseur_sont_limites() {
    let w = world(true);
    let mut b = signed_in(&w);
    for _ in 0..10 {
        assert_eq!(b.get("/compte/appareil?code=BCDF-GHJK&lang=fr").status, 200);
    }
    assert_eq!(b.get("/compte/appareil?code=BCDF-GHJK&lang=fr").status, 429);
    for _ in 0..20 {
        assert_eq!(api(&w, true, "/v1/devices/code", None, json!({ "name": "TV" })).status, 200);
    }
    let r = api(&w, true, "/v1/devices/code", None, json!({ "name": "TV" }));
    assert_eq!(r.status, 429);
    assert!(r.retry_after.is_some());
}

#[test]
fn un_nouveau_mot_de_passe_deconnecte_les_appareils() {
    let w = world(true);
    let _b = signed_in(&w);
    let account = w.acc.db().account_by_email("ana@exemple.org").unwrap().unwrap();
    let session = w.acc.open_device(&account, "iPad", "app").unwrap();
    let token = session["token"].as_str().unwrap();
    assert_eq!(api(&w, false, "/v1/computers", Some(token), Value::Null).status, 200);
    let mut b = w.browser_at("198.51.100.8");
    b.post("/compte/oubli?lang=fr", &[("email", "ana@exemple.org")]);
    let link = w.mail("ana@exemple.org").text;
    let jeton = between(&link, "nouveau-mot-de-passe?jeton=", "&").to_string();
    b.post("/compte/nouveau-mot-de-passe?lang=fr", &[("jeton", &jeton), ("password", "autre cheval agrafe")]);
    assert_eq!(api(&w, false, "/v1/computers", Some(token), Value::Null).status, 401);
    // Et l'app se déconnecte elle-même.
    let session = w.acc.open_device(&account, "iPad", "app").unwrap();
    let token = session["token"].as_str().unwrap();
    assert_eq!(api(&w, true, "/v1/devices/revoke", Some(token), Value::Null).status, 200);
    assert_eq!(api(&w, false, "/v1/computers", Some(token), Value::Null).status, 401);
}

// --- Avec le lot 1b : passkeys, export, suppression ---------------------------------

#[test]
fn passkey_depuis_une_app_export_et_suppression_avec_stations_et_appareils() {
    use super::super::passkeys::tests::Authenticator;
    use super::super::passkeys::ES256;

    let w = world(true);
    let link = linked(&w);
    let mut b = signed_in(&w);
    let ana = w.account_id("ana@exemple.org");
    let account = w.acc.db().account(ana).unwrap().unwrap();
    // Une Station rattachée, connectée, et un appareil vu sur elle.
    let st = station(7);
    link.online.lock().unwrap().insert(st.clone());
    attach(&w, &mut b, &st);
    let device = w.acc.open_device(&account, "iPad de Léa", "app").unwrap();
    let token = device["token"].as_str().unwrap().to_string();
    assert_eq!(api(&w, true, "/v1/access", Some(&token), json!({ "station": st })).status, 200);
    // Une passkey, puis la connexion par elle depuis une app : la question de l'appareil, comme après le mot de passe.
    let mut auth = Authenticator::new(ES256, "https://share.test");
    auth.user = w.acc.db().webauthn_user(ana).unwrap();
    b.add_passkey(&auth);
    b.post("/compte/deconnexion?lang=fr", &[]);
    let q = "?lang=fr&appareil=Mac";
    let r = b.passkey(&format!("/compte/connexion{q}&retour=kybernow://compte"), &format!("/compte/connexion/passkey{q}"), &mut auth, &[]);
    assert_eq!(location(&r), format!("/compte/connecter-appareil{q}"));
    assert!(html(&b.get(&format!("/compte/connecter-appareil{q}"))).contains("Connecter KyberNow Link sur Mac"));
    // L'export donne les ordinateurs rattachés et les appareils connectés, sans jeton.
    let r = b.post("/compte/export?lang=fr", &[]);
    let file = r.download.expect("un fichier");
    let data: Value = serde_json::from_str(&file.body).unwrap();
    assert_eq!(data["stations"][0]["id"], st.as_str());
    assert_eq!(data["stations"][0]["name"], "PC du salon");
    assert_eq!(data["devices"][0]["name"], "iPad de Léa");
    assert!(!file.body.contains(&token));
    // La suppression du compte détache ses Stations (prévenues) et ferme ses appareils.
    let r = b.post("/compte/supprimer?lang=fr", &[("action", "password"), ("password", PASSWORD)]);
    assert!(html(&r).contains("Compte supprimé"));
    assert!(matches!(link.last(), Some((ref to, Msg::Detached { req: None })) if *to == st));
    assert!(w.acc.db().station(&st).unwrap().is_none());
    assert_eq!(w.acc.attach_status(&st, 1), Msg::Detached { req: Some(1) });
    assert!(w.acc.db().devices_of(ana).unwrap().is_empty());
    assert_eq!(api(&w, false, "/v1/computers", Some(&token), Value::Null).status, 401);
    let left: i64 = w.acc.db().conn.query_row("SELECT COUNT(*) FROM device_stations", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);
}
