//! Les parcours de « Invitations, ajout de membres et sessions » (docs/comptes.md) :
//! invitations qui expirent et qu'on renvoie, ajout sans doublon avec les
//! suggestions, « Personnes » sans doublon, appareils déconnectés après 90 jours.

use serde_json::{Value, json};

use super::lot2::{api, attach, linked, station};
use super::lot45::{computers, device, person, with_totp};
use super::*;
use crate::accounts::devices::DEVICE_IDLE_SECS;
use crate::accounts::people::{INVITE_SECS, Subject};

/// Le texte d'une page.
fn txt(r: &Reply) -> String {
    html(r).to_string()
}

/// Les invitations par adresse d'un ordinateur.
fn invites(w: &World, st: &str) -> Vec<(i64, String)> {
    w.acc
        .db()
        .grants_of(st)
        .unwrap()
        .into_iter()
        .filter_map(|g| match g.subject {
            Subject::Invite { email } => Some((g.id, email)),
            _ => None,
        })
        .collect()
}

/// Crée et vérifie un compte par la page d'inscription (les invitations passent à la vérification).
fn sign_up(w: &World, name: &str, email: &str, ip: &str) -> Browser {
    let mut b = w.browser_at(ip);
    b.post("/compte/inscription?lang=fr", &[("name", name), ("email", email), ("password", PASSWORD), ("conditions", "1")]);
    let code: String = between(&w.mail(email).text, "    ", "\n").chars().filter(char::is_ascii_digit).collect();
    b.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", &code)]);
    b
}

#[test]
fn ajouter_deux_fois_met_a_jour_sans_doublon() {
    let w = world(true);
    let link = linked(&w);
    let st = station(31);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    let _lea = person(&w, "lea@exemple.org", "Léa Martin", "192.0.2.11");
    // Léa, deux fois, l'adresse écrite autrement : ses droits changent, une seule ligne.
    let r = ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "lea@exemple.org"), ("session", "1")]);
    assert!(txt(&r).contains("Personne ajoutée"));
    let r = ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "  LEA@Exemple.ORG "), ("lend", "1")]);
    assert!(txt(&r).contains("Léa Martin avait déjà accès") && txt(&r).contains("ses droits sont mis à jour"), "{}", txt(&r));
    let grants = w.acc.db().grants_of(&st).unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].rights, kybernow_tunnel::access::Rights::new([kybernow_tunnel::access::Right::Lend]));
    // En anglais.
    let r = ana.post("/compte/ordinateur/ajouter?lang=en", &[("id", &st), ("email", "lea@exemple.org"), ("lend", "1")]);
    assert!(txt(&r).contains("Léa Martin already had access: their rights are updated."));
    // Une invitation par adresse, deux fois : remplacée, pas doublée, et repartie pour 30 jours.
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "sam@exemple.org"), ("session", "1")]);
    w.advance(10 * 86_400);
    let r = ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "Sam@exemple.org"), ("share", "1")]);
    assert!(txt(&r).contains("Invitation envoyée"));
    assert_eq!(invites(&w, &st).len(), 1);
    let g = w.acc.db().grants_of(&st).unwrap().into_iter().find(|g| matches!(g.subject, Subject::Invite { .. })).unwrap();
    assert_eq!(g.sent_at, w.now(), "envoyée de nouveau");
    assert_eq!(g.rights, kybernow_tunnel::access::Rights::new([kybernow_tunnel::access::Right::Share]));
}

#[test]
fn invitation_d_un_ordinateur_expiree_puis_renvoyee() {
    let w = world(true);
    let link = linked(&w);
    let st = station(32);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "sam@exemple.org"), ("lend", "1")]);
    let sent = w.now();
    // Sous l'adresse : envoyée le, expire le ; le bouton « Renvoyer l'invitation ».
    let page = ana.get(&format!("/compte/ordinateur?id={st}&lang=fr"));
    let expires = crate::accounts::pages::date(Lang::Fr, sent + INVITE_SECS);
    let sent_day = crate::accounts::pages::date(Lang::Fr, sent);
    assert!(txt(&page).contains(&format!("Invitation envoyée le {sent_day} · expire le {expires}")), "{}", txt(&page));
    assert!(txt(&page).contains("/compte/ordinateur/renvoyer?lang=fr") && txt(&page).contains("Renvoyer l'invitation"));
    // 31 jours après : expirée ; Sam s'inscrit, l'ordinateur ne passe pas, l'invitation reste listée.
    w.advance(31 * 86_400);
    ana.sign_in("ana@exemple.org", PASSWORD);
    let page = ana.get(&format!("/compte/ordinateur?id={st}&lang=fr"));
    assert!(txt(&page).contains(&format!("Invitation expirée le {expires}")));
    let _sam_b = sign_up(&w, "Sam", "sam@exemple.org", "192.0.2.12");
    let sam = device(&w, "sam@exemple.org", "Pixel");
    assert_eq!(computers(&w, &sam), json!([]), "une invitation expirée ne passe pas");
    let list = invites(&w, &st);
    assert_eq!(list.len(), 1, "elle reste dans la liste");
    let id = list[0].0.to_string();
    // Renvoyer : l'e-mail repart, les 30 jours aussi ; Sam a maintenant un compte vérifié : l'ordinateur passe.
    let before = w.outbox.count_to("sam@exemple.org");
    let r = ana.post("/compte/ordinateur/renvoyer?lang=fr", &[("id", &st), ("acces", &id)]);
    assert!(txt(&r).contains("Invitation renvoyée à sam@exemple.org"), "{}", txt(&r));
    assert_eq!(w.outbox.count_to("sam@exemple.org"), before + 1);
    assert_eq!(computers(&w, &sam)[0]["rights"], json!(["lend"]));
    assert!(invites(&w, &st).is_empty());
    let audit = w.acc.db().audit_of(1).unwrap();
    assert!(audit.iter().any(|(k, _)| k == "invitation_renvoyee"));
}

#[test]
fn renvoyer_une_fois_par_heure() {
    let w = world(true);
    let link = linked(&w);
    let st = station(33);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "zoe@exemple.org"), ("session", "1")]);
    let id = invites(&w, &st)[0].0.to_string();
    w.advance(60);
    let r = ana.post("/compte/ordinateur/renvoyer?lang=fr", &[("id", &st), ("acces", &id)]);
    assert!(txt(&r).contains("Invitation renvoyée à zoe@exemple.org"));
    let count = w.outbox.count_to("zoe@exemple.org");
    w.advance(30 * 60);
    let r = ana.post("/compte/ordinateur/renvoyer?lang=fr", &[("id", &st), ("acces", &id)]);
    assert!(txt(&r).contains("Invitation déjà renvoyée il y a moins d'une heure."), "{}", txt(&r));
    assert_eq!(w.outbox.count_to("zoe@exemple.org"), count, "pas d'e-mail");
    let r = ana.post("/compte/ordinateur/renvoyer?lang=en", &[("id", &st), ("acces", &id)]);
    assert!(txt(&r).contains("Invitation already resent less than an hour ago."));
    w.advance(31 * 60);
    let r = ana.post("/compte/ordinateur/renvoyer?lang=en", &[("id", &st), ("acces", &id)]);
    assert!(txt(&r).contains("Invitation resent to zoe@exemple.org."));
    // Une personne (pas une invitation) ne se renvoie pas ; un autre ordinateur non plus.
    let _lea = person(&w, "lea@exemple.org", "Léa", "192.0.2.11");
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "lea@exemple.org"), ("session", "1")]);
    let lea_grant = w.acc.db().grants_of(&st).unwrap().into_iter().find(|g| matches!(g.subject, Subject::Person { .. })).unwrap().id.to_string();
    let r = ana.post("/compte/ordinateur/renvoyer?lang=fr", &[("id", &st), ("acces", &lea_grant)]);
    assert!(!txt(&r).contains("Invitation renvoyée"));
    let mut bob = person(&w, "bob@exemple.org", "Bob", "192.0.2.14");
    assert_eq!(bob.post("/compte/ordinateur/renvoyer?lang=fr", &[("id", &st), ("acces", &id)]).status, 404);
}

#[test]
fn inviter_un_membre_deja_la_et_renvoyer() {
    let w = world(true);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    with_totp(&w, &mut ana, "ana@exemple.org");
    ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Les Lignes Claires"), ("genre", "equipe")]);
    let org = w.acc.db().orgs_of(1).unwrap()[0].0.clone();
    let id = org.public_id.clone();
    let _hugo = person(&w, "hugo@exemple.org", "Hugo", "192.0.2.13");
    let r = ana.post("/compte/organisation/inviter?lang=fr", &[("id", &id), ("email", "hugo@exemple.org"), ("role", "member")]);
    assert!(txt(&r).contains("Membre ajouté"));
    // Déjà membre : refusé, en le nommant (l'adresse écrite autrement).
    let r = ana.post("/compte/organisation/inviter?lang=fr", &[("id", &id), ("email", " HUGO@exemple.org"), ("role", "admin")]);
    assert!(txt(&r).contains("Hugo en fait déjà partie."), "{}", txt(&r));
    let hugo_id = w.acc.db().account_by_email("hugo@exemple.org").unwrap().unwrap().id;
    assert_eq!(w.acc.db().org_role(org.id, hugo_id).unwrap(), Some(crate::accounts::orgs::Role::Member), "rôle inchangé");
    let r = ana.post("/compte/organisation/inviter?lang=en", &[("id", &id), ("email", "hugo@exemple.org"), ("role", "member")]);
    assert!(txt(&r).contains("Hugo is already a member."));
    // Une invitation en attente, deux fois : remplacée (le rôle), pas doublée.
    ana.post("/compte/organisation/inviter?lang=fr", &[("id", &id), ("email", "paul@exemple.org"), ("role", "member")]);
    w.advance(3 * 86_400);
    ana.post("/compte/organisation/inviter?lang=fr", &[("id", &id), ("email", "Paul@exemple.org"), ("role", "admin")]);
    let list = w.acc.db().org_invites(org.id).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].role, list[0].sent_at), (crate::accounts::orgs::Role::Admin, w.now()));
    // Expirée (envoyée il y a 31 jours : la session d'Ana, protégée, ne tiendrait pas si longtemps) :
    // la ligne le dit ; Paul s'inscrit, il n'entre pas.
    let old = w.now() - 31 * 86_400;
    w.acc.db().conn.execute("UPDATE org_invites SET sent_at = ?1", [old]).unwrap();
    let page = ana.get(&format!("/compte/organisation?id={id}&lang=fr"));
    let expires = crate::accounts::pages::date(Lang::Fr, old + INVITE_SECS);
    assert!(txt(&page).contains(&format!("Invitation expirée le {expires}")) && txt(&page).contains("/compte/organisation/renvoyer?lang=fr"));
    let _paul = sign_up(&w, "Paul", "paul@exemple.org", "192.0.2.15");
    let paul_id = w.acc.db().account_by_email("paul@exemple.org").unwrap().unwrap().id;
    assert_eq!(w.acc.db().org_role(org.id, paul_id).unwrap(), None);
    assert_eq!(w.acc.db().org_invites(org.id).unwrap().len(), 1);
    // Renvoyer : il entre, avec le rôle de l'invitation ; une ligne au journal.
    let inv = list[0].id.to_string();
    let r = ana.post("/compte/organisation/renvoyer?lang=fr", &[("id", &id), ("invitation", &inv)]);
    assert!(txt(&r).contains("Invitation renvoyée à paul@exemple.org"), "{}", txt(&r));
    assert_eq!(w.acc.db().org_role(org.id, paul_id).unwrap(), Some(crate::accounts::orgs::Role::Admin));
    assert!(w.acc.db().org_invites(org.id).unwrap().is_empty());
    assert!(w.acc.db().org_log_of(org.id, 50).unwrap().iter().any(|l| l.kind == "invitation_renvoyee"));
    // Une invitation sans compte : une fois par heure.
    ana.post("/compte/organisation/inviter?lang=fr", &[("id", &id), ("email", "ines@exemple.org"), ("role", "member")]);
    let inv = w.acc.db().org_invites(org.id).unwrap()[0].id.to_string();
    let r = ana.post("/compte/organisation/renvoyer?lang=fr", &[("id", &id), ("invitation", &inv)]);
    assert!(txt(&r).contains("Invitation renvoyée à ines@exemple.org"));
    let r = ana.post("/compte/organisation/renvoyer?lang=fr", &[("id", &id), ("invitation", &inv)]);
    assert!(txt(&r).contains("Invitation déjà renvoyée il y a moins d'une heure."));
}

#[test]
fn suggestions_des_personnes_connues_seulement() {
    let w = world(true);
    let link = linked(&w);
    let st = station(34);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    with_totp(&w, &mut ana, "ana@exemple.org");
    ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Famille"), ("genre", "famille")]);
    let org = w.acc.db().orgs_of(1).unwrap()[0].0.clone();
    let _hugo = person(&w, "hugo@exemple.org", "Hugo Martin", "192.0.2.13");
    let _lea = person(&w, "lea@exemple.org", "Léa", "192.0.2.11");
    let _inconnu = person(&w, "inconnu@exemple.org", "Inconnu", "192.0.2.16");
    ana.post("/compte/organisation/inviter?lang=fr", &[("id", &org.public_id), ("email", "hugo@exemple.org"), ("role", "member")]);
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "lea@exemple.org"), ("session", "1")]);
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "sam@exemple.org"), ("session", "1")]);
    // « Ajouter une personne » : une liste du navigateur, « Prénom Nom · adresse ».
    let page = txt(&ana.get(&format!("/compte/ordinateur?id={st}&lang=fr")));
    assert!(page.contains(r#"list="connus""#) && page.contains(r#"<datalist id="connus">"#));
    assert!(page.contains(r#"<option value="hugo@exemple.org">Hugo Martin · hugo@exemple.org</option>"#), "{page}");
    assert!(page.contains(r#"<option value="lea@exemple.org">"#) && page.contains(r#"<option value="sam@exemple.org">"#));
    assert!(!page.contains("inconnu@exemple.org"), "jamais un inconnu");
    assert!(!page.contains(r#"<option value="ana@exemple.org">"#), "pas soi");
    // « Inviter un membre » : les connus qui n'en font pas partie.
    let page = txt(&ana.get(&format!("/compte/organisation?id={}&lang=fr", org.public_id)));
    assert!(page.contains(r#"<option value="lea@exemple.org">"#) && !page.contains(r#"<option value="hugo@exemple.org">"#));
    assert!(!page.contains("inconnu@exemple.org"));
    // Hugo, membre de la famille, voit Ana (et pas les personnes qu'Ana a ajoutées à ses ordinateurs).
    let hugo_id = w.acc.db().account_by_email("hugo@exemple.org").unwrap().unwrap().id;
    let known = w.acc.db().known_people(hugo_id).unwrap();
    assert_eq!(known, vec![("Ana".to_string(), "ana@exemple.org".to_string())]);
}

#[test]
fn personnes_une_fois_chacune() {
    let w = world(true);
    let link = linked(&w);
    let (st1, st2) = (station(35), station(36));
    link.online.lock().unwrap().extend([st1.clone(), st2.clone()]);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st1);
    attach(&w, &mut ana, &st2);
    // Sam : invité par son adresse sur le premier ; l'invitation expire, il s'inscrit ; puis ajouté par son compte sur le second.
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st1), ("email", "sam@exemple.org"), ("lend", "1")]);
    w.advance(31 * 86_400);
    ana.sign_in("ana@exemple.org", PASSWORD);
    let _sam = sign_up(&w, "Sam Petit", "sam@exemple.org", "192.0.2.12");
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st2), ("email", "sam@exemple.org"), ("session", "1")]);
    // Léa, par son compte, sur les deux.
    let _lea = person(&w, "lea@exemple.org", "Léa", "192.0.2.11");
    for st in [&st1, &st2] {
        ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", st), ("email", "lea@exemple.org"), ("session", "1")]);
    }
    let page = txt(&ana.get("/compte/personnes?lang=fr"));
    assert_eq!(page.matches("<b>Sam Petit</b>").count(), 1, "{page}");
    assert!(!page.contains("<b>sam@exemple.org</b>"), "l'invitation va avec le compte");
    assert_eq!(page.matches("<b>Léa</b>").count(), 1);
    // Ajouter Sam par son compte sur le premier remplace l'invitation expirée.
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st1), ("email", "sam@exemple.org"), ("lend", "1")]);
    assert!(invites(&w, &st1).is_empty());
    assert_eq!(w.acc.db().grants_of(&st1).unwrap().len(), 2);
}

#[test]
fn appareil_deconnecte_apres_90_jours_sans_servir() {
    let w = world(true);
    let link = linked(&w);
    let st = station(37);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    let used = device(&w, "ana@exemple.org", "iPad");
    let idle = device(&w, "ana@exemple.org", "Apple TV");
    // L'iPad sert tous les mois ; l'Apple TV jamais.
    for _ in 0..3 {
        w.advance(30 * 86_400);
        assert_eq!(api(&w, false, "/v1/computers", Some(&used), Value::Null).status, 200);
    }
    w.advance(DEVICE_IDLE_SECS - 90 * 86_400 + 1);
    ana.sign_in("ana@exemple.org", PASSWORD);
    // Refusée comme un appareil déconnecté : les apps disent « reconnectez-vous ».
    let r = api(&w, false, "/v1/computers", Some(&idle), Value::Null);
    assert_eq!((r.status, r.body["error"].as_str()), (401, Some("bad_session")));
    let r = api(&w, true, "/v1/access", Some(&idle), json!({ "station": st }));
    assert_eq!((r.status, r.body["error"].as_str()), (401, Some("bad_session")));
    assert_eq!(api(&w, false, "/v1/computers", Some(&used), Value::Null).status, 200);
    // La page du compte : l'iPad, « Utilisé il y a… », et l'aide.
    let page = txt(&ana.get("/compte?lang=fr"));
    assert!(page.contains("iPad") && !page.contains("Apple TV"), "{page}");
    assert!(page.contains("Utilisé il y a") && page.contains("Un appareil qui ne sert pas pendant 90 jours est déconnecté."));
    assert!(w.acc.db().audit_of(1).unwrap().iter().any(|(k, _)| k == "appareil_expire"));
    // Le ménage efface aussi ceux qu'on ne présente plus.
    let other = device(&w, "ana@exemple.org", "Portable");
    let _ = other;
    w.advance(DEVICE_IDLE_SECS + 1);
    w.acc.db().sweep(w.now()).unwrap();
    assert!(w.acc.db().devices_of(1).unwrap().is_empty());
}
