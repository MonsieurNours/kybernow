//! Audit du 27/11, REL-06 et REL-07 : une session ancienne redonne le mot de passe avant d'ajouter
//! une passkey ou d'activer la double authentification ; un nouveau mot de passe retire les passkeys
//! récentes ; un secret TOTP illisible ferme la porte au lieu de l'ouvrir.

use super::lot45::person;
use super::*;

#[test]
fn session_ancienne_le_mot_de_passe_d_abord() {
    let w = world(true);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    w.advance(REAUTH_SECS + 1);
    // La page demande le mot de passe, et le formulaire de passkey aussi.
    let page = ana.get("/compte/double-authentification?lang=fr");
    assert!(html(&page).contains("Mot de passe, pour confirmer"));
    assert!(html(&ana.get("/compte?lang=fr")).contains("password-passkey"));
    // Le bon code sans mot de passe : refusé ; rien n'est activé.
    let r = ana.post("/compte/double-authentification?lang=fr", &[("code", &w.totp_code("ana@exemple.org", 0))]);
    assert!(html(&r).contains("Mot de passe incorrect"));
    let id = w.acc.db().account_by_email("ana@exemple.org").unwrap().unwrap().id;
    assert!(w.acc.db().totp(id).unwrap().unwrap().activated_at.is_none());
    // Une passkey sans mot de passe : refusée avant même de lire l'attestation.
    let r = ana.post("/compte/passkeys/ajouter?lang=fr", &[("client_data", "x"), ("attestation", "y")]);
    assert!(html(&r).contains("Mot de passe incorrect"));
    // Avec le mot de passe : activée.
    let r = ana.post("/compte/double-authentification?lang=fr", &[("code", &w.totp_code("ana@exemple.org", 0)), ("password", PASSWORD)]);
    assert!(html(&r).contains("Codes de secours"));
}

#[test]
fn session_recente_sans_mot_de_passe() {
    let w = world(true);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    let page = ana.get("/compte/double-authentification?lang=fr");
    assert!(!html(&page).contains("Mot de passe, pour confirmer"));
    assert!(!html(&ana.get("/compte?lang=fr")).contains("password-passkey"));
}

#[test]
fn nouveau_mot_de_passe_retire_les_passkeys_recentes() {
    let w = world(true);
    w.acc.add_verified("ana@exemple.org", "Ana", PASSWORD).unwrap();
    let id = w.acc.db().account_by_email("ana@exemple.org").unwrap().unwrap().id;
    let key = |c: &[u8]| crate::accounts::store::NewPasskey { credential: c.to_vec(), public_key: b"k".to_vec(), alg: -7, counter: 0 };
    w.acc.db().add_passkey(id, &key(b"ancienne"), "Ancienne", "", w.now() - RESET_PASSKEY_SECS - 10).unwrap();
    w.acc.db().add_passkey(id, &key(b"recente"), "Récente", "", w.now() - 60).unwrap();
    let mut b = w.browser();
    b.post("/compte/oubli?lang=fr", &[("email", "ana@exemple.org")]);
    let link = between(&w.mail("ana@exemple.org").text, "https://share.test", "\n").to_string();
    let token = between(&link, "jeton=", "&").to_string();
    let r = b.post("/compte/nouveau-mot-de-passe?lang=fr", &[("jeton", &token), ("password", "lune verte et sel")]);
    assert!(html(&r).contains("Mot de passe changé"));
    let names: Vec<String> = w.acc.db().passkeys(id).unwrap().into_iter().map(|p| p.name).collect();
    assert_eq!(names, vec!["Ancienne".to_string()]);
}

#[test]
fn secret_totp_illisible_refuse() {
    let w = world(true);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    super::lot45::with_totp(&w, &mut ana, "ana@exemple.org");
    let id = w.acc.db().account_by_email("ana@exemple.org").unwrap().unwrap().id;
    // Le secret chiffré avec une autre clé : illisible.
    let other = crate::accounts::Keys::new(&[7u8; 32]);
    w.acc.db().set_pending_totp(id, &other.seal(id, b"0123456789abcdefghij")).unwrap();
    w.acc.db().activate_totp(id, 0, w.now()).unwrap();
    ana.post("/compte/deconnexion?lang=fr", &[]);
    assert_eq!(location(&ana.sign_in("ana@exemple.org", PASSWORD)), "/compte/code?lang=fr");
    let r = ana.post("/compte/code?lang=fr", &[("code", "123456")]);
    assert!(html(&r).contains("Code incorrect"), "plus jamais ouverte");
    assert_eq!(location(&ana.get("/compte?lang=fr")), "/compte/code?lang=fr");
}

#[test]
fn inscription_sur_une_adresse_prise_ne_change_pas_le_compte_d_un_autre() {
    // REL-05 : la victime s'inscrit sans vérifier ; un tiers s'inscrit avec son adresse puis « change d'adresse ».
    let w = world(true);
    let mut victim = w.browser_at("192.0.2.30");
    victim.post("/compte/inscription?lang=fr", &[("name", "Léa"), ("email", "lea@exemple.org"), ("password", PASSWORD), ("conditions", "1")]);
    let mut thief = w.browser_at("198.51.100.66");
    let r = thief.post("/compte/inscription?lang=fr", &[("name", "X"), ("email", "lea@exemple.org"), ("password", "un autre mot long"), ("conditions", "1")]);
    assert_eq!(location(&r), "/compte/verifier?lang=fr", "même réponse qu'une inscription");
    let r = thief.post("/compte/verifier?lang=fr", &[("action", "changer"), ("email", "voleur@exemple.org")]);
    assert_eq!(r.status, 200);
    assert!(w.acc.db().account_by_email("lea@exemple.org").unwrap().is_some(), "le compte de Léa garde son adresse");
    assert!(w.outbox.last_to("voleur@exemple.org").is_none(), "aucun code ne part chez le tiers");
    // Léa, elle, peut encore changer d'adresse et vérifier.
    let r = victim.post("/compte/verifier?lang=fr", &[("action", "changer"), ("email", "lea.martin@exemple.org")]);
    assert_eq!(r.status, 200);
    let code: String = between(&w.mail("lea.martin@exemple.org").text, "    ", "\n").chars().filter(char::is_ascii_digit).collect();
    let r = victim.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", &code)]);
    assert_eq!(location(&r), "/compte/bienvenue?lang=fr");
}

#[test]
fn retour_vers_l_app_avec_pkce() {
    // REL-09 : l'app donne un défi S256 ; le code ne s'échange qu'avec son vérificateur.
    use super::lot2::api;
    use serde_json::json;
    let w = world(true);
    let mut b = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"; // RFC 7636, annexe B
    assert!(crate::accounts::devices::verifier_matches(verifier, challenge));
    let r = b.get(&format!("/compte/connexion?lang=fr&appareil=Mac&retour=kybernow://compte&etat=e1&code_challenge={challenge}&code_challenge_method=S256"));
    let next = location(&r).to_string();
    assert!(next.contains(&format!("code_challenge={challenge}")), "{next}");
    let consent = b.get(&next);
    assert!(html(&consent).contains(&format!(r#"name="code_challenge" value="{challenge}""#)));
    let connect = |b: &mut Browser| {
        let r = b.post("/compte/connecter-appareil?lang=fr&appareil=Mac", &[("etat", "e1"), ("code_challenge", challenge), ("action", "connecter")]);
        between(html(&r), "kybernow://compte?code=", "&amp;etat=e1").to_string()
    };
    let code = connect(&mut b);
    assert_eq!(api(&w, true, "/v1/devices/token", None, json!({ "code": code })).status, 400, "sans vérificateur");
    let code = connect(&mut b);
    assert_eq!(api(&w, true, "/v1/devices/token", None, json!({ "code": code, "code_verifier": "x".repeat(43) })).status, 400, "mauvais vérificateur");
    let code = connect(&mut b);
    assert_eq!(api(&w, true, "/v1/devices/token", None, json!({ "code": code, "code_verifier": verifier })).status, 200);
    // Une app d'avant PKCE (sans défi) passe comme avant.
    let r = b.post("/compte/connecter-appareil?lang=fr&appareil=Mac", &[("etat", "e2"), ("action", "connecter")]);
    let code = between(html(&r), "kybernow://compte?code=", "&amp;etat=e2").to_string();
    assert_eq!(api(&w, true, "/v1/devices/token", None, json!({ "code": code })).status, 200);
}
