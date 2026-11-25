//! Les parcours des lots 4 (personnes et droits, Admin sur le réseau) et 5
//! (organisations, groupes, journal, jetons d'enrôlement), sans réseau : la
//! fausse liaison du lot 2 remplace le hub.

use kybernow_tunnel::access::{Use, Verifier};
use kybernow_tunnel::msg::{ErrorCode, ErrorMsg, Msg};
use serde_json::{Value, json};

use super::lot2::{api, attach, linked, station};
use super::*;

/// Un compte vérifié et son navigateur connecté.
fn person(w: &World, email: &str, name: &str, ip: &str) -> Browser {
    w.acc.add_verified(email, name, PASSWORD).unwrap();
    let mut b = w.browser_at(ip);
    assert_eq!(location(&b.sign_in(email, PASSWORD)), "/compte/ordinateurs?lang=fr");
    b
}

/// Active la double authentification dans ce navigateur (la session reste entière).
fn with_totp(w: &World, b: &mut Browser, email: &str) {
    b.get("/compte/double-authentification?lang=fr");
    let code = w.totp_code(email, 0);
    let r = b.post("/compte/double-authentification?lang=fr", &[("code", &code)]);
    assert!(html(&r).contains("Codes de secours"));
}

/// Une session d'appareil (app) pour ce compte : son jeton porteur.
fn device(w: &World, email: &str, name: &str) -> String {
    let account = w.acc.db().account_by_email(email).unwrap().unwrap();
    w.acc.open_device(&account, name, "app").unwrap()["token"].as_str().unwrap().to_string()
}

fn computers(w: &World, token: &str) -> Value {
    let r = api(w, false, "/v1/computers", Some(token), Value::Null);
    assert_eq!(r.status, 200);
    r.body["computers"].clone()
}

#[test]
fn ajouter_des_personnes_droits_echeance_et_retrait() {
    let w = world(true);
    let link = linked(&w);
    let st = station(11);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    let _lea_browser = person(&w, "lea@exemple.org", "Léa Martin", "192.0.2.11");
    let lea = device(&w, "lea@exemple.org", "iPad de Léa");
    assert_eq!(computers(&w, &lea), json!([]));

    // « Personnes » : Ana, propriétaire ; le formulaire « Ajouter une personne ».
    let page = ana.get(&format!("/compte/ordinateur?id={st}&lang=fr"));
    let text = html(&page);
    assert!(text.contains("Personnes") && text.contains("Ana (vous)") && text.contains("Ajouter une personne") && text.contains("Envoyer l'invitation"));
    // Sans droit coché, ou une date passée : refusé, le formulaire reste.
    let r = ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "lea@exemple.org")]);
    assert!(html(&r).contains("Cochez au moins un droit") && html(&r).contains(r#"value="lea@exemple.org""#));
    let r = ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "lea@exemple.org"), ("session", "1"), ("fin", "2020-01-01")]);
    assert!(html(&r).contains("Cette date ne va pas"));
    // Léa a un compte : Session, Partage et Administrer, jusqu'au 31/12/2026.
    let r = ana.post(
        "/compte/ordinateur/ajouter?lang=fr",
        &[("id", &st), ("email", "Lea@Exemple.org"), ("session", "1"), ("share", "1"), ("admin", "1"), ("fin", "2026-11-30")],
    );
    let text = html(&r);
    assert!(text.contains("Personne ajoutée") && text.contains("Léa Martin") && text.contains("jusqu&#39;au 30/11/2026"), "{text}");
    let mail = w.mail("lea@exemple.org");
    assert_eq!(mail.subject, "Ana vous ouvre PC du salon");
    assert!(mail.text.contains("Ouvrir le bureau, Rejoindre vos partages, Administrer, jusqu'au 30/11/2026") && mail.text.contains("https://share.test/compte/ordinateurs?lang=fr"));
    // Ses ordinateurs : celui d'Ana, sans Administrer tant qu'elle n'a pas la double authentification.
    let list = computers(&w, &lea);
    assert_eq!(list[0]["station"], st.as_str());
    assert_eq!(list[0]["rights"], json!(["session", "share"]));
    assert_eq!((list[0]["mine"].clone(), list[0]["owner"]["kind"].clone(), list[0]["owner"]["name"].clone()), (json!(false), json!("person"), json!("Ana")));
    // Son premier jeton : avis au propriétaire, qui nomme Léa.
    let before = w.outbox.count_to("ana@exemple.org");
    let access = api(&w, true, "/v1/access", Some(&lea), json!({ "station": st }));
    assert_eq!(access.status, 200);
    assert_eq!(access.body["rights"], json!(["session", "share"]));
    assert_eq!(w.outbox.count_to("ana@exemple.org"), before + 1);
    let notice = w.mail("ana@exemple.org");
    assert_eq!(notice.subject, "Nouvel appareil sur PC du salon");
    assert!(notice.text.contains("avec le compte KyberNow de Léa Martin") && notice.text.contains("/compte/ordinateur?id="));
    api(&w, true, "/v1/access", Some(&lea), json!({ "station": st }));
    assert_eq!(w.outbox.count_to("ana@exemple.org"), before + 1, "une seule fois par appareil");
    // Léa voit « Ordinateurs » : celui d'Ana, ouvert pour elle ; ses droits, et l'avis pour Administrer.
    let mut lea_b = w.browser_at("192.0.2.11");
    lea_b.sign_in("lea@exemple.org", PASSWORD);
    lea_b.post("/compte/bienvenue?lang=fr", &[]);
    let page = lea_b.get("/compte/ordinateurs?lang=fr");
    assert!(html(&page).contains("Ouverts pour vous") && html(&page).contains("de Ana"));
    let page = lea_b.get(&format!("/compte/ordinateur?id={st}&lang=fr"));
    assert!(html(&page).contains("Ce que vous pouvez y faire") && html(&page).contains("ne compte qu'avec la double authentification"));
    assert!(!html(&page).contains("Ajouter une personne"), "sans Administrer effectif, elle ne gère pas");
    assert_eq!(lea_b.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "x@exemple.org"), ("session", "1")]).status, 404);
    // Avec la double authentification, Administrer compte, et elle gère les accès.
    with_totp(&w, &mut lea_b, "lea@exemple.org");
    assert_eq!(computers(&w, &lea)[0]["rights"], json!(["session", "share", "admin"]));
    assert!(html(&lea_b.get(&format!("/compte/ordinateur?id={st}&lang=fr"))).contains("Ajouter une personne"));

    // Sam n'a pas de compte : l'invitation attend son adresse vérifiée.
    let r = ana.post("/compte/ordinateur/ajouter?lang=fr", &[("id", &st), ("email", "sam@exemple.org"), ("lend", "1")]);
    assert!(html(&r).contains("Invitation envoyée") && html(&r).contains("pas encore de compte"));
    assert!(w.mail("sam@exemple.org").text.contains("https://share.test/compte/inscription?lang=fr"));
    let mut sam = w.browser_at("192.0.2.12");
    sam.post("/compte/inscription?lang=fr", &[("name", "Sam"), ("email", "sam@exemple.org"), ("password", PASSWORD), ("conditions", "1")]);
    let code: String = between(&w.mail("sam@exemple.org").text, "    ", "\n").chars().filter(char::is_ascii_digit).collect();
    sam.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", &code)]);
    let sam_token = device(&w, "sam@exemple.org", "Pixel");
    assert_eq!(computers(&w, &sam_token)[0]["rights"], json!(["lend"]), "l'ordinateur l'attendait");
    // L'échéance passée, Léa ne l'a plus.
    w.advance(8 * 86_400);
    assert_eq!(computers(&w, &lea), json!([]));
    assert_eq!(api(&w, true, "/v1/access", Some(&lea), json!({ "station": st })).status, 404);
    // Retirer Sam : plus rien.
    let grant = w.acc.db().grants_of(&st).unwrap().into_iter().find(|g| matches!(g.subject, crate::accounts::people::Subject::Person { ref email, .. } if email == "sam@exemple.org")).unwrap();
    let r = ana.post("/compte/ordinateur/retirer?lang=fr", &[("id", &st), ("acces", &grant.id.to_string())]);
    assert!(html(&r).contains("Accès retiré"));
    assert_eq!(computers(&w, &sam_token), json!([]));
    // Un autre compte ne voit pas la page d'un ordinateur qui ne lui est pas ouvert.
    assert_eq!(sam.get(&format!("/compte/ordinateur?id={st}&lang=fr")).status, 404);
}

#[test]
fn organisation_membres_groupes_ordinateurs_et_journal() {
    let w = world(true);
    let link = linked(&w);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    // Sans double authentification : pas d'organisation.
    let r = ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Les Lignes Claires")]);
    assert!(html(&r).contains("Activez d'abord la double authentification"));
    with_totp(&w, &mut ana, "ana@exemple.org");
    let r = ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Les Lignes Claires")]);
    assert!(html(&r).contains("vous en êtes propriétaire") && html(&r).contains("Vous êtes propriétaire"));
    let org = w.acc.db().orgs_of(1).unwrap()[0].0.clone();
    let id = org.public_id.clone();
    // Inviter : Hugo a un compte (membre tout de suite), Paul non (invitation en attente).
    let mut hugo_b = person(&w, "hugo@exemple.org", "Hugo", "192.0.2.13");
    let r = ana.post("/compte/organisation/inviter?lang=fr", &[("id", &id), ("email", "hugo@exemple.org"), ("role", "member")]);
    assert!(html(&r).contains("Membre ajouté"));
    assert_eq!(w.mail("hugo@exemple.org").subject, "Ana vous invite dans Les Lignes Claires");
    let r = ana.post("/compte/organisation/inviter?lang=fr", &[("id", &id), ("email", "paul@exemple.org"), ("role", "member")]);
    assert!(html(&r).contains("paul@exemple.org") && html(&r).contains("Invitation envoyée"));
    // Un groupe, Hugo dedans.
    let r = ana.post("/compte/organisation/groupes/creer?lang=fr", &[("id", &id), ("nom", "Graphistes")]);
    assert!(html(&r).contains("Groupe créé"));
    let grp = w.acc.db().groups(org.id).unwrap()[0].id;
    let hugo_id = w.acc.db().account_by_email("hugo@exemple.org").unwrap().unwrap().id;
    let r = ana.post(
        "/compte/organisation/groupes/membre?lang=fr",
        &[("id", &id), ("groupe", &grp.to_string()), ("compte", &hugo_id.to_string()), ("action", "ajouter")],
    );
    assert!(html(&r).contains("Groupe changé"));
    // Rattacher une Station à l'organisation : le choix du propriétaire.
    let st = station(21);
    link.online.lock().unwrap().insert(st.clone());
    let (code, _, _) = w.acc.attach_request(&st, "Station de rendu");
    let page = ana.get(&format!("/compte/rattacher?code={code}&lang=fr"));
    assert!(html(&page).contains(&format!(r#"name="proprietaire" value="{id}""#)) && html(&page).contains("Les Lignes Claires"));
    let r = ana.post("/compte/rattacher?lang=fr", &[("code", &code), ("action", "rattacher"), ("proprietaire", &id)]);
    assert!(html(&r).contains("rattaché à l'organisation Les Lignes Claires"));
    // « Personnes » : l'ordinateur de l'équipe, qu'Ana gère, a son bouton « Ajouter une personne ».
    let page = ana.get("/compte/personnes?lang=fr");
    assert!(html(&page).contains("#ajouter") && html(&page).contains("Ajouter une personne") && !html(&page).contains("rattachez-le d'abord"));
    // Groupes : le panneau de chaque groupe, et ses formulaires qui y reviennent.
    let page = ana.get(&format!("/compte/organisation?id={id}&vue=groupes&lang=fr"));
    assert!(html(&page).contains("id=\"groupe-") && html(&page).contains("/compte/organisation/groupes/membre?lang=fr#groupe-"));
    let Some((_, Msg::Attached(a))) = link.last() else { panic!("attached attendu") };
    assert_eq!((a.owner.kind.as_str(), a.owner.id.as_str(), a.owner.email.as_deref()), ("organization", id.as_str(), None));
    let row = w.acc.db().station(&st).unwrap().unwrap();
    assert_eq!((row.account, row.org), (None, Some(org.id)));
    // Hugo, membre : rien tant que le groupe n'a pas de droit ; puis Session.
    let hugo = device(&w, "hugo@exemple.org", "iPad de Hugo");
    assert_eq!(computers(&w, &hugo), json!([]));
    let r = ana.post("/compte/ordinateur/ouvrir?lang=fr", &[("id", &st), ("a", &format!("{id}:{grp}")), ("session", "1")]);
    assert!(html(&r).contains("Accès enregistré") && html(&r).contains("Les Lignes Claires › Graphistes"));
    let list = computers(&w, &hugo);
    assert_eq!(list[0]["rights"], json!(["session"]));
    assert_eq!((list[0]["owner"]["kind"].clone(), list[0]["owner"]["id"].clone()), (json!("organization"), json!(id)));
    // Son premier accès : avis aux Propriétaires et Administrateurs, et le journal (qui, quoi, d'où).
    let before = w.outbox.count_to("ana@exemple.org");
    assert_eq!(api(&w, true, "/v1/access", Some(&hugo), json!({ "station": st })).status, 200);
    assert_eq!(w.outbox.count_to("ana@exemple.org"), before + 1);
    assert!(w.mail("ana@exemple.org").text.contains("avec le compte KyberNow de Hugo"));
    let log = w.acc.db().org_log_of(org.id, 50).unwrap();
    let kinds: Vec<&str> = log.iter().map(|l| l.kind.as_str()).collect();
    for k in ["organisation_creee", "membre_ajoute", "membre_invite", "groupe_cree", "groupe_membre_ajoute", "station_rattachee", "droits_donnes", "acces", "nouvel_appareil"] {
        assert!(kinds.contains(&k), "{k} absent du journal : {kinds:?}");
    }
    assert_eq!(log.iter().find(|l| l.kind == "acces").unwrap().ip.as_deref(), Some("192.0.2.10"));
    let page = ana.get(&format!("/compte/organisation?id={id}&vue=journal&lang=fr"));
    assert!(html(&page).contains("Hugo a ouvert Station de rendu (iPad de Hugo)"));
    // Hugo, membre, ne gère pas : ni journal, ni invitation, ni page des jetons.
    let page = hugo_b.get(&format!("/compte/organisation?id={id}&lang=fr"));
    assert!(html(&page).contains("Vous êtes membre") && !html(&page).contains("Inviter un membre"));
    let r = hugo_b.post("/compte/organisation/inviter?lang=fr", &[("id", &id), ("email", "x@exemple.org"), ("role", "member")]);
    assert!(html(&r).contains("Vous ne pouvez pas faire cela ici"));
    // Hugo devient Administrateur, mais sans double authentification il ne gère pas encore.
    let r = ana.post("/compte/organisation/role?lang=fr", &[("id", &id), ("compte", &hugo_id.to_string()), ("role", "admin")]);
    assert!(html(&r).contains("Rôle changé"));
    let r = hugo_b.post("/compte/organisation/groupes/creer?lang=fr", &[("id", &id), ("nom", "Son")]);
    assert!(html(&r).contains("Activez d'abord la double authentification"));
    assert_eq!(computers(&w, &hugo)[0]["rights"], json!(["session"]), "sans double authentification : les droits donnés seulement");
    with_totp(&w, &mut hugo_b, "hugo@exemple.org");
    assert_eq!(computers(&w, &hugo)[0]["rights"], json!(["session", "lend", "share", "admin"]));
    // Un Administrateur ne touche pas au Propriétaire ; le dernier Propriétaire reste.
    let r = hugo_b.post("/compte/organisation/role?lang=fr", &[("id", &id), ("compte", "1"), ("role", "member")]);
    assert!(html(&r).contains("Vous ne pouvez pas faire cela ici"));
    let r = ana.post("/compte/organisation/role?lang=fr", &[("id", &id), ("compte", "1"), ("role", "member")]);
    assert!(html(&r).contains("au moins un propriétaire"));
    let r = ana.post("/compte/organisation/quitter?lang=fr", &[("id", &id)]);
    assert!(html(&r).contains("au moins un propriétaire"));
    // Paul s'inscrit : il devient membre.
    let mut paul = w.browser_at("192.0.2.14");
    paul.post("/compte/inscription?lang=fr", &[("name", "Paul"), ("email", "paul@exemple.org"), ("password", PASSWORD), ("conditions", "1")]);
    let code: String = between(&w.mail("paul@exemple.org").text, "    ", "\n").chars().filter(char::is_ascii_digit).collect();
    paul.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", &code)]);
    assert!(html(&paul.get(&format!("/compte/organisation?id={id}&lang=fr"))).contains("Vous êtes membre"));
    // Retirer Hugo : il perd tout.
    let r = ana.post("/compte/organisation/retirer?lang=fr", &[("id", &id), ("compte", &hugo_id.to_string())]);
    assert!(html(&r).contains("Membre retiré"));
    assert_eq!(computers(&w, &hugo), json!([]));
    // « Ordinateurs » d'Ana : la section de l'organisation.
    ana.post("/compte/bienvenue?lang=fr", &[]);
    let page = ana.get("/compte/ordinateurs?lang=fr");
    assert!(html(&page).contains("Les Lignes Claires") && html(&page).contains("Station de rendu"));
    // Détacher depuis la console : la Station le sait, le journal aussi.
    let r = ana.post("/compte/ordinateurs/detacher?lang=fr", &[("id", &st), ("depuis", "console")]);
    assert!(html(&r).contains("Ordinateur détaché"));
    assert!(matches!(link.last(), Some((_, Msg::Detached { req: None }))));
    assert!(w.acc.db().org_log_of(org.id, 5).unwrap().iter().any(|l| l.kind == "station_detachee"));
}

#[test]
fn ouvrir_un_ordinateur_personnel_a_une_organisation() {
    let w = world(true);
    let link = linked(&w);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    with_totp(&w, &mut ana, "ana@exemple.org");
    ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Atelier")]);
    let org = w.acc.db().orgs_of(1).unwrap()[0].0.clone();
    let _ines = person(&w, "ines@exemple.org", "Inès", "192.0.2.15");
    ana.post("/compte/organisation/inviter?lang=fr", &[("id", &org.public_id), ("email", "ines@exemple.org"), ("role", "member")]);
    let st = station(31);
    link.online.lock().unwrap().insert(st.clone());
    attach(&w, &mut ana, &st);
    let ines = device(&w, "ines@exemple.org", "Pixel 9");
    assert_eq!(computers(&w, &ines), json!([]));
    // Le PC d'Ana, ouvert à toute l'organisation : Prêt.
    let page = ana.get(&format!("/compte/ordinateur?id={st}&lang=fr"));
    assert!(html(&page).contains("Ouvrir à une famille ou une équipe") && html(&page).contains("Atelier (toute l&#39;équipe)"));
    let r = ana.post("/compte/ordinateur/ouvrir?lang=fr", &[("id", &st), ("a", &org.public_id), ("lend", "1")]);
    assert!(html(&r).contains("Accès enregistré"));
    let list = computers(&w, &ines);
    assert_eq!((list[0]["rights"].clone(), list[0]["owner"]["kind"].clone()), (json!(["lend"]), json!("person")));
    // Une organisation dont Ana n'est pas membre : refusé.
    let mut bob = person(&w, "bob@exemple.org", "Bob", "192.0.2.16");
    with_totp(&w, &mut bob, "bob@exemple.org");
    bob.post("/compte/organisations/creer?lang=fr", &[("nom", "Ailleurs")]);
    let bob_id = w.acc.db().account_by_email("bob@exemple.org").unwrap().unwrap().id;
    let other = w.acc.db().orgs_of(bob_id).unwrap()[0].0.clone();
    let r = ana.post("/compte/ordinateur/ouvrir?lang=fr", &[("id", &st), ("a", &other.public_id), ("session", "1")]);
    assert!(html(&r).contains("Vous ne pouvez pas faire cela ici"));
}

#[test]
fn jetons_d_enrolement_par_la_connexion_de_controle() {
    let w = world(true);
    let _link = linked(&w);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    with_totp(&w, &mut ana, "ana@exemple.org");
    ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Les Lignes Claires")]);
    let org = w.acc.db().orgs_of(1).unwrap()[0].0.clone();
    let id = org.public_id.clone();
    ana.post("/compte/organisation/groupes/creer?lang=fr", &[("id", &id), ("nom", "Graphistes")]);
    let grp = w.acc.db().groups(org.id).unwrap()[0].id;
    // « Déployer des Stations » : un jeton pour 2 Stations, 30 jours, Session au groupe Graphistes.
    let page = ana.get(&format!("/compte/organisation/deployer?id={id}&lang=fr"));
    assert!(html(&page).contains("Déployer des Stations") && html(&page).contains("Nouveau jeton"));
    let r = ana.post(
        "/compte/organisation/jetons/creer?lang=fr",
        &[("id", &id), ("nom", "Postes graphistes"), ("jours", "30"), ("nombre", "2"), ("groupe", &grp.to_string()), ("session", "1")],
    );
    let text = html(&r).to_string();
    assert!(text.contains("Montré une seule fois") && text.contains("/quiet /enroll=kne_") && text.contains("/relay=share.test"));
    let token = between(&text, "<p class=\"token\">", "<").to_string();
    assert!(token.starts_with("kne_"));
    // Montré une seule fois : la page suivante ne l'a plus.
    assert!(!html(&ana.get(&format!("/compte/organisation/deployer?id={id}&lang=fr"))).contains(&token));
    // La Station l'envoie par sa connexion de contrôle : rattachée à l'organisation.
    let st1 = station(41);
    let Msg::Attached(a) = w.acc.enroll(&st1, &token, "GRAPH-01", 7) else { panic!("attached attendu") };
    assert_eq!((a.req, a.owner.kind.as_str(), a.owner.id.as_str()), (Some(7), "organization", id.as_str()));
    assert_eq!(a.keys, w.acc.stations.keys.key_set().keys);
    let row = w.acc.db().station(&st1).unwrap().unwrap();
    assert_eq!((row.name.as_str(), row.org), ("GRAPH-01", Some(org.id)));
    assert!(w.acc.db().grants_of(&st1).unwrap().iter().any(|g| matches!(g.subject, crate::accounts::people::Subject::Group { .. })));
    // De nouveau (Admin redémarré) : la même réponse, sans compter une Station de plus.
    assert!(matches!(w.acc.enroll(&st1, &token, "GRAPH-01", 8), Msg::Attached(_)));
    assert!(matches!(w.acc.enroll(&station(42), &token, "GRAPH-02", 9), Msg::Attached(_)));
    let refused = |m: Msg| match m {
        Msg::Error(ErrorMsg { code: ErrorCode::EnrollRefused, message, req }) => (req, message.unwrap_or_default()),
        other => panic!("refus attendu : {other:?}"),
    };
    assert_eq!(refused(w.acc.enroll(&station(43), &token, "GRAPH-03", 10)), (Some(10), "used_up".into()));
    assert_eq!(refused(w.acc.enroll(&station(43), "kne_inconnu", "X", 11)).1, "unknown");
    assert_eq!(refused(w.acc.enroll(&station(43), "pas-un-jeton", "X", 12)).1, "unknown");
    let page = ana.get(&format!("/compte/organisation/deployer?id={id}&lang=fr"));
    assert!(html(&page).contains("2 / 2 Stations") && html(&page).contains("GRAPH-01") && html(&page).contains("Épuisé"));
    // Révoquer ; les Stations enrôlées restent à l'organisation.
    let tid = w.acc.db().enroll_tokens(org.id).unwrap()[0].id;
    let r = ana.post("/compte/organisation/jetons/revoquer?lang=fr", &[("id", &id), ("jeton", &tid.to_string())]);
    assert!(html(&r).contains("Jeton révoqué"));
    assert!(w.acc.db().station(&st1).unwrap().is_some());
    // Un jeton échu.
    ana.post("/compte/organisation/jetons/creer?lang=fr", &[("id", &id), ("nom", "Court"), ("jours", "1"), ("nombre", "5"), ("groupe", "")]);
    let page = html(&ana.get(&format!("/compte/organisation/deployer?id={id}&lang=fr"))).to_string();
    assert!(!page.contains("kne_") || page.contains("kne_") && !page.contains("<p class=\"token\">"));
    let short = w.acc.db().enroll_tokens(org.id).unwrap().into_iter().find(|t| t.name == "Court").unwrap();
    assert!(short.rights.is_empty() && short.grp.is_none());
    w.advance(2 * 86_400);
    assert_eq!(short.refusal(w.now()), Some("expired"));
    // Le journal.
    let kinds: Vec<String> = w.acc.db().org_log_of(org.id, 50).unwrap().into_iter().map(|l| l.kind).collect();
    for k in ["jeton_cree", "station_enrolee", "jeton_revoque"] {
        assert!(kinds.iter().any(|x| x == k), "{k}");
    }
}

#[test]
fn ouvrir_admin_sur_le_reseau_avec_le_droit_administrer() {
    let w = world(true);
    let link = linked(&w);
    let st = station(51);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    let nonce = "abcdefghijklmnopqrstuvwxyz";
    let url = format!("/compte/ouvrir-admin?station={st}&nonce={nonce}&retour=https%3A%2F%2F192.168.1.20%3A8443&lang=fr");
    // Sans double authentification : le droit Administrer ne compte pas.
    let r = ana.get(&url);
    assert_eq!(r.status, 403);
    assert!(html(&r).contains("double authentification"));
    with_totp(&w, &mut ana, "ana@exemple.org");
    let page = ana.get(&url);
    assert!(html(&page).contains("Ouvrir Admin de PC du salon") && html(&page).contains("192.168.1.20:8443"));
    // Une adresse hors du réseau local : refusée.
    let bad = format!("/compte/ouvrir-admin?station={st}&nonce={nonce}&retour=https%3A%2F%2Fexemple.org&lang=fr");
    assert_eq!(ana.get(&bad).status, 400);
    // Annuler : rien.
    let r = ana.post("/compte/ouvrir-admin?lang=fr", &[("station", &st), ("nonce", nonce), ("retour", "https://192.168.1.20:8443"), ("action", "annuler")]);
    assert!(!html(&r).contains("api/login/kybernow"));
    // Ouvrir : vers Admin, avec un jeton dont le `jti` est le `nonce`, droit Administrer compris.
    let r = ana.post("/compte/ouvrir-admin?lang=fr", &[("station", &st), ("nonce", nonce), ("retour", "https://192.168.1.20:8443"), ("action", "ouvrir")]);
    let text = html(&r);
    let target = between(text, "content=\"0;url=", "\"").replace("&amp;", "&");
    assert!(target.starts_with("https://192.168.1.20:8443/api/login/kybernow?token="), "{target}");
    let token = target.split("token=").nth(1).unwrap();
    let verifier = Verifier::new("share.test", &st, &w.acc.stations.keys.key_set().keys);
    let claims = verifier.verify(token, w.now() as u64, Use::Once).unwrap();
    assert_eq!(claims.jti, nonce);
    assert!(claims.rights.has(kybernow_tunnel::access::Right::Admin));
    assert_eq!(claims.device, "web");
    // Un autre compte, sans droit : refusé.
    let mut bob = person(&w, "bob@exemple.org", "Bob", "192.0.2.16");
    assert_eq!(bob.get(&url).status, 400);
    assert_eq!(bob.get(&format!("/compte/admin?id={st}&lang=fr")).status, 400);
    // « Ouvrir Admin » de la console (docs/comptes.md, « Admin depuis KyberNow Share ») :
    // sans nouvelles de la Station, hors ligne ; Admin à distance coupé, l'adresse sur le réseau.
    assert_eq!(ana.get(&format!("/compte/admin?id={st}&lang=fr")).status, 400);
    link.admin.lock().unwrap().insert(st.clone(), (vec!["https://192.168.1.20:8443".into()], false));
    let page = ana.get(&format!("/compte/admin?id={st}&lang=fr"));
    assert!(html(&page).contains("Admin à distance est coupé") && html(&page).contains("https://192.168.1.20:8443/api/login/kybernow/start?lang=fr"));
    let remote = format!("https://{st}.share.test");
    let remote_url = format!("/compte/ouvrir-admin?station={st}&nonce={nonce}&retour={}&lang=fr", remote.replace(':', "%3A").replace('/', "%2F"));
    assert_eq!(ana.get(&remote_url).status, 400, "Admin à distance coupé : refusé");
    // Allumé : la console mène au départ d'Admin par le relais.
    link.admin.lock().unwrap().insert(st.clone(), (vec!["https://192.168.1.20:8443".into()], true));
    assert_eq!(location(&ana.get(&format!("/compte/admin?id={st}&lang=fr"))), format!("{remote}/api/login/kybernow/start?lang=fr"));
    // À distance : sans passkey, pas de confirmation ; la demande d'ouvrir est refusée.
    let page = ana.get(&remote_url);
    assert!(html(&page).contains("De n'importe où") && html(&page).contains("Ajoutez d&#39;abord une passkey") || html(&page).contains("Ajoutez d'abord une passkey"));
    let r = ana.post("/compte/ouvrir-admin?lang=fr", &[("station", &st), ("nonce", nonce), ("retour", &remote), ("action", "ouvrir")]);
    assert_eq!(r.status, 403);
    assert!(!html(&r).contains("api/login/kybernow?token="));
    // Avec une passkey : confirmée, vers Admin par le relais, avec un jeton `via: "relay"`.
    let mut auth = Authenticator::new(ED25519, "https://share.test");
    assert!(html(&ana.add_passkey(&auth)).contains("Passkey ajoutée"));
    let extra = [("station", st.as_str()), ("nonce", nonce), ("retour", remote.as_str()), ("action", "ouvrir")];
    let r = ana.passkey(&remote_url, "/compte/ouvrir-admin?lang=fr", &mut auth, &extra);
    let target = between(html(&r), "content=\"0;url=", "\"").replace("&amp;", "&");
    assert!(target.starts_with(&format!("{remote}/api/login/kybernow?token=")), "{target}");
    // Un vérificateur neuf : le même `nonce` a déjà servi au jeton du réseau.
    let fresh = Verifier::new("share.test", &st, &w.acc.stations.keys.key_set().keys);
    let claims = fresh.verify(target.split("token=").nth(1).unwrap(), w.now() as u64, Use::Once).unwrap();
    assert_eq!(claims.via.as_deref(), Some("relay"));
    assert_eq!(claims.jti, nonce);
    // L'adresse d'une autre Station par le relais : refusée.
    let other = format!("https://{}.share.test", station(52));
    assert_eq!(ana.post("/compte/ouvrir-admin?lang=fr", &[("station", &st), ("nonce", nonce), ("retour", &other), ("action", "ouvrir")]).status, 400);
}
