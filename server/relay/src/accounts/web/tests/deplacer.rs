//! Les réglages synchronisés et « Déplacer un ordinateur » (docs/comptes.md,
//! « Réglages synchronisés et déplacer un ordinateur »), sans réseau : la
//! fausse liaison du lot 2 remplace le hub.

use kybernow_tunnel::msg::Msg;
use serde_json::{Value, json};

use super::lot2::{api, api_with, attach, linked, station};
use super::lot45::{computers, device, person, with_totp};
use super::*;
use crate::accounts::devices::{ApiMethod, ApiReply};
use crate::accounts::orgs::Org;

/// La page, les apostrophes rendues (`&#39;` → `'`), les espaces fines (`\u{202F}`) en espaces.
fn txt(r: &Reply) -> String {
    html(r).replace("&#39;", "'").replace('\u{202F}', " ")
}

fn get_settings(w: &World, token: &str) -> ApiReply {
    api(w, false, "/v1/settings", Some(token), Value::Null)
}

fn put_settings(w: &World, token: &str, st: &str, body: Value) -> ApiReply {
    api_with(w, ApiMethod::Put, &format!("/v1/settings/{st}"), Some(token), body)
}

/// Les stations des réglages gardés.
fn synced(w: &World, token: &str) -> Vec<String> {
    let r = get_settings(w, token);
    assert_eq!(r.status, 200, "{:?}", r.body);
    r.body["settings"].as_array().unwrap().iter().map(|s| s["station"].as_str().unwrap().to_string()).collect()
}

/// Crée une famille ou une équipe (il faut la double authentification) : la dernière créée par ce compte.
fn new_org(w: &World, b: &mut Browser, email: &str, name: &str, genre: &str) -> Org {
    let r = b.post("/compte/organisations/creer?lang=fr", &[("nom", name), ("genre", genre)]);
    assert!(txt(&r).contains("vous en êtes propriétaire"), "{}", txt(&r));
    let id = w.acc.db().account_by_email(email).unwrap().unwrap().id;
    w.acc.db().orgs_of(id).unwrap().into_iter().map(|(o, _)| o).find(|o| o.name == name).unwrap()
}

#[test]
fn reglages_synchronises_droits_limites_et_menage() {
    let w = world(true);
    let link = linked(&w);
    let st = station(91);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    let _lea_b = person(&w, "lea@exemple.org", "Léa", "192.0.2.11");
    let ana_t = device(&w, "ana@exemple.org", "Mac d'Ana");
    let lea_t = device(&w, "lea@exemple.org", "iPad de Léa");

    // Sans session d'appareil : refusé.
    assert_eq!(api(&w, false, "/v1/settings", None, Value::Null).status, 401);
    assert_eq!(put_settings(&w, "pas-un-jeton", &st, json!({ "values": {} })).status, 401);
    // Rien au départ ; puis Ana garde ses réglages (une clé inconnue reste).
    assert_eq!(get_settings(&w, &ana_t).body, json!({ "settings": [] }));
    let values = json!({ "profile": null, "codec": "av1", "yuv444": true, "mbps": 80, "hdr": false, "immersive": "always", "futur": { "x": 1 } });
    let r = put_settings(&w, &ana_t, &st, json!({ "values": values }));
    assert_eq!((r.status, r.body.clone()), (200, json!({ "updated_at": w.now() })));
    let r = get_settings(&w, &ana_t);
    assert_eq!(r.body, json!({ "settings": [{ "station": st, "updated_at": w.now(), "values": values }] }));
    // Le dernier l'emporte.
    w.advance(5);
    put_settings(&w, &ana_t, &st, json!({ "values": { "codec": "hevc" } }));
    assert_eq!(get_settings(&w, &ana_t).body["settings"][0], json!({ "station": st, "updated_at": w.now(), "values": { "codec": "hevc" } }));

    // Refus : un ordinateur sans droit (ou inconnu), des valeurs qui ne sont pas un objet, trop grandes, la mauvaise méthode.
    let other = station(92);
    assert_eq!(put_settings(&w, &lea_t, &st, json!({ "values": {} })).body, json!({ "error": "unknown_station" }));
    assert_eq!(put_settings(&w, &ana_t, &other, json!({ "values": {} })).status, 404);
    assert_eq!(put_settings(&w, &ana_t, "pas-une-station", json!({ "values": {} })).status, 404);
    for bad in [json!({ "values": [1] }), json!({ "values": "x" }), json!({}), Value::Null] {
        let r = put_settings(&w, &ana_t, &st, bad);
        assert_eq!((r.status, r.body), (400, json!({ "error": "bad_values" })));
    }
    let big = json!({ "values": { "x": "a".repeat(4090) } });
    assert_eq!(put_settings(&w, &ana_t, &st, big).body, json!({ "error": "too_large" }));
    let fits = json!({ "values": { "x": "a".repeat(4096 - 8) } });
    assert_eq!(put_settings(&w, &ana_t, &st, fits).status, 200, "4096 octets tout juste");
    assert_eq!(api(&w, true, "/v1/settings", Some(&ana_t), json!({})).status, 405);
    assert_eq!(api(&w, false, &format!("/v1/settings/{st}"), Some(&ana_t), Value::Null).status, 405);
    assert_eq!(api_with(&w, ApiMethod::Put, "/v1/computers", Some(&ana_t), Value::Null).status, 405);

    // Léa reçoit Ouvrir le bureau : elle garde ses réglages ; son accès retiré, ils partent à sa lecture suivante.
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "lea@exemple.org"), ("session", "1")]);
    w.accept_invites("lea@exemple.org");
    assert_eq!(put_settings(&w, &lea_t, &st, json!({ "values": { "hdr": true } })).status, 200);
    assert_eq!(synced(&w, &lea_t), vec![st.clone()]);
    let grant = w.acc.db().grants_of(&st).unwrap()[0].id.to_string();
    ana.post("/compte/ordinateur/retirer?lang=fr", &[("id", &st), ("acces", &grant)]);
    let lea_id = w.acc.db().account_by_email("lea@exemple.org").unwrap().unwrap().id;
    assert_eq!(w.acc.db().settings_of(lea_id).unwrap().len(), 1, "gardée jusqu'à la lecture ou au ménage");
    assert!(synced(&w, &lea_t).is_empty());
    assert!(w.acc.db().settings_of(lea_id).unwrap().is_empty(), "effacée à la lecture");
    // De même au ménage périodique.
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "lea@exemple.org"), ("session", "1")]);
    w.accept_invites("lea@exemple.org");
    put_settings(&w, &lea_t, &st, json!({ "values": { "hdr": true } }));
    let grant = w.acc.db().grants_of(&st).unwrap()[0].id.to_string();
    ana.post("/compte/ordinateur/retirer?lang=fr", &[("id", &st), ("acces", &grant)]);
    w.acc.sweep();
    assert!(w.acc.db().settings_of(lea_id).unwrap().is_empty(), "effacée au ménage");

    // Détaché : les réglages d'Ana partent.
    assert_eq!(synced(&w, &ana_t), vec![st.clone()]);
    ana.post("/compte/ordinateurs/detacher?lang=fr", &[("id", &st)]);
    let ana_id = w.acc.db().account_by_email("ana@exemple.org").unwrap().unwrap().id;
    assert!(w.acc.db().settings_of(ana_id).unwrap().is_empty());
}

#[test]
fn deplacer_d_une_personne_a_une_equipe_puis_a_une_famille_puis_a_soi() {
    let w = world(true);
    let link = linked(&w);
    let st = station(93);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    with_totp(&w, &mut ana, "ana@exemple.org");
    let team = new_org(&w, &mut ana, "ana@exemple.org", "L'atelier", "equipe");
    let family = new_org(&w, &mut ana, "ana@exemple.org", "Famille Martin", "famille");
    let _lea_b = person(&w, "lea@exemple.org", "Léa", "192.0.2.11");
    let lea_t = device(&w, "lea@exemple.org", "iPad de Léa");
    let ana_t = device(&w, "ana@exemple.org", "Mac d'Ana");
    // Léa a Ouvrir le bureau, la famille aussi, un lien attend.
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "lea@exemple.org"), ("session", "1")]);
    w.accept_invites("lea@exemple.org");
    ana.post("/compte/ordinateur/ouvrir?lang=fr", &[("id", &st), ("a", &family.public_id), ("session", "1")]);
    ana.post("/compte/ordinateur/liens/creer?lang=fr", &[("id", &st), ("jours", "7"), ("session", "1")]);
    put_settings(&w, &lea_t, &st, json!({ "values": { "mbps": 30 } }));
    put_settings(&w, &ana_t, &st, json!({ "values": { "mbps": 150 } }));

    // La page de l'ordinateur : « Déplacer… », vers l'équipe ou la famille (pas « Moi » : il est à elle).
    let page = txt(&ana.get(&format!("/compte/ordinateur?id={st}&lang=fr")));
    assert!(page.contains("Déplacer…") && page.contains("href=\"#deplacer\"") && page.contains("id=\"deplacer\""));
    assert!(page.contains(&format!("name=\"vers\" value=\"{}\"", team.public_id)) && page.contains(&format!("name=\"vers\" value=\"{}\"", family.public_id)));
    assert!(!page.contains("name=\"vers\" value=\"moi\""));
    // « Ce qui change », vers l'équipe.
    let r = ana.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &st), ("vers", &team.public_id)]);
    let preview = txt(&r);
    assert!(preview.contains("Ce qui change") && preview.contains("Vers L'atelier"), "{preview}");
    assert!(preview.contains("Perdent l'accès : Famille Martin"), "{preview}");
    assert!(preview.contains("Liens en attente annulés : 1"));
    assert!(preview.contains("Garder les accès donnés à des personnes") && preview.contains("name=\"garder\" value=\"1\" checked") && preview.contains("Léa : décochée"));
    assert!(preview.contains("Déplacer PC du salon"));
    assert_eq!(w.acc.db().station(&st).unwrap().unwrap().org, None, "l'aperçu ne change rien");
    // Déplacer, la case cochée : Léa garde son accès, la famille le perd, le lien est annulé.
    let r = ana.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &st), ("vers", &team.public_id), ("garder", "1"), ("action", "deplacer")]);
    assert!(txt(&r).contains("PC du salon est maintenant à L'atelier."), "{}", txt(&r));
    let row = w.acc.db().station(&st).unwrap().unwrap();
    assert_eq!((row.owner_kind.as_str(), row.account, row.org), ("organization", None, Some(team.id)));
    assert_eq!(computers(&w, &lea_t)[0]["rights"], json!(["session"]));
    assert_eq!(computers(&w, &lea_t)[0]["owner"]["id"], json!(team.public_id));
    assert!(w.acc.db().grants_of(&st).unwrap().iter().all(|g| !matches!(g.subject, crate::accounts::people::Subject::Org { .. })));
    assert_eq!(w.acc.db().pending_links(&st, w.now()).unwrap(), 0);
    // La Station l'apprend comme un rattachement.
    let Some((to, Msg::Attached(a))) = link.last() else { panic!("attached attendu") };
    assert_eq!((to.as_str(), a.owner.kind.as_str(), a.owner.id.as_str(), a.req), (st.as_str(), "organization", team.public_id.as_str(), None));
    // Les journaux : l'équipe, et le compte d'Ana (une personne était propriétaire).
    let log = w.acc.db().org_log_of(team.id, 10).unwrap();
    let line = log.iter().find(|l| l.kind == "ordinateur_deplace").expect("journal de l'équipe");
    assert_eq!((line.detail["from"].as_str(), line.detail["to"].as_str()), (Some("Ana"), Some("L'atelier")));
    let journal = txt(&ana.get(&format!("/compte/organisation?id={}&vue=journal&lang=fr", team.public_id)));
    assert!(journal.contains("Ana a déplacé PC du salon de Ana vers L'atelier"), "{journal}");
    assert!(w.acc.db().audit_of(1).unwrap().iter().any(|(k, _)| k == "ordinateur_deplace"));
    // Les réglages : Léa et Ana y ont toujours droit, ils restent.
    assert_eq!(synced(&w, &lea_t), vec![st.clone()]);
    assert_eq!(synced(&w, &ana_t), vec![st.clone()]);

    // De l'équipe à la famille, la case décochée : Léa perd l'accès, et ses réglages.
    let page = txt(&ana.get(&format!("/compte/ordinateur?id={st}&lang=fr")));
    assert!(page.contains("name=\"vers\" value=\"moi\"") && !page.contains(&format!("name=\"vers\" value=\"{}\"", team.public_id)));
    let r = ana.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &st), ("vers", &family.public_id), ("action", "deplacer")]);
    assert!(txt(&r).contains("PC du salon est maintenant à Famille Martin."));
    assert_eq!(computers(&w, &lea_t), json!([]));
    let lea_id = w.acc.db().account_by_email("lea@exemple.org").unwrap().unwrap().id;
    assert!(w.acc.db().settings_of(lea_id).unwrap().is_empty(), "plus de droit : ses réglages partent");
    assert_eq!(synced(&w, &ana_t), vec![st.clone()], "Ana gère la famille : les siens restent");
    for org in [team.id, family.id] {
        assert!(w.acc.db().org_log_of(org, 10).unwrap().iter().any(|l| l.kind == "ordinateur_deplace" && l.detail["to"] == "Famille Martin"), "les deux journaux");
    }

    // De la famille à soi.
    let r = ana.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &st), ("vers", "moi")]);
    assert!(txt(&r).contains("Vers vous") && txt(&r).contains("Personne d'autre ne perd l'accès"));
    let r = ana.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &st), ("vers", "moi"), ("garder", "1"), ("action", "deplacer")]);
    assert!(txt(&r).contains("PC du salon est maintenant à vous."));
    let row = w.acc.db().station(&st).unwrap().unwrap();
    assert_eq!((row.owner_kind.as_str(), row.account, row.org), ("person", Some(1), None));
    let Some((_, Msg::Attached(a))) = link.last() else { panic!("attached attendu") };
    assert_eq!((a.owner.kind.as_str(), a.owner.email.as_deref()), ("person", Some("ana@exemple.org")));
    // Déjà à elle : « Moi » est refusé.
    let r = ana.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &st), ("vers", "moi"), ("action", "deplacer")]);
    assert!(txt(&r).contains("Vous ne pouvez pas faire cela ici"));
}

#[test]
fn deplacer_refuse_sans_gerer_la_destination_sans_double_authentification_ou_sans_etre_proprietaire() {
    let w = world(true);
    let link = linked(&w);
    let (st, stb) = (station(94), station(95));
    for s in [&st, &stb] {
        link.online.lock().unwrap().insert(s.clone());
    }
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    with_totp(&w, &mut ana, "ana@exemple.org");
    let team = new_org(&w, &mut ana, "ana@exemple.org", "L'atelier", "equipe");
    // Paul a son équipe ; Ana n'y est que membre.
    let mut paul = person(&w, "paul@exemple.org", "Paul", "192.0.2.12");
    with_totp(&w, &mut paul, "paul@exemple.org");
    let other = new_org(&w, &mut paul, "paul@exemple.org", "Studio <Paul>", "equipe");
    paul.post("/compte/organisation/inviter?lang=fr", &[("id", &other.public_id), ("email", "ana@exemple.org"), ("role", "member")]);
    w.accept_invites("ana@exemple.org");
    let page = txt(&ana.get(&format!("/compte/ordinateur?id={st}&lang=fr")));
    assert!(!page.contains(&format!("name=\"vers\" value=\"{}\"", other.public_id)), "seulement celles qu'elle gère");
    let r = ana.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &st), ("vers", &other.public_id), ("action", "deplacer")]);
    assert!(txt(&r).contains("Vous ne gérez pas Studio &lt;Paul&gt;."), "{}", txt(&r));
    assert_eq!(w.acc.db().station(&st).unwrap().unwrap().account, Some(1), "pas déplacé");
    // Une organisation inconnue, ou la propriétaire actuelle : refusé.
    let r = ana.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &st), ("vers", "org_inconnue")]);
    assert!(txt(&r).contains("Vous ne pouvez pas faire cela ici"));

    // Bob, Administrateur de l'équipe sans double authentification, propriétaire de son ordinateur :
    // « Déplacer… » lui est proposé, et refusé.
    let mut bob = person(&w, "bob@exemple.org", "Bob", "192.0.2.13");
    attach_as(&w, &mut bob, &stb);
    let bob_id = w.acc.db().account_by_email("bob@exemple.org").unwrap().unwrap().id;
    ana.post("/compte/organisation/inviter?lang=fr", &[("id", &team.public_id), ("email", "bob@exemple.org"), ("role", "admin")]);
    w.accept_invites("bob@exemple.org");
    assert!(w.acc.db().org_role(team.id, bob_id).unwrap().is_some());
    let page = txt(&bob.get(&format!("/compte/ordinateur?id={stb}&lang=fr")));
    assert!(page.contains("Déplacer…") && page.contains(&format!("name=\"vers\" value=\"{}\"", team.public_id)));
    let r = bob.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &stb), ("vers", &team.public_id)]);
    assert!(txt(&r).contains("Activez d'abord la double authentification"), "{}", txt(&r));
    assert_eq!(w.acc.db().station(&stb).unwrap().unwrap().account, Some(bob_id));

    // Léa, avec Administrer et la double authentification, gère les accès sans pouvoir déplacer.
    let mut lea = person(&w, "lea@exemple.org", "Léa", "192.0.2.11");
    with_totp(&w, &mut lea, "lea@exemple.org");
    ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "lea@exemple.org"), ("session", "1"), ("admin", "1")]);
    w.accept_invites("lea@exemple.org");
    let page = txt(&lea.get(&format!("/compte/ordinateur?id={st}&lang=fr")));
    assert!(page.contains("Ajouter une personne") && !page.contains("Déplacer…"));
    let r = lea.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &st), ("vers", "moi"), ("action", "deplacer")]);
    assert!(txt(&r).contains("Vous ne pouvez pas faire cela ici"));
    // Sans aucun accès : comme un ordinateur inconnu.
    assert_eq!(paul.post("/compte/ordinateur/deplacer?lang=fr", &[("id", &st), ("vers", "moi")]).status, 404);
    assert_eq!(w.acc.db().station(&st).unwrap().unwrap().account, Some(1));
}

/// Rattache `st` au compte connecté dans ce navigateur (pas forcément Ana).
fn attach_as(w: &World, b: &mut Browser, st: &str) {
    let (code, _, _) = w.acc.attach_request(st, "PC de Bob");
    let r = b.post("/compte/rattacher?lang=fr", &[("code", &code), ("action", "rattacher")]);
    assert!(txt(&r).contains("Ordinateur rattaché"));
}
