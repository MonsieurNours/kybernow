//! Les parcours des liens d'accès et des groupes d'ordinateurs
//! (docs/comptes.md, « Regarder, liens d'accès et groupes d'ordinateurs »),
//! sans réseau : la fausse liaison du lot 2 remplace le hub.

use kybernow_tunnel::msg::Msg;
use serde_json::json;

use super::lot2::{attach, linked, station};
use super::lot45::{computers, device, person, with_totp};
use super::*;

/// La page, les apostrophes rendues (`&#39;` → `'`).
fn txt(r: &Reply) -> String {
    html(r).replace("&#39;", "'")
}

/// Crée un lien sur la page de l'ordinateur et le lit (montré une seule fois) : son chemin `/compte/lien/kla_…`.
fn new_link(b: &mut Browser, st: &str, form: &[(&str, &str)]) -> String {
    let mut all = vec![("id", st)];
    all.extend_from_slice(form);
    let r = b.post("/compte/ordinateur/liens/creer?lang=fr", &all);
    let text = txt(&r).to_string();
    assert!(text.contains("Nouveau lien") && text.contains("Copier le lien"), "{}", &text[text.len().saturating_sub(800)..]);
    let path = between(&text, "<p class=\"token\">https://share.test", "<").to_string();
    assert!(path.starts_with("/compte/lien/kla_") && path.len() == "/compte/lien/".len() + 36, "{path}");
    path
}

/// Inscrit une personne dans ce navigateur, jusqu'à la vérification de l'adresse : la redirection qui suit.
fn sign_up(w: &World, b: &mut Browser, name: &str, email: &str) -> String {
    let r = b.post("/compte/inscription?lang=fr", &[("name", name), ("email", email), ("password", PASSWORD), ("conditions", "1")]);
    assert_eq!(location(&r), "/compte/verifier?lang=fr");
    let code: String = between(&w.mail(email).text, "    ", "\n").chars().filter(char::is_ascii_digit).collect();
    let r = b.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", &code)]);
    location(&r).to_string()
}

#[test]
fn liens_d_acces_crees_montres_une_fois_et_utilises_une_fois() {
    let w = world(true);
    let link = linked(&w);
    let st = station(61);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);

    // La section « Liens d'accès » de la page de l'ordinateur, et son panneau.
    let page = ana.get(&format!("/compte/ordinateur?id={st}&lang=fr"));
    let text = txt(&page);
    assert!(text.contains("Liens d'accès"));
    assert!(text.contains("id=\"lien\"") && text.contains("Le lien sert pendant") && text.contains("7 jours (proposé)"));
    // Refus : sans droit, une durée inconnue, une date passée.
    let r = ana.post("/compte/ordinateur/liens/creer?lang=fr", &[("id", &st), ("jours", "7")]);
    assert!(txt(&r).contains("Cochez au moins un droit") && txt(&r).contains("ka-sheet-wrap is-open\" id=\"lien\""));
    let r = ana.post("/compte/ordinateur/liens/creer?lang=fr", &[("id", &st), ("jours", "3"), ("view", "1")]);
    assert!(txt(&r).contains("Ce nombre ne va pas"));
    let r = ana.post("/compte/ordinateur/liens/creer?lang=fr", &[("id", &st), ("jours", "7"), ("view", "1"), ("fin", "2020-01-01")]);
    assert!(txt(&r).contains("Cette date ne va pas"));

    // Un lien : Regarder et Ouvrir le bureau, 7 jours, accès jusqu'au 31/12/2026.
    let path = new_link(&mut ana, &st, &[("jours", "7"), ("view", "1"), ("session", "1"), ("fin", "2026-12-31")]);
    let token = path.trim_start_matches("/compte/lien/").to_string();
    // Montré une seule fois : la page suivante ne l'a plus, seulement son début.
    let page = txt(&ana.get(&format!("/compte/ordinateur?id={st}&lang=fr"))).to_string();
    assert!(!page.contains(&token) && page.contains(&format!("{}…", &token[..9])) && page.contains("En attente · jusqu'au 30/11/2026"), "{page}");
    // Le propriétaire qui ouvre son propre lien : il reste en attente.
    let r = ana.get(&format!("{path}?lang=fr"));
    assert!(txt(&r).contains("C'est votre ordinateur"));
    let r = ana.post(&format!("{path}?lang=fr"), &[]);
    assert!(txt(&r).contains("ce lien est pour quelqu"));
    assert_eq!(w.acc.db().pending_links(&st, w.now()).unwrap(), 1);

    // Léa n'a pas de compte : se connecter ou en créer un, puis retour au lien.
    let mut lea = w.browser_at("192.0.2.11");
    let r = lea.get(&format!("{path}?lang=fr"));
    let text = txt(&r);
    assert!(text.contains("Ana vous ouvre PC du salon") && text.contains("/compte/connexion?lang=fr") && text.contains("/compte/inscription?lang=fr"));
    assert!(!text.contains("Accepter"));
    assert_eq!(sign_up(&w, &mut lea, "Léa", "lea@exemple.org"), format!("{path}?lang=fr"), "après la vérification : le lien");
    let r = lea.get(&format!("{path}?lang=fr"));
    let text = txt(&r);
    assert!(text.contains("Ana vous ouvre PC du salon") && text.contains("Regarder") && text.contains("Ouvrir le bureau") && text.contains("Jusqu'au 31/12/2026"));
    assert!(text.contains(&format!("action=\"{path}?lang=fr\"")) && text.contains("Accepter"));
    // Bob ouvre la page en même temps ; Léa accepte d'abord.
    let mut bob = person(&w, "bob@exemple.org", "Bob", "192.0.2.12");
    assert!(txt(&bob.get(&format!("{path}?lang=fr"))).contains("Accepter"));
    let r = lea.post(&format!("{path}?lang=fr"), &[]);
    let text = txt(&r);
    assert!(text.contains("C'est fait"), "{text}");
    let lea_dev = device(&w, "lea@exemple.org", "iPad de Léa");
    let list = computers(&w, &lea_dev);
    assert_eq!((list[0]["station"].clone(), list[0]["rights"].clone()), (json!(st), json!(["view", "session"])));
    // Bob arrive trop tard : le lien a déjà servi, et ne lui donne rien.
    let r = bob.post(&format!("{path}?lang=fr"), &[]);
    assert_eq!(r.status, 410);
    assert!(txt(&r).contains("Ce lien a déjà servi."));
    assert_eq!(computers(&w, &device(&w, "bob@exemple.org", "Pixel")), json!([]));
    // La liste d'Ana : « Utilisé par Léa le … ».
    let page = txt(&ana.get(&format!("/compte/ordinateur?id={st}&lang=fr"))).to_string();
    assert!(page.contains("Utilisé par Léa le 23/11/2026"), "{page}");

    // Un second lien pour Léa : Prêt, sans échéance d'accès. Ajouté au reste ; plus d'échéance.
    let path2 = new_link(&mut ana, &st, &[("jours", "1"), ("lend", "1")]);
    lea.post(&format!("{path2}?lang=fr"), &[]);
    let lea_id = w.acc.db().account_by_email("lea@exemple.org").unwrap().unwrap().id;
    let grant = w.acc.db().grants_of(&st).unwrap().into_iter().find(|g| matches!(g.subject, crate::accounts::people::Subject::Person { account, .. } if account == lea_id)).unwrap();
    assert_eq!((grant.rights.iter().map(|r| r.as_str()).collect::<Vec<_>>(), grant.expires_at), (vec!["view", "session", "lend"], None));

    // Illisible, inconnu.
    let r = bob.get("/compte/lien/kla_nimporte?lang=fr");
    assert_eq!(r.status, 404);
    assert!(txt(&r).contains("Ce lien est illisible."));
    // Expiré : un jour, puis deux jours passent.
    let path3 = new_link(&mut ana, &st, &[("jours", "1"), ("share", "1")]);
    // Révoqué : avec la confirmation (« Révoquer ce lien ? »).
    let path4 = new_link(&mut ana, &st, &[("jours", "30"), ("share", "1")]);
    let page = txt(&ana.get(&format!("/compte/ordinateur?id={st}&lang=fr"))).to_string();
    assert!(page.contains("Révoquer ce lien\u{202F}? Il ne servira plus."));
    let id4 = w.acc.db().access_links(&st, 1).unwrap()[0].id;
    let r = ana.post("/compte/ordinateur/liens/revoquer?lang=fr", &[("id", &st), ("lien", &id4.to_string())]);
    assert!(txt(&r).contains("Lien révoqué"));
    let r = bob.get(&format!("{path4}?lang=fr"));
    assert!(txt(&r).contains("Ce lien a été révoqué."));
    w.advance(2 * 86_400);
    let r = bob.post(&format!("{path3}?lang=fr"), &[]);
    assert!(txt(&r).contains("Ce lien a expiré."));
    let page = txt(&ana.get(&format!("/compte/ordinateur?id={st}&lang=fr"))).to_string();
    assert!(page.contains("Expiré") && page.contains("Révoqué"));
    // Quelqu'un d'autre ne crée ni ne révoque de lien.
    let r = bob.post("/compte/ordinateur/liens/creer?lang=fr", &[("id", &st), ("jours", "7"), ("view", "1")]);
    assert_eq!(r.status, 404);

    // Dix liens en attente au plus.
    for _ in 0..10 {
        new_link(&mut ana, &st, &[("jours", "7"), ("view", "1")]);
    }
    let r = ana.post("/compte/ordinateur/liens/creer?lang=fr", &[("id", &st), ("jours", "7"), ("view", "1")]);
    assert!(txt(&r).contains("Révoquez un lien avant d'en créer un autre."));

    // Le journal du compte ; jamais le lien dans les journaux.
    let ana_id = w.acc.db().account_by_email("ana@exemple.org").unwrap().unwrap().id;
    let kinds: Vec<String> = w.acc.db().audit_of(ana_id).unwrap().into_iter().map(|(k, _)| k).collect();
    for k in ["lien_cree", "lien_revoque", "lien_utilise"] {
        assert!(kinds.iter().any(|x| x == k), "{k} : {kinds:?}");
    }
    assert!(w.acc.db().audit_of(lea_id).unwrap().iter().any(|(k, _)| k == "lien_accepte"));
}

#[test]
fn les_liens_inconnus_sont_limites() {
    let w = world(true);
    let mut b = w.browser_at("192.0.2.30");
    for i in 0..5 {
        let r = b.get(&format!("/compte/lien/kla_{:0>32}?lang=fr", i));
        assert_eq!(r.status, 404);
    }
    let r = b.get("/compte/lien/kla_abcdefghijklmnopqrstuvwxyz234567?lang=fr");
    assert_eq!(r.status, 429);
    assert!(txt(&r).contains("Trop d'essais"));
}

#[test]
fn un_lien_avec_administrer_demande_la_double_authentification() {
    let w = world(true);
    let link = linked(&w);
    let st = station(62);
    link.online.lock().unwrap().insert(st.clone());
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    attach(&w, &mut ana, &st);
    let path = new_link(&mut ana, &st, &[("jours", "7"), ("admin", "1")]);
    let mut lea = person(&w, "lea@exemple.org", "Léa", "192.0.2.11");
    let r = lea.get(&format!("{path}?lang=fr"));
    assert!(txt(&r).contains("Le droit Administrer ne comptera qu"));
    let r = lea.post(&format!("{path}?lang=fr"), &[]);
    assert!(txt(&r).contains("Lien accepté") && txt(&r).contains("/compte/double-authentification"));
    let lea_dev = device(&w, "lea@exemple.org", "iPad de Léa");
    assert_eq!(computers(&w, &lea_dev), json!([]));
    with_totp(&w, &mut lea, "lea@exemple.org");
    assert_eq!(computers(&w, &lea_dev)[0]["rights"], json!(["admin"]));
}

#[test]
fn groupes_d_ordinateurs_presents_et_a_venir() {
    let w = world(true);
    let link = linked(&w);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    with_totp(&w, &mut ana, "ana@exemple.org");
    ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Les Lignes Claires")]);
    let org = w.acc.db().orgs_of(1).unwrap()[0].0.clone();
    let id = org.public_id.clone();
    let mut hugo_b = person(&w, "hugo@exemple.org", "Hugo", "192.0.2.13");
    ana.post("/compte/organisation/inviter?lang=fr", &[("id", &id), ("email", "hugo@exemple.org"), ("role", "member")]);
    w.accept_invites("hugo@exemple.org");
    let _ines_b = person(&w, "ines@exemple.org", "Inès", "192.0.2.14");
    ana.post("/compte/organisation/inviter?lang=fr", &[("id", &id), ("email", "ines@exemple.org"), ("role", "member")]);
    w.accept_invites("ines@exemple.org");
    ana.post("/compte/organisation/groupes/creer?lang=fr", &[("id", &id), ("nom", "Graphistes")]);
    let people = w.acc.db().groups(org.id).unwrap()[0].id;
    let hugo_id = w.acc.db().account_by_email("hugo@exemple.org").unwrap().unwrap().id;
    let ines_id = w.acc.db().account_by_email("ines@exemple.org").unwrap().unwrap().id;
    ana.post("/compte/organisation/groupes/membre?lang=fr", &[("id", &id), ("groupe", &people.to_string()), ("compte", &hugo_id.to_string()), ("action", "ajouter")]);
    // Deux ordinateurs de l'équipe, et un à Ana.
    let (st1, st2, perso) = (station(71), station(72), station(73));
    for st in [&st1, &st2, &perso] {
        link.online.lock().unwrap().insert(st.clone());
    }
    w.acc.db().attach_station_to_org(&st1, "Poste 1", org.id, Some(1), None, w.now()).unwrap();
    w.acc.db().attach_station_to_org(&st2, "Poste 2", org.id, Some(1), None, w.now()).unwrap();
    attach(&w, &mut ana, &perso);
    let hugo = device(&w, "hugo@exemple.org", "iPad de Hugo");
    let ines = device(&w, "ines@exemple.org", "Pixel d'Inès");
    assert_eq!(computers(&w, &hugo), json!([]));

    // L'onglet « Ordinateurs » : créer un groupe d'ordinateurs.
    let page = txt(&ana.get(&format!("/compte/organisation?id={id}&vue=ordinateurs&lang=fr"))).to_string();
    assert!(page.contains("Groupes d'ordinateurs") && page.contains("id=\"groupe-ordinateurs\""));
    // Hugo, membre, ne gère pas.
    let r = hugo_b.post("/compte/organisation/groupes-ordinateurs/creer?lang=fr", &[("id", &id), ("nom", "Postes")]);
    assert!(txt(&r).contains("Vous ne pouvez pas faire cela ici"));
    let r = ana.post("/compte/organisation/groupes-ordinateurs/creer?lang=fr", &[("id", &id), ("nom", "Postes de l'atelier")]);
    assert!(txt(&r).contains("Groupe créé"));
    let r = ana.post("/compte/organisation/groupes-ordinateurs/creer?lang=fr", &[("id", &id), ("nom", "Postes de l'atelier")]);
    assert!(txt(&r).contains("Un groupe porte déjà ce nom"));
    let sg = w.acc.db().station_groups(org.id).unwrap()[0].id;
    let sg_s = sg.to_string();
    let add = |b: &mut Browser, st: &str, action: &str| b.post("/compte/organisation/groupes-ordinateurs/ordinateur?lang=fr", &[("id", &id), ("groupe", &sg_s), ("ordinateur", st), ("action", action)]);
    assert!(txt(&add(&mut ana, &st1, "ajouter")).contains("Groupe changé"));
    assert!(txt(&add(&mut ana, &perso, "ajouter")).contains("Vous ne pouvez pas faire cela ici"), "pas un ordinateur de l'équipe");
    // Le panneau du groupe : ses ordinateurs, « Ouvrir le groupe », « Supprimer le groupe » avec confirmation.
    let page = txt(&ana.get(&format!("/compte/organisation?id={id}&vue=ordinateurs&lang=fr"))).to_string();
    assert!(page.contains(&format!("id=\"ordinateurs-{sg}\"")) && page.contains("Ouvrir le groupe") && page.contains(&format!("href=\"#supprimer-ordinateurs-{sg}\"")));
    assert!(page.contains("Supprimer le groupe «\u{202F}Postes de l'atelier\u{202F}»\u{202F}?"), "confirmation");
    assert!(page.contains("/compte/organisation/groupes-ordinateurs/ouvrir?lang=fr#ordinateurs-"));

    // Ouvrir le groupe aux Graphistes (Session, jusqu'au 30/11/2026) et à toute l'équipe (Regarder).
    let open = |b: &mut Browser, a: &str, rights: &[(&str, &str)], fin: &str| {
        let mut form = vec![("id", id.as_str()), ("groupe", sg_s.as_str()), ("a", a), ("fin", fin)];
        form.extend_from_slice(rights);
        b.post("/compte/organisation/groupes-ordinateurs/ouvrir?lang=fr", &form)
    };
    assert!(txt(&open(&mut ana, &format!("grp:{people}"), &[("session", "1")], "2026-11-30")).contains("Accès enregistré"));
    assert!(txt(&open(&mut ana, "org", &[("view", "1")], "")).contains("Accès enregistré"));
    assert!(txt(&open(&mut ana, "grp:9999", &[("view", "1")], "")).contains("Vous ne pouvez pas faire cela ici"));
    assert!(txt(&open(&mut ana, &format!("grp:{people}"), &[], "")).contains("Cochez au moins un droit"));
    assert_eq!(computers(&w, &hugo).as_array().unwrap().len(), 1);
    assert_eq!(computers(&w, &hugo)[0]["rights"], json!(["view", "session"]));
    assert_eq!(computers(&w, &ines)[0]["rights"], json!(["view"]));
    // Un ordinateur ajouté ensuite : les mêmes droits ; en plus de ceux donnés ordinateur par ordinateur.
    add(&mut ana, &st2, "ajouter");
    w.acc.db().grant_person(&st2, ines_id, &kybernow_tunnel::access::Rights::new([kybernow_tunnel::access::Right::Lend]), None, 1, w.now()).unwrap();
    let list = computers(&w, &ines);
    assert_eq!(list.as_array().unwrap().len(), 2);
    let on_st2 = list.as_array().unwrap().iter().find(|c| c["station"] == json!(st2)).unwrap().clone();
    assert_eq!(on_st2["rights"], json!(["view", "lend"]));
    // « Qui y a accès » sur l'ordinateur : « Par le groupe … », sans « Retirer » pour ces lignes.
    let page = txt(&ana.get(&format!("/compte/ordinateur?id={st2}&lang=fr"))).to_string();
    assert!(page.contains("Par le groupe Postes de l'atelier"));
    assert_eq!(page.matches("name=\"acces\"").count(), 1, "seul le droit d'Inès se retire ici");
    // Un membre seul : Hugo, Prêt ; puis retiré.
    open(&mut ana, &format!("compte:{hugo_id}"), &[("lend", "1")], "");
    assert_eq!(computers(&w, &hugo)[0]["rights"], json!(["view", "session", "lend"]));
    let grant = w.acc.db().station_groups(org.id).unwrap()[0].grants.iter().find(|g| matches!(g.subject, crate::accounts::people::Subject::Person { .. })).unwrap().id;
    let r = ana.post("/compte/organisation/groupes-ordinateurs/retirer?lang=fr", &[("id", &id), ("groupe", &sg_s), ("acces", &grant.to_string())]);
    assert!(txt(&r).contains("Accès retiré"));
    assert_eq!(computers(&w, &hugo)[0]["rights"], json!(["view", "session"]));
    // L'échéance des Graphistes passée : Regarder seulement.
    w.advance(8 * 86_400);
    assert_eq!(computers(&w, &hugo)[0]["rights"], json!(["view"]));

    // Détaché puis rattaché : il a quitté ses groupes.
    let r = ana.post("/compte/ordinateurs/detacher?lang=fr", &[("id", &st2), ("depuis", "console")]);
    assert!(txt(&r).contains("Ordinateur détaché"));
    w.acc.db().attach_station_to_org(&st2, "Poste 2", org.id, Some(1), None, w.now()).unwrap();
    assert!(!computers(&w, &ines).as_array().unwrap().iter().any(|c| c["station"] == json!(st2)));

    // Déployer : un jeton qui range ses Stations dans le groupe.
    let page = txt(&ana.get(&format!("/compte/organisation/deployer?id={id}&lang=fr"))).to_string();
    assert!(page.contains("Ranger dans le groupe") && page.contains(&format!("<option value=\"{sg}\">Postes de l'atelier</option>")));
    let r = ana.post(
        "/compte/organisation/jetons/creer?lang=fr",
        &[("id", &id), ("nom", "Atelier"), ("jours", "30"), ("nombre", "5"), ("groupe", ""), ("groupe_ordinateurs", &sg_s)],
    );
    let text = txt(&r).to_string();
    let token = between(&text, "<p class=\"token\">", "<").to_string();
    assert!(text.contains("rangées dans Postes de l'atelier"));
    let st3 = station(74);
    assert!(matches!(w.acc.enroll(&st3, &token, "POSTE-03", 1), Msg::Attached(_)));
    assert!(computers(&w, &ines).as_array().unwrap().iter().any(|c| c["station"] == json!(st3) && c["rights"] == json!(["view"])));
    assert!(w.acc.db().station_groups(org.id).unwrap()[0].stations.iter().any(|(s, _)| *s == st3));
    // Un groupe d'ordinateurs d'une autre équipe : refusé.
    let r = ana.post(
        "/compte/organisation/jetons/creer?lang=fr",
        &[("id", &id), ("nom", "X"), ("jours", "30"), ("nombre", "5"), ("groupe", ""), ("groupe_ordinateurs", "9999")],
    );
    assert!(txt(&r).contains("Vous ne pouvez pas faire cela ici"));

    // Retirer un ordinateur du groupe, puis supprimer le groupe : ses droits partent avec lui.
    assert!(txt(&add(&mut ana, &st3, "retirer")).contains("Groupe changé"));
    assert!(!computers(&w, &ines).as_array().unwrap().iter().any(|c| c["station"] == json!(st3)));
    let r = ana.post("/compte/organisation/groupes-ordinateurs/supprimer?lang=fr", &[("id", &id), ("groupe", &sg_s)]);
    assert!(txt(&r).contains("Groupe supprimé"));
    assert_eq!(computers(&w, &hugo), json!([]));

    // Le journal de l'équipe.
    let kinds: Vec<String> = w.acc.db().org_log_of(org.id, 100).unwrap().into_iter().map(|l| l.kind).collect();
    for k in [
        "groupe_ordinateurs_cree",
        "groupe_ordinateurs_ajoute",
        "groupe_ordinateurs_ouvert",
        "groupe_ordinateurs_ferme",
        "groupe_ordinateurs_retire",
        "groupe_ordinateurs_supprime",
        "station_enrolee",
    ] {
        assert!(kinds.iter().any(|x| x == k), "{k} absent du journal : {kinds:?}");
    }
    let page = txt(&ana.get(&format!("/compte/organisation?id={id}&vue=journal&lang=fr"))).to_string();
    assert!(page.contains("POSTE-03 enrôlée par le jeton Atelier, rangée dans Postes de l'atelier"));
}

#[test]
fn liens_d_un_ordinateur_d_equipe_au_journal() {
    let w = world(true);
    let _link = linked(&w);
    let mut ana = person(&w, "ana@exemple.org", "Ana", "192.0.2.10");
    with_totp(&w, &mut ana, "ana@exemple.org");
    ana.post("/compte/organisations/creer?lang=fr", &[("nom", "Atelier")]);
    let org = w.acc.db().orgs_of(1).unwrap()[0].0.clone();
    let st = station(81);
    w.acc.db().attach_station_to_org(&st, "Poste 1", org.id, Some(1), None, w.now()).unwrap();
    let path = new_link(&mut ana, &st, &[("jours", "7"), ("view", "1")]);
    let mut paul = person(&w, "paul@exemple.org", "Paul", "192.0.2.15");
    let r = paul.post(&format!("{path}?lang=fr"), &[]);
    assert!(txt(&r).contains("C'est fait"));
    let path2 = new_link(&mut ana, &st, &[("jours", "7"), ("view", "1")]);
    let id2 = w.acc.db().access_links(&st, 1).unwrap()[0].id;
    ana.post("/compte/ordinateur/liens/revoquer?lang=fr", &[("id", &st), ("lien", &id2.to_string())]);
    assert!(txt(&paul.get(&format!("{path2}?lang=fr"))).contains("Ce lien a été révoqué."));
    let log = w.acc.db().org_log_of(org.id, 20).unwrap();
    let kinds: Vec<&str> = log.iter().map(|l| l.kind.as_str()).collect();
    for k in ["lien_cree", "lien_utilise", "lien_revoque"] {
        assert!(kinds.contains(&k), "{k} : {kinds:?}");
    }
    let page = txt(&ana.get(&format!("/compte/organisation?id={}&vue=journal&lang=fr", org.public_id))).to_string();
    assert!(page.contains("Paul a utilisé un lien d'accès à Poste 1 (Regarder)"), "{page}");
}
