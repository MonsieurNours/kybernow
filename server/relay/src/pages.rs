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
    download: &'static str,
    guide: &'static str,
    legal: &'static str,
}

fn site(lang: Lang) -> Site {
    match lang {
        Lang::Fr => Site {
            home: "https://www.kybernow.app/",
            download: "https://www.kybernow.app/#telecharger",
            guide: "https://www.kybernow.app/guide/",
            legal: "https://www.kybernow.app/mentions-legales/",
        },
        Lang::En => Site {
            home: "https://www.kybernow.app/en/",
            download: "https://www.kybernow.app/en/#telecharger",
            guide: "https://www.kybernow.app/en/guide/",
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

/// Un bloc de la liste « Ce que fait KyberNow Share ».
fn card(icon_name: &str, title: String, text: String) -> String {
    format!("<article class=\"card\">{}<h3>{title}</h3><p>{text}</p></article>\n", icon(icon_name, "ic-lg"))
}

/// L'accueil : ce que fait KyberNow Share, et la saisie du code pour rejoindre un partage.
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

    let hero = format!(
        r##"<section class="wrap hero">
<div class="hero-copy">
<h1 class="display">{h1}</h1>
<p class="lede">{lede}</p>
<dl class="facts">
<div><dt>{f1t}</dt><dd>{f1}</dd></div>
<div><dt>{f2t}</dt><dd>{f2}</dd></div>
<div><dt>{f3t}</dt><dd>{f3}</dd></div>
</dl>
</div>
<div class="join" id="rejoindre">
<h2>{join_title}</h2>
<p class="help">{help}</p>
{notice_html}<form method="get" action="/j">
<input type="hidden" name="lang" value="{lang_code}">
<label class="mono-label" for="code">{code}</label>
<input id="code" name="code" inputmode="numeric" enterkeyhint="go" autocomplete="one-time-code" pattern="[0-9 ]*" maxlength="7" placeholder="123 456" required{autofocus}>
<button class="btn kn-primary" type="submit">{join}</button>
</form>
<p class="hint">{app}</p>
</div>
</section>
"##,
        h1 = tr("Montrez votre ordinateur à <strong>vos amis, où qu'ils soient.</strong>", "Show your computer to <strong>your friends, wherever they are.</strong>"),
        lede = tr(
            "Avec KyberNow Share, vos invités voient et entendent votre écran. Si vous le décidez, ils jouent avec leurs propres manettes, ou se servent du clavier et de la souris. Ils entrent avec un code à six chiffres, depuis une app KyberNow Link ou un simple navigateur.",
            "With KyberNow Share, your guests see and hear your screen. If you choose, they can play with their own controllers or use the keyboard and mouse. They get in with a six-digit code, from a KyberNow Link app or a plain browser.",
        ),
        f1t = tr("Invités", "Guests"),
        f1 = tr("Quatre au plus", "Four at most"),
        f2t = tr("Code", "Code"),
        f2 = tr("Six chiffres", "Six digits"),
        f3t = tr("Prix", "Price"),
        f3 = tr("Gratuit", "Free"),
        join_title = tr("Rejoindre avec un code", "Join with a code"),
        help = tr("Le code s'affiche chez la personne qui partage.", "The code is shown by the person sharing."),
        code = tr("Code", "Code"),
        join = tr("Rejoindre", "Join"),
        app = tr("Vous avez KyberNow Link ? Touchez « Rejoindre avec un code » dans l'app.", "Have KyberNow Link? Use “Join with a code” in the app."),
        lang_code = lang.code(),
    );

    let how = format!(
        r##"<section class="wrap section" id="comment">
<h2 class="title section-head">{title}</h2>
<div class="how">
<ol class="steps">
<li><div><h3>{s1t}</h3><p>{s1}</p></div></li>
<li><div><h3>{s2t}</h3><p>{s2}</p></div></li>
<li><div><h3>{s3t}</h3><p>{s3}</p></div></li>
</ol>
<!-- Le panneau de l'hôte, recréé avec les textes de docs/link-ui.md -->
<div class="panel" role="img" aria-label="{panel_alt}">
<div class="panel-title">{p_title}</div>
<div class="panel-code"><span class="mono-label">{p_code}</span><b>123 456</b></div>
<span class="mono-label">{p_requests}</span>
<div class="row"><div><b>{p_req}</b><small>{p_req_app}</small></div><div class="acts"><span class="pill pill-on">{p_accept}</span><span class="pill">{p_decline}</span></div></div>
<span class="mono-label">{p_guests}</span>
<div class="row"><div><b>{p_lea}</b><small>{p_lea_app}</small></div><div class="acts"><span class="pill">{p_give_pads}</span><span class="pill">{p_give_kbm}</span></div></div>
<div class="row"><div><b>{p_max}</b><small>{p_max_app}</small></div><div class="acts"><span class="pill">{p_take_pads}</span><span class="pill">{p_give_kbm}</span></div></div>
</div>
<!-- /panel -->
</div>
</section>
"##,
        title = tr("Partager en <strong>trois étapes.</strong>", "Sharing in <strong>three steps.</strong>"),
        s1t = tr("Vous ouvrez le partage", "You open the share"),
        s1 = tr(
            "Choisissez « Partager » dans la barre de la session de KyberNow Link, ou « Partager un écran… » dans le menu de KyberNow Station sur le PC, près de l'horloge. Un code à six chiffres s'affiche, avec un lien et un QR code.",
            "Choose “Share” in the session bar of KyberNow Link, or “Share a screen…” in the KyberNow Station menu on the PC, next to the clock. A six-digit code appears, with a link and a QR code.",
        ),
        s2t = tr("Vos invités entrent", "Your guests get in"),
        s2 = tr(
            "Ils tapent le code sur cette page ou dans l'app KyberNow Link, ou ils ouvrent votre lien ou scannent le QR code. Un invité qui vient d'internet attend votre accord.",
            "They type the code on this page or in the KyberNow Link app, or they open your link or scan the QR code. A guest coming from the internet waits for your approval.",
        ),
        s3t = tr("Vous choisissez ce qu'ils peuvent faire", "You choose what they can do"),
        s3 = tr(
            "Chacun arrive en spectateur. Vous pouvez lui donner ses manettes, le clavier et la souris, ou les deux, et les reprendre quand vous voulez.",
            "Everyone arrives as a spectator. You can give them their controllers, the keyboard and mouse, or both, and take them back whenever you like.",
        ),
        panel_alt = tr(
            "Le panneau de partage de KyberNow : le code, une demande à accepter et deux invités, chacun avec ses droits.",
            "The KyberNow share panel: the code, a request to accept and two guests, each with their own rights.",
        ),
        p_title = tr("Partager Atelier", "Share Atelier"),
        p_code = tr("Code", "Code"),
        p_requests = tr("Demandes", "Requests"),
        p_req = tr("Camille demande à rejoindre", "Camille wants to join"),
        p_req_app = tr("iPhone ou iPad · hors de chez vous", "iPhone or iPad · away from home"),
        p_accept = tr("Accepter", "Accept"),
        p_decline = tr("Refuser", "Decline"),
        p_guests = tr("Invités", "Guests"),
        p_lea = tr("Léa · Spectateur", "Léa · Spectator"),
        p_lea_app = tr("Navigateur · depuis 3\u{a0}min", "Browser · for 3\u{a0}min"),
        p_max = tr("Max · Joueur", "Max · Player"),
        p_max_app = tr("Mac · depuis 12\u{a0}min", "Mac · for 12\u{a0}min"),
        p_give_pads = tr("Donner les manettes", "Give controllers"),
        p_take_pads = tr("Reprendre les manettes", "Take back controllers"),
        p_give_kbm = tr("Donner le clavier et la souris", "Give keyboard and mouse"),
    );

    let mut cards = String::new();
    cards.push_str(&card(
        "game-controller",
        tr("Spectateur ou joueur", "Spectator or player"),
        tr(
            "Un invité arrive en spectateur : il voit et entend votre écran. Si vous le décidez, il devient joueur, et sa manette est vue par votre ordinateur comme une manette de plus.",
            "A guest arrives as a spectator, seeing and hearing your screen. If you choose, they become a player, and their controller shows up on your computer as one more controller.",
        ),
    ));
    cards.push_str(&card(
        "cursor-click",
        tr("Chaque droit se donne à part", "Each right is given separately"),
        tr(
            "Les manettes d'un côté, le clavier et la souris de l'autre : ce sont deux droits distincts. Vous donnez ou reprenez chacun à tout moment, invité par invité. Un invité n'a jamais que ce que vous lui donnez, et jamais votre presse-papiers.",
            "Controllers and keyboard with mouse are two separate rights. You give or take back either one at any time, guest by guest. A guest only ever has what you give them, and never your clipboard.",
        ),
    ));
    cards.push_str(&card(
        "frame-corners",
        tr("Une image à la mesure de chaque invité", "A picture sized for each guest"),
        tr(
            "Chaque invité reçoit une image adaptée à son écran et à sa connexion. La vôtre passe toujours en premier : rien de ce que fait un invité ne change votre écran ni votre image.",
            "Each guest gets a picture suited to their screen and their connection. Yours always comes first: nothing a guest does changes your screen or your picture.",
        ),
    ));
    cards.push_str(&card(
        "globe",
        tr("Sur le même réseau, ou de n'importe où", "On the same network, or from anywhere"),
        tr(
            "Sur le même réseau, le code suffit. Ailleurs, vos invités passent par le relais de KyberNow, avec le même code, le même lien et le même QR code.",
            "On the same network, the code is enough. Elsewhere, your guests go through the KyberNow relay, with the same code, the same link and the same QR code.",
        ),
    ));
    cards.push_str(&card(
        "shield-check",
        tr("Hors de chez vous, vous acceptez chaque invité", "Away from home, you accept each guest"),
        tr(
            "Quand un invité arrive d'internet, une demande s'affiche chez vous : vous l'acceptez ou vous la refusez. Sur votre réseau, il n'y a rien à accepter. Quatre invités au plus.",
            "When a guest arrives from the internet, a request appears on your side and you accept or decline it. On your own network, there is nothing to accept. Four guests at most.",
        ),
    ));
    cards.push_str(&card(
        "browser",
        tr("Une app ou un simple navigateur", "An app or a plain browser"),
        tr(
            "Dans une app KyberNow Link, choisissez « Rejoindre avec un code ». Sans app, ouvrez le lien ou tapez le code sur cette page : il n'y a rien à installer.",
            "In a KyberNow Link app, choose “Join with a code”. Without an app, open the link or type the code on this page: there is nothing to install.",
        ),
    ));

    let more = format!(
        r##"<div class="cards cards-more">
{away}<article class="card card-soon"><div class="head">{pc_icon}<span class="tag">{soon}</span></div><h3>{pc_title}</h3><p>{pc_text}</p></article>
</div>
"##,
        away = card(
            "desktop-tower",
            tr("Votre ordinateur, même hors de chez vous", "Your computer, even away from home"),
            tr(
                "Le même relais ouvre aussi votre propre ordinateur depuis l'hôtel ou le train, sans VPN. Vos appareils déjà appairés le retrouvent quand ils ne sont pas sur votre réseau. Le réglage « Accès hors de chez vous » se trouve dans Admin, et il reste coupé tant que vous ne l'activez pas.",
                "The same relay also opens your own computer from the hotel or the train, without a VPN. Your already paired devices find it when they are not on your network. The “Access away from home” setting is in Admin, and it stays off until you turn it on.",
            ),
        ),
        pc_icon = icon("plugs", "ic-lg"),
        soon = tr("Bientôt", "Coming soon"),
        pc_title = tr("Prêter votre PC", "Lend your PC"),
        pc_text = tr(
            "Un ami utilise votre ordinateur avec un compte à part, dans les apps que vous choisissez, à tour de rôle avec vous. Ce n'est pas encore disponible.",
            "A friend uses your computer with a separate account, in the apps you choose, taking turns with you. This is not available yet.",
        ),
    );

    let features = format!(
        r##"<section class="wrap section" id="fonctions">
<h2 class="title section-head">{title}</h2>
<div class="cards">
{cards}</div>
{more}<p class="footnote">{privacy_note} <a href="/privacy">{privacy_link}</a>.</p>
</section>
"##,
        title = tr("Ce que fait <strong>KyberNow Share.</strong>", "What <strong>KyberNow Share</strong> does."),
        privacy_note = tr(
            "Dans les apps KyberNow Link, l'image, le son et ce que vous tapez restent chiffrés d'un bout à l'autre : le relais ne les voit pas. Dans un navigateur, il fait suivre la connexion sans rien en garder. Le détail est dans la page",
            "In the KyberNow Link apps, the picture, the sound and what you type stay encrypted from end to end: the relay cannot see them. In a browser, it passes the connection on and keeps none of it. The details are on the",
        ),
        privacy_link = tr("Confidentialité", "Privacy page"),
    );

    let get = format!(
        r##"<section class="wrap section" id="installer">
<div class="get">
<div>
<h2>{title}</h2>
<p>{text}</p>
</div>
<div class="acts">
<a class="btn btn-ghost" href="{download}" rel="noopener">{dl_icon}{download_label}</a>
<a class="btn btn-ghost" href="{guide}" rel="noopener">{guide_icon}{guide_label}</a>
</div>
</div>
</section>
"##,
        title = tr("Ce qu'il vous faut pour partager", "What you need to share"),
        text = tr(
            "KyberNow Station, en version 0.8.0 ou plus récente, sur l'ordinateur que vous partagez. Vos invités n'ont rien à installer s'ils passent par un navigateur.",
            "KyberNow Station, version 0.8.0 or later, on the computer you share. Your guests have nothing to install if they use a browser.",
        ),
        download = site.download,
        guide = site.guide,
        dl_icon = icon("download-simple", "ic"),
        guide_icon = icon("book-open-text", "ic"),
        download_label = tr("Télécharger KyberNow", "Download KyberNow"),
        guide_label = tr("Lire le guide", "Read the guide"),
    );

    let main = format!("<main>\n{hero}{how}{features}{get}</main>");
    layout(
        lang,
        &tr("KyberNow Share · Montrez votre ordinateur à vos amis", "KyberNow Share · Show your computer to your friends"),
        &tr(
            "Faites voir et entendre votre ordinateur à vos amis, où qu'ils soient, et laissez-les jouer avec leurs manettes si vous le voulez. Ils entrent avec un code à six chiffres, depuis une app KyberNow Link ou un navigateur.",
            "Let your friends see and hear your computer, wherever they are, and let them play with their own controllers if you wish. They get in with a six-digit code, from a KyberNow Link app or a browser.",
        ),
        "/",
        true,
        &main,
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
    fn l_accueil_dit_ce_que_fait_share() {
        let fr = home(Lang::Fr, None);
        let en = home(Lang::En, None);
        // Le titre, le champ du code et ce qui est annoncé.
        assert_eq!(fr.matches("<h1").count(), 1);
        assert!(fr.contains("Montrez votre ordinateur à <strong>vos amis, où qu'ils soient.</strong>"));
        assert!(en.contains("Show your computer to <strong>your friends, wherever they are.</strong>"));
        assert!(fr.contains(r#"<form method="get" action="/j">"#));
        assert!(fr.contains(r#"name="code""#) && fr.contains(r#"inputmode="numeric""#));
        for needle in [
            "Spectateur ou joueur",
            "Chaque droit se donne à part",
            "Une image à la mesure de chaque invité",
            "Sur le même réseau, ou de n'importe où",
            "Hors de chez vous, vous acceptez chaque invité",
            "Une app ou un simple navigateur",
            "Votre ordinateur, même hors de chez vous",
            "Accès hors de chez vous",
        ] {
            assert!(fr.contains(needle), "FR : {needle}");
        }
        for needle in [
            "Spectator or player",
            "Each right is given separately",
            "A picture sized for each guest",
            "On the same network, or from anywhere",
            "Away from home, you accept each guest",
            "An app or a plain browser",
            "Your computer, even away from home",
            "Access away from home",
        ] {
            assert!(en.contains(needle), "EN : {needle}");
        }
        // « Bientôt » : le prêt du PC est annoncé, pas offert.
        assert!(fr.contains("Prêter votre PC") && fr.contains(">Bientôt<") && fr.contains("pas encore disponible"));
        assert!(en.contains("Lend your PC") && en.contains(">Coming soon<") && en.contains("not available yet"));
    }

    #[test]
    fn liens_vers_le_site_dans_la_langue_de_la_page() {
        let fr = home(Lang::Fr, None);
        assert!(fr.contains(r#"href="https://www.kybernow.app/""#) && fr.contains("Découvrir KyberNow"));
        assert!(fr.contains(r#"href="https://www.kybernow.app/#telecharger""#));
        assert!(fr.contains(r#"href="https://www.kybernow.app/guide/""#));
        assert!(fr.contains(r#"href="https://www.kybernow.app/mentions-legales/""#));
        assert!(fr.contains(r#"href="/?lang=en""#));
        let en = home(Lang::En, None);
        assert!(en.contains(r#"href="https://www.kybernow.app/en/""#) && en.contains("Discover KyberNow"));
        assert!(en.contains(r#"href="https://www.kybernow.app/en/guide/""#));
        assert!(en.contains(r#"href="https://www.kybernow.app/en/legal/""#));
        assert!(en.contains(r#"href="/?lang=fr""#));
        // La page de confidentialité garde la langue choisie.
        assert!(privacy(Lang::Fr).contains(r#"href="/privacy?lang=en""#));
        // Le formulaire redonne la langue à /j.
        assert!(fr.contains(r#"name="lang" value="fr""#) && en.contains(r#"name="lang" value="en""#));
    }

    #[test]
    fn pas_de_liste_de_plateformes_dans_le_texte() {
        // Message agnostique : « votre ordinateur sur tous vos appareils ». Seul l'écran recréé
        // de l'hôte (aria-hidden) nomme des apps, comme les vraies.
        for lang in [Lang::Fr, Lang::En] {
            let page = home(lang, None);
            let (before, rest) = page.split_once("<!-- Le panneau de l'hôte").unwrap();
            let (_, after) = rest.split_once("<!-- /panel -->").unwrap();
            let text = text_of(&format!("{before}{after}"));
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
