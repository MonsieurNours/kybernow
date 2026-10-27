//! Les pages du relais, en français ou en anglais selon le navigateur
//! (ton : docs/ton-du-site.md ; textes communs : docs/link-ui.md).

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

fn layout(lang: Lang, title: &str, body: &str) -> String {
    let footer_relay = t(lang, "Relais de KyberNow", "KyberNow relay");
    let footer_privacy = t(lang, "Confidentialité", "Privacy");
    format!(
        r#"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>{title} · KyberNow</title>
<style>
:root {{ color-scheme: dark light; --bg: #05070A; --surface: #0E141A; --line: #1E2832; --text: #E9EEF1; --muted: #9AA7B0; --signal: #9A8CFF; --bad: #FF6B6B; }}
@media (prefers-color-scheme: light) {{ :root {{ --bg: #F2F5F6; --surface: #FFFFFF; --line: #D8E0E4; --text: #10161B; --muted: #55626B; --signal: #5340E6; --bad: #C93A3A; }} }}
* {{ box-sizing: border-box; }}
body {{ margin: 0; min-height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 24px 16px; background: var(--bg); color: var(--text); font: 16px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; }}
main {{ width: 100%; max-width: 420px; background: var(--surface); border: 1px solid var(--line); border-radius: 16px; padding: 28px 24px; }}
h1 {{ font-size: 1.5rem; line-height: 1.25; margin: 0 0 8px; }}
p {{ margin: 0 0 16px; color: var(--muted); }}
label {{ display: block; font-weight: 600; margin-bottom: 6px; }}
input {{ width: 100%; font: 600 1.75rem/1.2 ui-monospace, "SF Mono", Menlo, monospace; letter-spacing: 0.15em; text-align: center; padding: 12px; border-radius: 10px; border: 1px solid var(--line); background: var(--bg); color: var(--text); }}
input:focus-visible, button:focus-visible, a:focus-visible {{ outline: 2px solid var(--signal); outline-offset: 2px; }}
button {{ width: 100%; margin-top: 16px; padding: 12px; border: 0; border-radius: 10px; background: var(--signal); color: #fff; font: 600 1rem/1.2 inherit; cursor: pointer; }}
.notice {{ color: var(--bad); font-weight: 600; }}
.hint {{ font-size: 0.9rem; margin: 16px 0 0; }}
footer {{ margin-top: 20px; font-size: 0.85rem; color: var(--muted); }}
footer a, main a {{ color: var(--signal); }}
</style>
</head>
<body>
<main>
{body}
</main>
<footer>{footer_relay} · <a href="/privacy">{footer_privacy}</a></footer>
</body>
</html>
"#,
        lang = lang.code(),
    )
}

/// L'accueil : rejoindre un partage avec son code.
pub fn home(lang: Lang, notice: Option<Notice>) -> String {
    let title = t(lang, "Rejoindre un partage", "Join a share");
    let notice = match notice {
        None => String::new(),
        Some(Notice::BadCode) => t(lang, "Le code a six chiffres.", "The code has six digits."),
        Some(Notice::Unknown) => t(
            lang,
            "Aucun partage avec ce code. Vérifiez les six chiffres chez la personne qui partage.",
            "No share with this code. Check the six digits with the person sharing.",
        ),
        Some(Notice::TooMany) => t(lang, "Trop d'essais de code : réessayez dans une minute.", "Too many code attempts: try again in a minute."),
    };
    let notice = if notice.is_empty() { notice } else { format!(r#"<p class="notice" role="alert">{notice}</p>"#) };
    let body = format!(
        r#"<h1>{title}</h1>
<p>{help}</p>
{notice}<form method="get" action="/j">
<label for="code">{code}</label>
<input id="code" name="code" inputmode="numeric" autocomplete="one-time-code" pattern="[0-9 ]*" maxlength="7" placeholder="123 456" required autofocus>
<button type="submit">{join}</button>
</form>
<p class="hint">{app}</p>"#,
        help = t(lang, "Le code s'affiche chez la personne qui partage.", "The code is shown by the person sharing."),
        code = t(lang, "Code", "Code"),
        join = t(lang, "Rejoindre", "Join"),
        app = t(
            lang,
            "Vous avez KyberNow Link ? Touchez « Rejoindre avec un code » dans l'app.",
            "Have KyberNow Link? Use “Join with a code” in the app.",
        ),
    );
    layout(lang, &title, &body)
}

/// La Station n'est pas connectée au relais.
pub fn offline(lang: Lang) -> String {
    let title = t(lang, "Ordinateur injoignable", "Computer unreachable");
    let body = format!(
        "<h1>{title}</h1>\n<p>{text}</p>\n<p><a href=\"/\">{back}</a></p>",
        text = t(lang, "Cet ordinateur n'est pas joignable : il est éteint ou en veille.", "This computer cannot be reached: it is off or asleep."),
        back = t(lang, "Rejoindre avec un autre code", "Join with another code"),
    );
    layout(lang, &title, &body)
}

/// La Station est connectée mais ne répond pas à la requête.
pub fn not_answering(lang: Lang) -> String {
    let title = t(lang, "Ordinateur injoignable", "Computer unreachable");
    let body = format!(
        "<h1>{title}</h1>\n<p>{text}</p>",
        text = t(lang, "L'ordinateur ne répond pas pour le moment : réessayez dans un instant.", "The computer is not answering right now: try again in a moment."),
    );
    layout(lang, &title, &body)
}

pub fn not_found(lang: Lang) -> String {
    let title = t(lang, "Page introuvable", "Page not found");
    let body = format!("<h1>{title}</h1>\n<p><a href=\"/\">{back}</a></p>", back = t(lang, "Rejoindre un partage", "Join a share"));
    layout(lang, &title, &body)
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
    layout(lang, &title, &format!("<h1>{title}</h1>\n{paragraphs}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn langue_du_navigateur() {
        assert_eq!(Lang::of(Some("fr-FR,fr;q=0.9,en;q=0.8")), Lang::Fr);
        assert_eq!(Lang::of(Some("en-GB,fr;q=0.5")), Lang::En);
        assert_eq!(Lang::of(Some("de-DE,fr;q=0.7")), Lang::Fr);
        assert_eq!(Lang::of(None), Lang::En);
    }

    #[test]
    fn typographie_francaise() {
        let page = home(Lang::Fr, Some(Notice::TooMany));
        assert!(page.contains("Rejoindre un partage"));
        assert!(page.contains("d'essais de code\u{202F}: réessayez"));
        assert!(page.contains("«\u{202F}Rejoindre avec un code\u{202F}»"));
        assert!(home(Lang::En, None).contains("Join a share"));
    }
}
