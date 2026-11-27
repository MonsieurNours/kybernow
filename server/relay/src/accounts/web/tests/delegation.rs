//! Audit du 27/11, REL-02 à REL-04 : un droit Administrer reçu ne donne que ce qu'on a, jusqu'à son
//! échéance ; ce qu'on a donné part avec son droit ; quitter une équipe retire ses droits sur ses ordinateurs.

use kybernow_tunnel::access::{Right, Rights};

use super::lot2::{attach, linked, station};
use super::lot45::{person, with_totp};
use super::*;
use crate::accounts::orgs::{Kind, Role};

fn grant_end(w: &World, st: &str, email: &str) -> Option<Option<i64>> {
    let id = w.acc.db().account_by_email(email).unwrap().unwrap().id;
    let ends = w.acc.db().rights_ends(id, st, w.now()).unwrap();
    if ends.is_empty() { None } else { Some(ends.iter().map(|(_, e)| *e).max().flatten()) }
}

#[test]
fn administrer_delegue_borne_et_retire_en_cascade() {
    let w = world(true);
    let link = linked(&w);
    let st = station(51);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    // Ana donne Session et Administrer à Xavier jusqu'au 30/11/2026 (T0 : 23/11).
    let mut xav = person(&w, "xav@exemple.org", "Xavier", "192.0.2.20");
    with_totp(&w, &mut xav, "xav@exemple.org");
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "xav@exemple.org"), ("session", "1"), ("admin", "1"), ("fin", "2026-11-30")]);
    let xav_end = grant_end(&w, &st, "xav@exemple.org").unwrap().unwrap();
    // Xavier ne donne pas Prêt, qu'il n'a pas.
    let _yan = person(&w, "yan@exemple.org", "Yan", "192.0.2.21");
    xav.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "yan@exemple.org"), ("lend", "1")]);
    assert_eq!(grant_end(&w, &st, "yan@exemple.org"), None, "Prêt refusé");
    // Session sans échéance pour Yan : ramenée à celle de Xavier.
    xav.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "yan@exemple.org"), ("session", "1")]);
    assert_eq!(grant_end(&w, &st, "yan@exemple.org"), Some(Some(xav_end)));
    // Un lien Administrer sans fin, accepté par un second compte : jusqu'à l'échéance de Xavier aussi.
    let r = xav.post("/compte/ordinateur/liens/creer?lang=fr", &[("id", &st), ("jours", "7"), ("admin", "1"), ("session", "1")]);
    let path = between(html(&r), "<p class=\"token\">https://share.test", "<").to_string();
    let mut zoe = person(&w, "zoe@exemple.org", "Zoé", "192.0.2.22");
    with_totp(&w, &mut zoe, "zoe@exemple.org");
    zoe.post(&format!("{path}?lang=fr"), &[]);
    assert_eq!(grant_end(&w, &st, "zoe@exemple.org"), Some(Some(xav_end)), "le second compte n'a pas Administrer pour toujours");
    // Un deuxième lien, pas encore accepté.
    let r = xav.post("/compte/ordinateur/liens/creer?lang=fr", &[("id", &st), ("jours", "7"), ("session", "1")]);
    let pending = between(html(&r), "<p class=\"token\">https://share.test", "<").to_string();
    // Ana retire Xavier : Yan et Zoé (donnés par lui) partent, son lien en attente ne sert plus.
    let xav_id = w.acc.db().account_by_email("xav@exemple.org").unwrap().unwrap().id;
    let grant = w.acc.db().grants_of(&st).unwrap().into_iter().find(|g| matches!(g.subject, crate::accounts::people::Subject::Person { account, .. } if account == xav_id)).unwrap();
    ana.post("/compte/ordinateur/retirer?lang=fr", &[("id", &st), ("acces", &grant.id.to_string())]);
    for who in ["xav@exemple.org", "yan@exemple.org", "zoe@exemple.org"] {
        assert_eq!(grant_end(&w, &st, who), None, "{who} n'a plus rien");
    }
    let mut sam = person(&w, "sam@exemple.org", "Sam", "192.0.2.23");
    let r = sam.post(&format!("{pending}?lang=fr"), &[]);
    assert_eq!(r.status, 410, "le lien de Xavier est révoqué");
    assert_eq!(grant_end(&w, &st, "sam@exemple.org"), None);
}

#[test]
fn quitter_l_equipe_retire_les_droits_sur_ses_ordinateurs() {
    let w = world(false);
    let db = w.acc.db();
    let ana = db.create_account("ana@exemple.org", "Ana", "h", "fr", Some(0), T0).unwrap().unwrap();
    let bob = db.create_account("bob@exemple.org", "Bob", "h", "fr", Some(0), T0).unwrap().unwrap();
    let org = db.create_org("Atelier", Kind::Team, ana, T0).unwrap();
    db.set_member(org.id, bob, Role::Member, T0).unwrap();
    db.attach_station_to_org("st-org", "Rendu", org.id, Some(ana), None, T0).unwrap();
    db.attach_station("st-ana", "PC d'Ana", ana, ana, T0).unwrap();
    let session = Rights::new([Right::Session]);
    db.grant_person("st-org", bob, &session, None, ana, T0).unwrap();
    db.grant_person("st-ana", bob, &session, None, ana, T0).unwrap();
    assert!(db.remove_member(org.id, bob, T0).unwrap());
    assert!(db.granted_rights(bob, "st-org", T0).unwrap().is_empty(), "l'ordinateur de l'équipe n'est plus ouvert");
    assert_eq!(db.granted_rights(bob, "st-ana", T0).unwrap(), session, "celui d'Ana, hors de l'équipe, reste");
}

#[test]
fn un_administrateur_ne_sort_pas_un_ordinateur_de_l_equipe() {
    // REL-12 : déplacer ou détacher un ordinateur de l'équipe, c'est aux Propriétaires.
    let w = world(false);
    let (ana, bob, st) = {
        let db = w.acc.db();
        let ana = db.create_account("ana@exemple.org", "Ana", "h", "fr", Some(0), T0).unwrap().unwrap();
        let bob = db.create_account("bob@exemple.org", "Bob", "h", "fr", Some(0), T0).unwrap().unwrap();
        let org = db.create_org("Atelier", Kind::Team, ana, T0).unwrap();
        db.set_member(org.id, bob, Role::Admin, T0).unwrap();
        db.attach_station_to_org("st-org", "Rendu", org.id, Some(ana), None, T0).unwrap();
        for a in [ana, bob] {
            let key = crate::accounts::store::NewPasskey { credential: format!("cle-{a}").into_bytes(), public_key: b"k".to_vec(), alg: -7, counter: 0 };
            db.add_passkey(a, &key, "Clé", "", T0).unwrap();
        }
        (db.account(ana).unwrap().unwrap(), db.account(bob).unwrap().unwrap(), db.station("st-org").unwrap().unwrap())
    };
    assert!(w.acc.may_move(&ana, &st));
    assert!(!w.acc.may_move(&bob, &st), "un Administrateur ne déplace pas");
    assert!(!w.acc.detach_by_account("st-org", &bob).unwrap(), "ni ne détache");
    assert!(w.acc.db().station("st-org").unwrap().is_some());
    assert!(w.acc.detach_by_account("st-org", &ana).unwrap());
}

#[test]
fn pas_de_jetons_d_enrolement_pour_une_famille() {
    // REL-27 : même par un POST direct.
    let w = world(false);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    with_totp(&w, &mut ana, "ana@exemple.org");
    ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Les Martin"), ("genre", "famille")]);
    let org = w.acc.db().orgs_of(1).unwrap()[0].0.clone();
    assert_eq!(org.kind, Kind::Family);
    ana.post("/compte/organisation/jetons/creer?lang=fr", &[("id", &org.public_id), ("nom", "Salon"), ("jours", "7"), ("nombre", "5")]);
    assert!(w.acc.db().enroll_tokens(org.id).unwrap().is_empty());
    let r = ana.get(&format!("/compte/organisation/deployer?id={}&lang=fr", org.public_id));
    assert!(!html(&r).contains("jetons/creer"));
}
