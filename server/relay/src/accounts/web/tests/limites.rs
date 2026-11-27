//! Audit du 27/11, REL-13 : un compte n'envoie pas d'invitations sans fin.

use super::lot2::{attach, linked, station};
use super::lot45::{person, with_totp};
use super::*;
use crate::accounts::{INVITES_PER_HOUR, MAX_OWNED_ORGS};

#[test]
fn invitations_limitees_par_expediteur() {
    let w = world(true);
    let link = linked(&w);
    let st = station(61);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    with_totp(&w, &mut ana, "ana@exemple.org");
    attach(&w, &mut ana, &st);
    ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Atelier")]);
    let org = w.acc.db().orgs_of(1).unwrap()[0].0.public_id.clone();
    // Les invitations d'équipe et les partages d'ordinateur comptent ensemble.
    for i in 0..INVITES_PER_HOUR {
        let email = format!("p{i}@exemple.org");
        let r = if i % 2 == 0 {
            ana.post("/compte/organisation/inviter?lang=fr", &[("id", &org), ("email", &email), ("role", "member")])
        } else {
            ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", &email), ("session", "1")])
        };
        assert_eq!(r.status, 200, "invitation {i}");
    }
    let r = ana.post("/compte/organisation/inviter?lang=fr", &[("id", &org), ("email", "trop@exemple.org"), ("role", "member")]);
    assert_eq!(r.status, 429);
    assert!(w.outbox.last_to("trop@exemple.org").is_none());
    let r = ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "trop@exemple.org"), ("session", "1")]);
    assert_eq!(r.status, 429);
    // Un autre compte n'est pas gêné.
    let mut bob = person(&w, "bob@exemple.org", "Bob", "192.0.2.11");
    with_totp(&w, &mut bob, "bob@exemple.org");
    bob.post("/compte/organisations/creer?lang=fr", &[("nom", "Ailleurs")]);
    let bob_id = w.acc.db().account_by_email("bob@exemple.org").unwrap().unwrap().id;
    let bob_org = w.acc.db().orgs_of(bob_id).unwrap()[0].0.public_id.clone();
    assert_eq!(bob.post("/compte/organisation/inviter?lang=fr", &[("id", &bob_org), ("email", "x@exemple.org"), ("role", "member")]).status, 200);
}

#[test]
fn pas_d_equipes_a_la_chaine() {
    let w = world(false);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    with_totp(&w, &mut ana, "ana@exemple.org");
    for i in 0..MAX_OWNED_ORGS {
        ana.post("/compte/organisations/creer?lang=fr", &[("nom", &format!("Équipe {i}"))]);
    }
    let r = ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Une de trop")]);
    assert_eq!(r.status, 429);
    assert_eq!(w.acc.db().orgs_of(1).unwrap().len(), MAX_OWNED_ORGS);
}
