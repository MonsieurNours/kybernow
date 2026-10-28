//! Les pages du relais, en français ou en anglais selon le navigateur (ou le
//! choix `?lang=` du visiteur). Ton et règles de texte : docs/ton-du-site.md ;
//! textes communs : docs/link-ui.md. Mise en page : `assets/share.css`, servie
//! sous `/assets/` ([`crate::assets`]) ; aucune page n'a de script.

use crate::assets;

/// La langue d'une page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    /// D'après `Accept-Language` : le français s'il vient avant l'anglais.
    pub fn of(accept: Option<&str>) -> Lang {
        let Some(accept) = accept else { return Lang::En };
        for tag in accept.split(',') {
            let tag = tag.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
            if tag.starts_with("fr") {
                return Lang::Fr;
            }
            if tag.starts_with("en") {
                return Lang::En;
            }
        }
        Lang::En
    }

    /// Le choix du visiteur (`?lang=fr` ou `?lang=en`, sans cookie), sinon `Accept-Language`.
    pub fn of_request(query: Option<&str>, accept: Option<&str>) -> Lang {
        match query.and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("lang="))) {
            Some("fr") => Lang::Fr,
            Some("en") => Lang::En,
            _ => Lang::of(accept),
        }
    }

    fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }
}

/// Un avis au-dessus du formulaire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    BadCode,
    Unknown,
    TooMany,
}

/// Espaces fines insécables du français devant : ; ! ? et dans les guillemets.
fn fr(s: &str) -> String {
    s.replace(" :", "\u{202F}:")
        .replace(" ;", "\u{202F};")
        .replace(" !", "\u{202F}!")
        .replace(" ?", "\u{202F}?")
        .replace("« ", "«\u{202F}")
        .replace(" »", "\u{202F}»")
}

fn t(lang: Lang, fr_text: &str, en: &str) -> String {
    match lang {
        Lang::Fr => fr(fr_text),
        Lang::En => en.to_string(),
    }
}

/// Les adresses du site de KyberNow, dans la langue de la page.
struct Site {
    home: &'static str,
    legal: &'static str,
}

fn site(lang: Lang) -> Site {
    match lang {
        Lang::Fr => Site {
            home: "https://www.kybernow.app/",
            legal: "https://www.kybernow.app/mentions-legales/",
        },
        Lang::En => Site {
            home: "https://www.kybernow.app/en/",
            legal: "https://www.kybernow.app/en/legal/",
        },
    }
}

/// Une icône Phosphor de la page (les symboles sont écrits en ligne, [`assets::ICONS`]).
fn icon(name: &str, class: &str) -> String {
    format!(r##"<svg class="{class}" aria-hidden="true" focusable="false"><use href="#{name}"/></svg>"##)
}

/// Une page : barre du haut (logo, lien vers le site, langue), contenu, pied de page.
/// `here` est l'adresse de la page, pour le changement de langue (`?lang=`, sans cookie) ;
/// `with_icons` écrit les icônes en ligne (l'accueil seulement).
fn layout(lang: Lang, doc_title: &str, description: &str, here: &str, with_icons: bool, main: &str) -> String {
    let site = site(lang);
    let (other_code, other_name, other_short) = match lang {
        Lang::Fr => ("en", "English", "EN"),
        Lang::En => ("fr", "Français", "FR"),
    };
    let logo_label = t(lang, "KyberNow Share, version bêta, accueil", "KyberNow Share, beta version, home");
    let beta = t(lang, "Bêta", "Beta");
    let discover = t(lang, "Découvrir KyberNow", "Discover KyberNow");
    let footer_relay = t(lang, "Relais de KyberNow", "KyberNow relay");
    let footer_privacy = t(lang, "Confidentialité", "Privacy");
    let footer_legal = t(lang, "Mentions légales", "Legal notice");
    let sprite = if with_icons {
        format!(r##"<svg class="sprite" aria-hidden="true" focusable="false" xmlns="http://www.w3.org/2000/svg">{}</svg>"##, assets::ICONS)
    } else {
        String::new()
    };
    let arrow = if with_icons { icon("arrow-right", "ic ic-go") } else { String::new() };
    format!(
        r##"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="dark">
<meta name="theme-color" content="#05070A">
<meta name="robots" content="noindex">
<meta name="description" content="{description}">
<link rel="icon" type="image/svg+xml" href="{favicon}">
<link rel="preload" href="{font}" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="{css}">
<title>{doc_title}</title>
</head>
<body>
{sprite}
<header class="wrap bar">
<a class="bar-logo" href="/" aria-label="{logo_label}"><img src="{logo}" alt="KyberNow Share" width="185" height="34"><span class="beta" aria-hidden="true">{beta}</span></a>
<nav class="bar-links">
<a class="btn btn-ghost btn-sm" href="{site_home}" rel="noopener">{discover}{arrow}</a>
<a class="lang" href="{here}?lang={other_code}" hreflang="{other_code}" lang="{other_code}" title="{other_name}">{other_short}</a>
</nav>
</header>
{main}
<footer class="footer">
<div class="wrap footer-in"><span>{footer_relay}</span><a href="/privacy">{footer_privacy}</a><a href="{site_legal}" rel="noopener">{footer_legal}</a></div>
</footer>
</body>
</html>
"##,
        lang = lang.code(),
        favicon = assets::url("kybernow-favicon.svg"),
        font = assets::url("archivo.woff2"),
        css = assets::url("share.css"),
        logo = assets::url("kybernow-share-dark.svg"),
        site_home = site.home,
        site_legal = site.legal,
    )
}

/// Une page simple (confidentialité, ordinateur injoignable, page introuvable).
fn simple(lang: Lang, title: &str, here: &str, body: &str) -> String {
    let description = t(
        lang,
        "KyberNow Share : montrez votre ordinateur à vos amis, où qu'ils soient.",
        "KyberNow Share: show your computer to your friends, wherever they are.",
    );
    let main = format!("<main class=\"wrap page\">\n{body}\n</main>");
    layout(lang, &format!("{title} · KyberNow Share"), &description, here, false, &main)
}

/// L'accueil : le logo, la saisie du code pour rejoindre un partage, un lien vers le site et la
/// confidentialité. Rien d'autre : le site présente KyberNow.
pub fn home(lang: Lang, notice: Option<Notice>) -> String {
    let site = site(lang);
    let tr = |f: &str, e: &str| t(lang, f, e);
    let notice_text = match notice {
        None => String::new(),
        Some(Notice::BadCode) => tr("Le code a six chiffres.", "The code has six digits."),
        Some(Notice::Unknown) => tr(
            "Aucun partage avec ce code. Vérifiez les six chiffres chez la personne qui partage.",
            "No share with this code. Check the six digits with the person sharing.",
        ),
        Some(Notice::TooMany) => tr("Trop d'essais de code : réessayez dans une minute.", "Too many code attempts: try again in a minute."),
    };
    // Après un refus, le curseur revient dans le champ ; sinon la page ne fait pas surgir le clavier d'un téléphone.
    let (notice_html, autofocus) = if notice_text.is_empty() {
        (String::new(), "")
    } else {
        (format!("<p class=\"notice\" role=\"alert\">{notice_text}</p>\n"), " autofocus")
    };
    format!(
        r##"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="dark">
<meta name="theme-color" content="#05070A">
<meta name="robots" content="noindex">
<link rel="icon" type="image/svg+xml" href="{favicon}">
<link rel="preload" href="{font}" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="{css}">
<title>{doc_title}</title>
</head>
<body class="solo">
<main class="solo-in">
<img class="solo-logo" src="{logo}" alt="KyberNow Share" width="260" height="48">
<div class="join">
<h1>{join_title}</h1>
<p class="help">{help}</p>
{notice_html}<form method="get" action="/j">
<input type="hidden" name="lang" value="{lang}">
<label class="mono-label" for="code">{code}</label>
<input id="code" name="code" inputmode="numeric" enterkeyhint="go" autocomplete="one-time-code" pattern="[0-9 ]*" maxlength="7" placeholder="123 456" required{autofocus}>
<button class="btn kn-primary" type="submit">{join}</button>
</form>
<p class="hint">{app}</p>
</div>
<nav class="solo-links"><a href="{site_home}" rel="noopener">{discover}</a><a href="/privacy">{privacy}</a></nav>
</main>
</body>
</html>
"##,
        lang = lang.code(),
        favicon = assets::url("kybernow-favicon.svg"),
        font = assets::url("archivo.woff2"),
        css = assets::url("share.css"),
        logo = assets::url("kybernow-share-dark.svg"),
        doc_title = tr("Rejoindre un partage · KyberNow Share", "Join a share · KyberNow Share"),
        join_title = tr("Rejoindre avec un code", "Join with a code"),
        help = tr("Le code s'affiche chez la personne qui partage.", "The code is shown by the person sharing."),
        code = tr("Code", "Code"),
        join = tr("Rejoindre", "Join"),
        app = tr("Vous avez KyberNow Link ? Touchez « Rejoindre avec un code » dans l'app.", "Have KyberNow Link? Use “Join with a code” in the app."),
        site_home = site.home,
        discover = tr("Découvrir KyberNow", "Discover KyberNow"),
        privacy = tr("Confidentialité", "Privacy"),
    )
}

/// La Station n'est pas connectée au relais.
pub fn offline(lang: Lang) -> String {
    let title = t(lang, "Ordinateur injoignable", "Computer unreachable");
    let body = format!(
        "<h1>{title}</h1>\n<p>{text}</p>\n<p><a href=\"/\">{back}</a></p>",
        text = t(lang, "Cet ordinateur n'est pas joignable : il est éteint ou en veille.", "This computer cannot be reached: it is off or asleep."),
        back = t(lang, "Rejoindre avec un autre code", "Join with another code"),
    );
    simple(lang, &title, "/", &body)
}

/// La Station est connectée mais ne répond pas à la requête.
pub fn not_answering(lang: Lang) -> String {
    let title = t(lang, "Ordinateur injoignable", "Computer unreachable");
    let body = format!(
        "<h1>{title}</h1>\n<p>{text}</p>",
        text = t(lang, "L'ordinateur ne répond pas pour le moment : réessayez dans un instant.", "The computer is not answering right now: try again in a moment."),
    );
    simple(lang, &title, "/", &body)
}

pub fn not_found(lang: Lang) -> String {
    let title = t(lang, "Page introuvable", "Page not found");
    let body = format!("<h1>{title}</h1>\n<p><a href=\"/\">{back}</a></p>", back = t(lang, "Rejoindre un partage", "Join a share"));
    simple(lang, &title, "/", &body)
}

/// Ce que le relais voit et garde.
pub fn privacy(lang: Lang) -> String {
    let title = t(lang, "Confidentialité du relais", "Relay privacy");
    let items: [(&str, &str); 5] = [
        (
            "Ce relais fait passer KyberNow par internet quand votre appareil n'est pas sur le même réseau que l'ordinateur.",
            "This relay carries KyberNow over the internet when your device is not on the same network as the computer.",
        ),
        (
            "Les apps KyberNow Link restent chiffrées de bout en bout : le relais ne voit ni l'image, ni le son, ni ce que vous tapez.",
            "The KyberNow Link apps stay encrypted end to end: the relay sees neither the picture, nor the sound, nor what you type.",
        ),
        (
            "Dans un navigateur, le relais déchiffre la connexion pour la faire suivre à l'ordinateur. Il n'en garde rien.",
            "In a browser, the relay decrypts the connection to pass it on to the computer. It keeps none of it.",
        ),
        (
            "Pour se protéger des abus, le relais garde dans ses journaux les adresses IP et l'heure des connexions, quelques jours. Il ne garde jamais les codes de partage.",
            "To protect itself from abuse, the relay keeps IP addresses and connection times in its logs for a few days. It never keeps share codes.",
        ),
        (
            "Le relais public de KyberNow est en France, chez l'auteur de KyberNow. Vous pouvez aussi installer le vôtre.",
            "The public KyberNow relay is in France, at the KyberNow author's home. You can also install your own.",
        ),
    ];
    let paragraphs: String = items.iter().map(|(f, e)| format!("<p>{}</p>\n", t(lang, f, e))).collect();
    let body = format!(
        "<h1>{title}</h1>\n{paragraphs}<p><a href=\"/\">{back}</a></p>",
        back = t(lang, "Retour à KyberNow Share", "Back to KyberNow Share"),
    );
    simple(lang, &title, "/privacy", &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le texte d'une page, sans balises.
    fn text_of(html: &str) -> String {
        let mut out = String::new();
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    out.push(' ');
                }
                _ if !in_tag => out.push(c),
                _ => {}
            }
        }
        out
    }

    fn pages(lang: Lang) -> Vec<(&'static str, String)> {
        vec![
            ("home", home(lang, None)),
            ("home/bad", home(lang, Some(Notice::BadCode))),
            ("home/unknown", home(lang, Some(Notice::Unknown))),
            ("home/too_many", home(lang, Some(Notice::TooMany))),
            ("offline", offline(lang)),
            ("not_answering", not_answering(lang)),
            ("not_found", not_found(lang)),
            ("privacy", privacy(lang)),
        ]
    }

    #[test]
    fn langue_du_navigateur() {
        assert_eq!(Lang::of(Some("fr-FR,fr;q=0.9,en;q=0.8")), Lang::Fr);
        assert_eq!(Lang::of(Some("en-GB,fr;q=0.5")), Lang::En);
        assert_eq!(Lang::of(Some("de-DE,fr;q=0.7")), Lang::Fr);
        assert_eq!(Lang::of(None), Lang::En);
    }

    #[test]
    fn langue_choisie_par_le_visiteur() {
        assert_eq!(Lang::of_request(Some("lang=fr"), Some("en-GB")), Lang::Fr);
        assert_eq!(Lang::of_request(Some("code=123456&lang=en"), Some("fr-FR")), Lang::En);
        assert_eq!(Lang::of_request(Some("lang=de"), Some("fr-FR")), Lang::Fr, "une langue inconnue laisse le navigateur décider");
        assert_eq!(Lang::of_request(None, Some("fr-FR")), Lang::Fr);
        assert_eq!(Lang::of_request(None, None), Lang::En);
    }

    #[test]
    fn typographie_francaise() {
        let page = home(Lang::Fr, Some(Notice::TooMany));
        assert!(page.contains("Rejoindre avec un code"));
        assert!(page.contains("d'essais de code\u{202F}: réessayez"));
        assert!(page.contains("«\u{202F}Rejoindre avec un code\u{202F}»"));
        assert!(home(Lang::En, None).contains("Join with a code"));
        // Jamais d'espace ordinaire devant : ; ! ? en français, ni de tiret cadratin.
        for (name, page) in pages(Lang::Fr) {
            let text = text_of(&page);
            for bad in [" :", " ;", " !", " ?", "« ", " »"] {
                assert!(!text.contains(bad), "{name} : « {bad} » dans le texte");
            }
        }
        for lang in [Lang::Fr, Lang::En] {
            for (name, page) in pages(lang) {
                assert!(!page.contains('\u{2014}'), "{name} : tiret cadratin");
            }
        }
    }

    #[test]
    fn l_accueil_garde_l_essentiel() {
        let fr = home(Lang::Fr, None);
        let en = home(Lang::En, None);
        // Le logo, le champ du code, « Découvrir KyberNow » et la confidentialité : rien d'autre.
        assert_eq!(fr.matches("<h1").count(), 1);
        assert!(fr.contains(r#"alt="KyberNow Share""#));
        assert!(fr.contains("Rejoindre avec un code") && en.contains("Join with a code"));
        assert!(fr.contains(r#"<form method="get" action="/j">"#));
        assert!(fr.contains(r#"name="code""#) && fr.contains(r#"inputmode="numeric""#));
        assert!(fr.contains(r#"href="/privacy""#) && fr.contains(">Confidentialité<") && en.contains(">Privacy<"));
        for page in [&fr, &en] {
            assert!(!page.contains("<header") && !page.contains("<footer") && !page.contains("<section"));
        }
    }

    #[test]
    fn liens_vers_le_site_dans_la_langue_de_la_page() {
        let fr = home(Lang::Fr, None);
        assert!(fr.contains(r#"href="https://www.kybernow.app/""#) && fr.contains("Découvrir KyberNow"));
        let en = home(Lang::En, None);
        assert!(en.contains(r#"href="https://www.kybernow.app/en/""#) && en.contains("Discover KyberNow"));
        // Les autres pages gardent leurs liens vers le site et le changement de langue.
        assert!(privacy(Lang::Fr).contains(r#"href="https://www.kybernow.app/mentions-legales/""#));
        assert!(privacy(Lang::En).contains(r#"href="https://www.kybernow.app/en/legal/""#));
        assert!(privacy(Lang::Fr).contains(r#"href="/privacy?lang=en""#));
        // Le formulaire redonne la langue à /j.
        assert!(fr.contains(r#"name="lang" value="fr""#) && en.contains(r#"name="lang" value="en""#));
    }

    #[test]
    fn pas_de_liste_de_plateformes_dans_le_texte() {
        // Message agnostique : « votre ordinateur sur tous vos appareils ».
        for lang in [Lang::Fr, Lang::En] {
            let text = text_of(&home(lang, None));
            for platform in ["Windows", "macOS", "Android", "Linux", "iPhone", "iPad", "Apple TV", "webOS", "Raspberry", "Steam Deck", "tvOS"] {
                assert!(!text.contains(platform), "{platform} dans le texte de présentation");
            }
        }
    }

    #[test]
    fn rien_qui_suive_ou_qui_charge_du_dehors() {
        for lang in [Lang::Fr, Lang::En] {
            for (name, page) in pages(lang) {
                for bad in ["<script", "<iframe", "style=", "onclick=", "onload=", "<form method=\"post\"", "analytics", "cookie"] {
                    assert!(!page.contains(bad), "{name} : {bad}");
                }
                // Les seules adresses absolues sont des liens vers le site de KyberNow ; tout ce que la page charge vient du relais.
                for part in page.split("src=\"").skip(1).chain(page.split("<link ").skip(1).map(|l| l.split("href=\"").nth(1).unwrap_or(""))) {
                    assert!(part.starts_with('/'), "{name} : ressource hors du relais ({})", &part[..part.len().min(40)]);
                }
                for abs in page.split("https://").skip(1) {
                    assert!(abs.starts_with("www.kybernow.app/"), "{name} : lien vers {}", &abs[..abs.len().min(30)]);
                }
            }
        }
    }

    #[test]
    fn les_fichiers_de_la_page_existent() {
        // Chaque /assets/… cité par une page est servi, avec la version courante.
        for lang in [Lang::Fr, Lang::En] {
            for (name, page) in pages(lang) {
                let mut found = 0;
                for part in page.split("/assets/").skip(1) {
                    let file = part.split('?').next().unwrap();
                    assert!(assets::get(file).is_some(), "{name} : /assets/{file} n'est pas servi");
                    assert!(part[file.len()..].starts_with(&format!("?v={}", assets::version())), "{name} : {file} sans version");
                    found += 1;
                }
                assert!(found >= 3, "{name} : feuille de styles, police et logo attendus");
            }
        }
    }

    #[test]
    fn mots_du_site_qu_on_n_ecrit_plus() {
        // docs/ton-du-site.md : les mots creux et les tournures bannies.
        for lang in [Lang::Fr, Lang::En] {
            let text = text_of(&home(lang, None)).to_lowercase();
            for word in ["fluide", "fluidité", "révolutionn", "puissant", "ultime", "seamless", "effortless", "next-gen", "sans compromis", "expérience", "écosystème", "solution"] {
                assert!(!text.contains(word), "« {word} » dans l'accueil");
            }
        }
    }

    #[test]
    fn le_champ_du_code_ne_prend_le_curseur_qu_apres_un_refus() {
        assert!(!home(Lang::Fr, None).contains("autofocus"));
        assert!(home(Lang::Fr, Some(Notice::Unknown)).contains("autofocus"));
        assert!(home(Lang::En, Some(Notice::BadCode)).contains(r#"role="alert""#));
    }
}
