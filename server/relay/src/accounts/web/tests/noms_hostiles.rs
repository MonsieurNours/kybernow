//! REL-01 : des noms d'ordinateur, d'équipe et de personne contenant `<"'&` ne sortent jamais
//! du texte, ni dans `<title>`, ni dans le corps des pages de la console.

use super::lot2::{linked, station};
use super::lot45::{person, with_totp};
use super::*;

const HOSTILE: &str = "</title><meta http-equiv=\"refresh\" content=\"0;url=https://piege.example\">'&<b>";

fn assert_clean(page: &str, what: &str) {
    assert!(!page.contains("http-equiv=\"refresh\" content=\"0;url=https://piege.example\""), "injection dans {what}");
    assert!(!page.contains("</title><meta"), "titre refermé dans {what}");
    assert!(!page.contains("'&<b>"), "texte brut dans {what}");
    let title = between(page, "<title>", "</title>");
    assert!(!title.contains('<') && !title.contains('"'), "titre non échappé dans {what} : {title}");
}

#[test]
fn noms_hostiles_echappes_partout() {
    let w = world(true);
    let link = linked(&w);
    let mut ana = person(&w, "ana@exemple.org", HOSTILE, "192.0.2.10");
    with_totp(&w, &mut ana, "ana@exemple.org");
    // Un ordinateur au nom hostile, à elle.
    let st = station(41);
    link.online.lock().unwrap().insert(st.clone());
    let (code, _, _) = w.acc.attach_request(&st, HOSTILE);
    let page = ana.get(&format!("/compte/rattacher?code={code}&lang=fr"));
    assert_clean(html(&page), "rattacher");
    ana.post("/compte/rattacher?lang=fr", &[("code", &code), ("action", "rattacher")]);
    // Une équipe au nom hostile.
    ana.post("/compte/organisations/creer?lang=fr", &[("nom", HOSTILE)]);
    let org = w.acc.db().orgs_of(1).unwrap()[0].0.public_id.clone();
    let pages = [
        "/compte?lang=fr".to_string(),
        "/compte/ordinateurs?lang=fr".to_string(),
        format!("/compte/ordinateur?id={st}&lang=fr"),
        format!("/compte/ordinateur?id={st}&lang=en"),
        "/compte/personnes?lang=fr".to_string(),
        "/compte/familles?lang=fr".to_string(),
        format!("/compte/organisation?id={org}&lang=fr"),
        format!("/compte/organisation?id={org}&lang=en"),
        format!("/compte/organisation/deployer?id={org}&lang=fr"),
        format!("/compte/ouvrir-admin?station={st}&nonce=abc&retour=https://pc.local&lang=fr"),
    ];
    for p in pages {
        let r = ana.get(&p);
        if let Some(h) = r.html.as_deref() {
            assert_clean(h, &p);
        }
    }
}

#[test]
fn texte_du_titre() {
    use crate::accounts::pages::title_text;
    assert_eq!(title_text("Déplacer A&amp;B"), "Déplacer A&amp;B");
    assert_eq!(title_text("<b>x</b> & \"y\""), "x &amp; &quot;y&quot;");
    assert_eq!(title_text(HOSTILE), "&#39;&amp;");
    assert_eq!(title_text("a &lt;b&gt;"), "a &lt;b&gt;");
}
