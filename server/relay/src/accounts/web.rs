//! L'aiguillage des pages `/compte/…` (docs/comptes.md, « Lot 1 », « Pages »).
//! Une requête déjà lue ([`Request`] : méthode, chemin, cookies, formulaire)
//! donne une réponse ([`Reply`]) ; `crate::web` fait le lien avec hyper.
//!
//! Sécurité (docs/comptes.md, « Sécurité ») :
//! - session : cookie `__Host-kn_session` (`Secure`, `HttpOnly`, `SameSite=Lax`),
//!   jeton de 256 bits gardé en empreinte ; un nouveau jeton à chaque étape
//!   (mot de passe, puis double authentification) ;
//! - CSRF : cookie `__Host-kn_csrf` et, dans chaque formulaire, sa signature
//!   (HMAC de la clé du relais) ; les envois d'un autre site sont refusés ;
//! - réponses identiques qu'un compte existe ou non (connexion, oubli,
//!   inscription d'une adresse déjà prise : un e-mail le dit au titulaire) ;
//! - limites d'essais ([`super::Limiter`]) ; jamais d'adresse ni de jeton dans les journaux.

use std::net::IpAddr;

use super::mail::{self, Message};
use super::pages::{self, AccountView, Forgot, Notice, SessionView, SignUp, View};
use super::store::{Account, CodeCheck, Session};
use super::{Accounts, Limited, b64, new_code, new_token, normalize_email, password, token_hash, totp, unb64};
use crate::pages::{Lang, t};

pub const SESSION_COOKIE: &str = "__Host-kn_session";
pub const CSRF_COOKIE: &str = "__Host-kn_csrf";
/// L'adresse en attente de vérification (signée).
pub const PENDING_COOKIE: &str = "__Host-kn_attente";
/// Le navigateur de confiance (« Ne plus demander pendant 30 jours »).
pub const TRUST_COOKIE: &str = "__Host-kn_confiance";

const DAY: i64 = 86_400;
/// Une session de la console.
pub const SESSION_SECS: i64 = 30 * DAY;
/// Une demi-session : mot de passe donné, code de double authentification attendu.
pub const HALF_SESSION_SECS: i64 = 10 * 60;
/// Le lien et le code de vérification.
pub const VERIFY_SECS: i64 = DAY;
/// Le lien du mot de passe oublié.
pub const RESET_SECS: i64 = 3600;
/// « Ne plus demander sur cet appareil pendant 30 jours ».
pub const TRUST_SECS: i64 = 30 * DAY;
/// Ajouter une passkey ou activer la double authentification : sans mot de passe dans ce temps
/// après l'ouverture de la session (REL-06).
pub const REAUTH_SECS: i64 = 15 * 60;
/// Un nouveau mot de passe retire les passkeys ajoutées depuis ce temps (REL-06).
pub const RESET_PASSKEY_SECS: i64 = 7 * DAY;
/// Les codes de secours donnés à l'activation.
pub const RECOVERY_CODES: usize = 10;

/// La méthode d'une requête.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Other,
}

/// Une requête vers `/compte/…`, déjà lue.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub cookies: Vec<(String, String)>,
    /// Le formulaire (`application/x-www-form-urlencoded`) d'un POST.
    pub form: Vec<(String, String)>,
    pub ip: IpAddr,
    pub lang: Lang,
    pub user_agent: String,
    /// L'envoi vient d'un autre site (`Sec-Fetch-Site`, `Origin`).
    pub cross_site: bool,
}

fn find<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

impl Request {
    pub fn query(&self, key: &str) -> Option<&str> {
        find(&self.query, key)
    }

    pub fn form(&self, key: &str) -> &str {
        find(&self.form, key).unwrap_or("")
    }

    pub fn cookie(&self, key: &str) -> Option<&str> {
        find(&self.cookies, key).filter(|v| !v.is_empty())
    }
}

/// La réponse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    /// La page, ou rien pour une redirection.
    pub html: Option<String>,
    /// Une redirection (303).
    pub location: Option<String>,
    /// Les `Set-Cookie`.
    pub cookies: Vec<String>,
    pub retry_after: Option<u64>,
    /// Un fichier à télécharger (l'export des données, lot 1b) au lieu d'une page.
    pub download: Option<Download>,
}

/// Un fichier JSON à télécharger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub filename: String,
    pub body: String,
}

/// Décode `a=b&c=d` (formulaire ou requête) : `+` et `%XX`.
pub fn parse_pairs(s: &str) -> Vec<(String, String)> {
    s.split('&')
        .filter(|kv| !kv.is_empty())
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Les cookies d'un en-tête `Cookie`.
pub fn parse_cookies(header: &str) -> Vec<(String, String)> {
    header
        .split(';')
        .filter_map(|c| {
            let (k, v) = c.trim().split_once('=')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

fn set_cookie(name: &str, value: &str, max_age: Option<i64>) -> String {
    let age = max_age.map(|a| format!("; Max-Age={a}")).unwrap_or_default();
    format!("{name}={value}; Path=/; Secure; HttpOnly; SameSite=Lax{age}")
}

fn clear_cookie(name: &str) -> String {
    set_cookie(name, "", Some(0))
}

/// Un nom court de l'appareil, d'après `User-Agent` (seul ce nom est gardé).
pub fn device_name(ua: &str) -> String {
    let browser = if ua.contains("Firefox/") {
        "Firefox"
    } else if ua.contains("Edg/") {
        "Edge"
    } else if ua.contains("OPR/") {
        "Opera"
    } else if ua.contains("Chrome/") || ua.contains("CriOS/") {
        "Chrome"
    } else if ua.contains("Safari/") {
        "Safari"
    } else {
        ""
    };
    let os = if ua.contains("iPhone") {
        "iPhone"
    } else if ua.contains("iPad") {
        "iPad"
    } else if ua.contains("Android") {
        "Android"
    } else if ua.contains("Windows") {
        "Windows"
    } else if ua.contains("Mac OS X") || ua.contains("Macintosh") {
        "Mac"
    } else if ua.contains("CrOS") {
        "ChromeOS"
    } else if ua.contains("Linux") {
        "Linux"
    } else {
        ""
    };
    match (browser, os) {
        ("", "") => String::new(),
        (b, "") => b.to_string(),
        ("", o) => o.to_string(),
        (b, o) => format!("{b}, {o}"),
    }
}

/// Le nom de l'appareil d'une app (`?appareil=iPad de Léa`) : 64 caractères au plus, sans caractère de contrôle.
fn appareil(req: &Request) -> Option<&str> {
    req.query("appareil").filter(|a| !a.trim().is_empty() && a.chars().count() <= 64 && !a.chars().any(char::is_control))
}

/// Le contexte d'une requête : les cookies à poser, le jeton CSRF.
struct Cx<'a> {
    acc: &'a Accounts,
    req: &'a Request,
    lang: Lang,
    now: i64,
    csrf_cookie: String,
    csrf: String,
    cookies: Vec<String>,
    /// Le défi des passkeys de la page (connexion, confirmation), lot 1b.
    challenge: String,
}

impl<'a> Cx<'a> {
    fn new(acc: &'a Accounts, req: &'a Request) -> Cx<'a> {
        let mut cookies = Vec::new();
        let csrf_cookie = match req.cookie(CSRF_COOKIE).filter(|c| c.len() == 43) {
            Some(c) => c.to_string(),
            None => {
                let c = b64(&super::random::<32>());
                cookies.push(set_cookie(CSRF_COOKIE, &c, None));
                c
            }
        };
        let csrf = acc.keys.sign(format!("csrf:{csrf_cookie}").as_bytes());
        let now = acc.now();
        let challenge = passkeys::challenge(&acc.keys, passkeys::GET_PURPOSE, &csrf_cookie, now);
        Cx { acc, req, lang: req.lang, now, csrf_cookie, csrf, cookies, challenge }
    }

    fn view(&self) -> View<'_> {
        View { lang: self.lang, csrf: &self.csrf, appareil: appareil(self.req), challenge: &self.challenge }
    }

    fn q(&self) -> String {
        self.view().q()
    }

    fn tr(&self, fr: &str, en: &str) -> String {
        t(self.lang, fr, en)
    }

    /// Le formulaire vient de cette page : même site, jeton CSRF signé.
    fn csrf_ok(&self) -> bool {
        !self.req.cross_site
            && self.req.cookie(CSRF_COOKIE) == Some(self.csrf_cookie.as_str())
            && self.acc.keys.check(format!("csrf:{}", self.csrf_cookie).as_bytes(), self.req.form("csrf"))
    }

    fn page(self, status: u16, html: String) -> Reply {
        Reply { status, html: Some(html), location: None, cookies: self.cookies, retry_after: None, download: None }
    }

    /// La base n'a pas répondu.
    fn failed(self) -> Reply {
        let lang = self.lang;
        self.page(500, crate::pages::not_answering(lang))
    }

    fn redirect(self, location: String) -> Reply {
        Reply { status: 303, html: None, location: Some(location), cookies: self.cookies, retry_after: None, download: None }
    }

    fn limited(self, wait: Limited, html: impl FnOnce(&Cx<'_>, Notice) -> String) -> Reply {
        let minutes = wait.0.as_secs().div_ceil(60);
        let page = html(&self, Notice::TooMany(minutes));
        let mut r = self.page(429, page);
        r.retry_after = Some(wait.0.as_secs().max(1));
        r
    }

    fn ip(&self) -> String {
        self.req.ip.to_canonical().to_string()
    }

    fn audit(&self, account: Option<i64>, kind: &str) {
        if let Err(e) = self.acc.db().audit(account, kind, Some(&self.ip()), self.now) {
            tracing::warn!("journal des comptes : {e:#}");
        }
    }

    /// La session du cookie (valide, compte existant).
    fn session(&self) -> Option<(Session, Account)> {
        let token = self.req.cookie(SESSION_COOKIE)?;
        let db = self.acc.db();
        let session = db.session(&token_hash(token), self.now).ok()??;
        let account = db.account(session.account).ok()??;
        Some((session, account))
    }

    /// Une session entière : adresse vérifiée, double authentification faite.
    fn full_session(&self) -> Option<(Session, Account)> {
        self.session().filter(|(s, a)| s.mfa_done && a.verified())
    }

    /// La session a été ouverte il y a moins de [`REAUTH_SECS`] (mot de passe, code ou passkey).
    fn recent_auth(&self) -> bool {
        self.session().is_some_and(|(s, _)| self.now - s.created_at < REAUTH_SECS)
    }

    /// Avant d'ajouter une passkey ou d'activer la double authentification (REL-06) : une session
    /// récente suffit ; sinon le mot de passe du formulaire (`password`), limité comme les codes.
    fn reauth(&self, account: &Account) -> Result<(), Notice> {
        if self.recent_auth() {
            return Ok(());
        }
        let key = format!("reauth:{}", account.id);
        let limit = self.acc.limits.lock().unwrap_or_else(|e| e.into_inner()).code(self.req.ip, &key);
        if let Err(wait) = limit {
            return Err(Notice::TooMany(wait.0.as_secs().div_ceil(60)));
        }
        if !self.acc.hasher.verify(self.req.form("password"), Some(&account.password)) {
            return Err(Notice::BadPassword);
        }
        self.acc.limits.lock().unwrap_or_else(|e| e.into_inner()).code_ok(self.req.ip, &key);
        Ok(())
    }

    /// Ouvre une session (et ferme celle du cookie, s'il y en a une).
    fn open_session(&mut self, account: i64, mfa_done: bool) {
        let db = self.acc.db();
        if let Some(old) = self.req.cookie(SESSION_COOKIE)
            && let Ok(Some(s)) = db.session(&token_hash(old), self.now)
        {
            let _ = db.delete_session(s.id, s.account);
        }
        let (token, hash) = new_token();
        let (life, max_age) = if mfa_done { (SESSION_SECS, Some(SESSION_SECS)) } else { (HALF_SESSION_SECS, None) };
        let device = device_name(&self.req.user_agent);
        if let Err(e) = db.create_session(&hash, account, &device, self.now, self.now + life, mfa_done) {
            tracing::warn!("session non ouverte : {e:#}");
            return;
        }
        drop(db);
        self.cookies.push(set_cookie(SESSION_COOKIE, &token, max_age));
    }

    /// L'adresse en attente de vérification (cookie signé, 24 h).
    fn pending(&self) -> Option<String> {
        let value = self.req.cookie(PENDING_COOKIE)?;
        let mut parts = value.split('.');
        let (email, exp, sig) = (parts.next()?, parts.next()?, parts.next()?);
        let data = format!("attente:{email}.{exp}");
        if !self.acc.keys.check(data.as_bytes(), sig) || exp.parse::<i64>().ok()? <= self.now {
            return None;
        }
        String::from_utf8(unb64(email)?).ok()
    }

    fn set_pending(&mut self, email: &str) {
        let email = b64(email.as_bytes());
        let exp = self.now + VERIFY_SECS;
        let sig = self.acc.keys.sign(format!("attente:{email}.{exp}").as_bytes());
        self.cookies.push(set_cookie(PENDING_COOKIE, &format!("{email}.{exp}.{sig}"), Some(VERIFY_SECS)));
    }

    fn mail(&self, to: &str, lang: Lang, message: Message<'_>) {
        if let Some(mailer) = &self.acc.mailer {
            mailer.send(mail::compose(&self.acc.public_url, to, lang, message));
        }
    }

    /// Un e-mail de plus vers cette adresse, dans la limite.
    fn may_mail(&self, to: &str) -> bool {
        self.acc.mailer.is_some() && self.acc.limits.lock().unwrap_or_else(|e| e.into_inner()).mail(to)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.acc.public_url)
    }

    /// Un nouveau code et un nouveau lien de vérification, envoyés.
    fn send_verification(&self, account: &Account) {
        if !self.may_mail(&account.email) {
            return;
        }
        let (token, hash) = new_token();
        let code = new_code();
        if self.acc.db().replace_email_token(account.id, "verify", &hash, Some(&code), self.now + VERIFY_SECS).is_err() {
            return;
        }
        let link = self.url(&format!("/compte/verifier?jeton={token}&lang={}", lang_of(account).code()));
        self.mail(&account.email, lang_of(account), Message::Verify { name: &account.name, code: &code, link: &link });
    }

    /// Le titulaire d'une adresse déjà prise est prévenu.
    fn send_taken(&self, email: &str) {
        if !self.may_mail(email) {
            return;
        }
        let lang = self.acc.db().account_by_email(email).ok().flatten().map(|a| lang_of(&a)).unwrap_or(self.lang);
        let q = format!("?lang={}", lang.code());
        let (sign_in, forgot) = (self.url(&format!("/compte/connexion{q}")), self.url(&format!("/compte/oubli{q}")));
        self.mail(email, lang, Message::SignupTaken { sign_in: &sign_in, forgot: &forgot });
    }
}

/// Le compte d'un jeton d'e-mail valide (sans le consommer).
fn account_by_token(acc: &Accounts, token: &str, purpose: &str, now: i64) -> Option<Account> {
    let db = acc.db();
    let id = db.email_token_account(&token_hash(token), purpose, now).ok().flatten()?;
    db.account(id).ok().flatten()
}

fn lang_of(account: &Account) -> Lang {
    if account.lang == "en" { Lang::En } else { Lang::Fr }
}

/// Les chemins servis par les comptes (`/tv` : le code d'un téléviseur, lot 3).
pub fn is_account_path(path: &str) -> bool {
    path == "/compte" || path.starts_with("/compte/") || path == "/tv"
}

impl Accounts {
    /// Sert une page des comptes.
    pub fn handle(&self, req: &Request) -> Reply {
        let mut cx = Cx::new(self, req);
        let path = req.path.trim_end_matches('/');
        let post = match req.method {
            Method::Get => false,
            Method::Post => true,
            Method::Other => return cx.page(405, crate::pages::not_found(req.lang)),
        };
        if post && !cx.csrf_ok() {
            let page = pages::message(
                &cx.view(),
                path,
                &cx.tr("Formulaire expiré", "Form expired"),
                &Notice::FormExpired.text(cx.lang),
                (path, &cx.tr("Recharger la page", "Reload the page")),
            );
            return cx.page(403, page);
        }
        // Un lien d'accès : `/compte/lien/kla_…` (le lien est dans le chemin).
        if let Some(token) = path.strip_prefix("/compte/lien/") {
            return link_pages::open(cx, token, post);
        }
        match (path, post) {
            ("/compte", false) => account_page(cx, None),
            ("/compte/connexion", false) => {
                // Depuis une app (lot 3) : sa demande est retenue pour après la connexion.
                let request = if appareil(req).is_some() {
                    match device_pages::remember_device_request(&mut cx) {
                        Some(path) => Some(path),
                        None => return device_pages::bad_return(cx),
                    }
                } else {
                    None
                };
                if cx.full_session().is_some() {
                    // Déjà connecté : cette demande-ci, tout de suite. La suite
                    // retenue par le cookie reçu peut être celle d'une demande
                    // abandonnée (autre `etat` : l'app refusait le retour).
                    if let Some(path) = request {
                        cx.cookies.push(device_pages::clear_next());
                        return cx.redirect(path);
                    }
                    return device_pages::after_sign_in(cx);
                }
                let page = pages::sign_in(&cx.view(), "", None, self.mailer.is_some(), self.mailer.is_some());
                cx.page(200, page)
            }
            ("/compte/connexion", true) => sign_in(cx),
            ("/compte/code", false) => code_page(cx),
            ("/compte/code", true) => code_post(cx),
            ("/compte/inscription", _) => sign_up(cx, post),
            ("/compte/verifier", false) => verify_get(cx),
            ("/compte/verifier", true) => verify_post(cx),
            ("/compte/proteger", false) => {
                let Some((_, account)) = cx.full_session() else { return to_sign_in(cx) };
                let on = totp_active(self, account.id);
                let register = cx.registration(&account);
                let page = pages::protect(&cx.view(), on, register.as_ref());
                cx.page(200, page)
            }
            ("/compte/oubli", _) => forgot(cx, post),
            ("/compte/nouveau-mot-de-passe", _) => new_password(cx, post),
            ("/compte/double-authentification", _) => totp_setup(cx, post),
            ("/compte/double-authentification/couper", true) => totp_off(cx, false),
            ("/compte/codes-de-secours", true) => totp_off(cx, true),
            ("/compte/sessions/fermer", true) => close_session(cx),
            // Lot 2 : rattacher une Station, la détacher.
            ("/compte/rattacher", _) => attach_pages::attach(cx, post),
            ("/compte/ordinateurs/detacher", true) => attach_pages::detach(cx),
            // Lot 4 : la console, les personnes et leurs droits.
            ("/compte/ordinateurs", false) => console::computers(cx, None),
            ("/compte/personnes", false) => console::all_people(cx),
            ("/compte/familles", false) => console::families(cx),
            ("/compte/bienvenue", false) => console::welcome(cx),
            ("/compte/bienvenue", true) => console::welcome_done(cx),
            ("/compte/ordinateur", false) => console::people(cx),
            ("/compte/ordinateur/ajouter", true) => console::add(cx),
            ("/compte/ordinateur/ouvrir", true) => console::open_to(cx),
            ("/compte/ordinateur/retirer", true) => console::remove(cx),
            ("/compte/ordinateur/renvoyer", true) => console::resend(cx),
            ("/compte/ordinateur/deplacer", true) => move_pages::deplacer(cx),
            // Liens d'accès et groupes d'ordinateurs (docs/comptes.md, « Regarder, liens d'accès et groupes d'ordinateurs »).
            ("/compte/ordinateur/liens/creer", true) => link_pages::create(cx),
            ("/compte/ordinateur/liens/revoquer", true) => link_pages::revoke(cx),
            // Lot 5 : les organisations.
            ("/compte/organisations/creer", _) => org_pages::create(cx, post),
            ("/compte/organisation", false) => org_pages::page(cx),
            ("/compte/organisation/renommer", true) => org_pages::rename(cx),
            ("/compte/organisation/inviter", true) => org_pages::invite(cx),
            ("/compte/organisation/role", true) => org_pages::set_role(cx),
            ("/compte/organisation/retirer", true) => org_pages::remove(cx),
            ("/compte/organisation/renvoyer", true) => org_pages::resend(cx),
            ("/compte/organisation/quitter", true) => org_pages::leave(cx),
            ("/compte/organisation/groupes/creer", true) => org_pages::group_create(cx),
            ("/compte/organisation/groupes/supprimer", true) => org_pages::group_delete(cx),
            ("/compte/organisation/groupes/membre", true) => org_pages::group_member(cx),
            ("/compte/organisation/groupes-ordinateurs/creer", true) => org_pages::sgroup_create(cx),
            ("/compte/organisation/groupes-ordinateurs/supprimer", true) => org_pages::sgroup_delete(cx),
            ("/compte/organisation/groupes-ordinateurs/ordinateur", true) => org_pages::sgroup_station(cx),
            ("/compte/organisation/groupes-ordinateurs/ouvrir", true) => org_pages::sgroup_open(cx),
            ("/compte/organisation/groupes-ordinateurs/retirer", true) => org_pages::sgroup_remove(cx),
            ("/compte/organisation/deployer", false) => org_pages::deploy(cx),
            ("/compte/organisation/jetons/creer", true) => org_pages::token_create(cx),
            ("/compte/organisation/jetons/revoquer", true) => org_pages::token_revoke(cx),
            ("/compte/ouvrir-admin", _) => admin_login::open_admin(cx, post),
            ("/compte/admin", false) => admin_login::start(cx),
            // Lot 3 : les appareils connectés.
            ("/compte/connecter-appareil", _) => device_pages::consent(cx, post),
            ("/compte/appareil", _) => device_pages::approve(cx, post),
            ("/compte/appareils/deconnecter", true) => device_pages::revoke(cx),
            ("/tv", false) => device_pages::tv(cx),
            // Lot 1b : passkeys, suppression du compte, export des données.
            ("/compte/connexion/passkey", true) => passkeys::sign_in(cx),
            ("/compte/passkeys/ajouter", true) => passkeys::add(cx),
            ("/compte/passkeys/renommer", true) => passkeys::rename(cx),
            ("/compte/passkeys/retirer", true) => passkeys::remove(cx),
            ("/compte/supprimer", _) => data::delete(cx, post),
            ("/compte/export", true) => data::export(cx),
            ("/compte/deconnexion", true) => {
                if let Some((s, a)) = cx.session() {
                    let _ = self.db().delete_session(s.id, a.id);
                    cx.audit(Some(a.id), "deconnexion");
                }
                cx.cookies.push(clear_cookie(SESSION_COOKIE));
                let q = cx.q();
                cx.redirect(format!("/compte/connexion{q}"))
            }
            _ => cx.page(404, crate::pages::not_found(req.lang)),
        }
    }
}

fn to_sign_in(cx: Cx<'_>) -> Reply {
    let q = cx.q();
    cx.redirect(format!("/compte/connexion{q}"))
}

fn totp_active(acc: &Accounts, account: i64) -> bool {
    acc.db().totp(account).ok().flatten().is_some_and(|t| t.activated_at.is_some())
}

fn account_page(cx: Cx<'_>, notice: Option<Notice>) -> Reply {
    let Some((session, account)) = cx.full_session() else {
        // Mot de passe donné, code attendu : la double authentification d'abord.
        if cx.session().is_some_and(|(s, _)| !s.mfa_done) {
            let q = cx.q();
            return cx.redirect(format!("/compte/code{q}"));
        }
        return to_sign_in(cx);
    };
    let view = {
        let db = cx.acc.db();
        if cx.now - session.seen_at > 300 {
            let _ = db.touch_session(session.id, cx.now);
        }
        let totp_on = db.totp(account.id).ok().flatten().is_some_and(|t| t.activated_at.is_some());
        let sessions = db
            .sessions(account.id, cx.now)
            .unwrap_or_default()
            .into_iter()
            .map(|s| SessionView {
                id: s.id,
                device: if s.device.is_empty() { t(cx.lang, "Navigateur", "Browser") } else { s.device.clone() },
                created_at: s.created_at,
                seen_at: s.seen_at.max(if s.id == session.id { cx.now } else { 0 }),
                current: s.id == session.id,
            })
            .collect();
        AccountView {
            email: account.email.clone(),
            name: account.name.clone(),
            totp_on,
            recovery_left: db.recovery_codes_left(account.id).unwrap_or(0),
            sessions,
            computers: attach_pages::computers(&db, &cx.acc.stations, account.id),
            devices: device_pages::devices(&db, account.id, cx.now),
            passkeys: Vec::new(),
            register: None,
            orgs: db.orgs_of(account.id).unwrap_or_default().into_iter().map(|(o, _)| (o.public_id, o.name)).collect(),
            now: cx.now,
        }
    };
    let view = AccountView { passkeys: passkeys::views(&cx, account.id), register: cx.registration(&account), ..view };
    let shell = console::shell(&cx, &account);
    let page = pages::account(&cx.view(), &view, &shell, notice);
    cx.page(200, page)
}

// --- Connexion ----------------------------------------------------------------

fn sign_in(mut cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let mail_on = acc.mailer.is_some();
    let typed = cx.req.form("email").trim().to_string();
    let email = normalize_email(&typed).unwrap_or_else(|| typed.to_lowercase());
    let password = cx.req.form("password").to_string();
    let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).sign_in(cx.req.ip, &email);
    if let Err(wait) = limit {
        return cx.limited(wait, |cx, n| pages::sign_in(&cx.view(), &typed, Some(n), mail_on, mail_on));
    }
    let account = acc.db().account_by_email(&email).ok().flatten();
    // Toujours un calcul d'Argon2 : le temps ne dit pas si le compte existe.
    let ok = acc.hasher.verify(&password, account.as_ref().map(|a| a.password.as_str()));
    if ok {
        acc.limits.lock().unwrap_or_else(|e| e.into_inner()).sign_in_ok(cx.req.ip, &email);
    }
    let Some(account) = account.filter(|_| ok) else {
        let known = acc.db().account_by_email(&email).ok().flatten();
        if let Some(a) = known {
            cx.audit(Some(a.id), "connexion_refusee");
        }
        let page = pages::sign_in(&cx.view(), &typed, Some(Notice::BadCredentials), mail_on, mail_on);
        return cx.page(200, page);
    };
    if !account.verified() {
        if !mail_on {
            let page = pages::sign_in(&cx.view(), &typed, Some(Notice::UnverifiedNoMail), mail_on, mail_on);
            return cx.page(200, page);
        }
        cx.send_verification(&account);
        cx.set_pending(&account.email);
        let q = cx.q();
        return cx.redirect(format!("/compte/verifier{q}"));
    }
    let trusted = cx.req.cookie(TRUST_COOKIE).is_some_and(|tok| acc.db().is_trusted(&token_hash(tok), account.id, cx.now).unwrap_or(false));
    let needs_code = totp_active(acc, account.id) && !trusted;
    cx.open_session(account.id, !needs_code);
    let q = cx.q();
    if needs_code {
        cx.audit(Some(account.id), "connexion_code_attendu");
        return cx.redirect(format!("/compte/code{q}"));
    }
    cx.audit(Some(account.id), "connexion");
    // Lot 3 : avec `?appareil=` ou une suite retenue (rattachement), on y va.
    device_pages::after_sign_in(cx)
}

fn code_page(cx: Cx<'_>) -> Reply {
    match cx.session() {
        None => to_sign_in(cx),
        Some((s, _)) if s.mfa_done => {
            let q = cx.q();
            cx.redirect(format!("/compte{q}"))
        }
        Some(_) => {
            let recovery = cx.req.query("secours").is_some();
            let page = pages::code(&cx.view(), None, recovery);
            cx.page(200, page)
        }
    }
}

/// Un code de secours, tel que tapé : minuscules, sans tiret ni espace.
fn normalize_recovery(code: &str) -> String {
    code.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect()
}

fn code_post(mut cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let Some((session, account)) = cx.session() else { return to_sign_in(cx) };
    if session.mfa_done {
        let q = cx.q();
        return cx.redirect(format!("/compte{q}"));
    }
    let recovery = !cx.req.form("secours").is_empty();
    let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).code(cx.req.ip, &format!("totp:{}", account.id));
    if let Err(wait) = limit {
        return cx.limited(wait, |cx, n| pages::code(&cx.view(), Some(n), recovery));
    }
    let ok = if recovery {
        let code = normalize_recovery(cx.req.form("secours"));
        acc.db().use_recovery_code(account.id, &token_hash(&code), cx.now).unwrap_or(false)
    } else {
        let totp = acc.db().totp(account.id).ok().flatten().filter(|t| t.activated_at.is_some());
        match totp {
            // Plus de double authentification (coupée entre-temps) : le mot de passe suffit.
            None => true,
            Some(t) => match acc.keys.open(account.id, &t.secret) {
                Some(secret) => match totp::verify(&secret, cx.req.form("code"), cx.now, t.last_step) {
                    Some(step) => acc.db().use_totp_step(account.id, step).unwrap_or(false),
                    None => false,
                },
                // Le secret ne se déchiffre plus (clé des comptes changée ?) : refusé, jamais ouvert (REL-07).
                None => {
                    tracing::error!(compte = account.id, "secret TOTP illisible : la clé des comptes a changé ? (code de secours seulement)");
                    false
                }
            },
        }
    };
    if !ok {
        cx.audit(Some(account.id), if recovery { "code_de_secours_refuse" } else { "code_refuse" });
        let page = pages::code(&cx.view(), Some(Notice::WrongCode), recovery);
        return cx.page(200, page);
    }
    acc.limits.lock().unwrap_or_else(|e| e.into_inner()).code_ok(cx.req.ip, &format!("totp:{}", account.id));
    cx.audit(Some(account.id), if recovery { "connexion_code_de_secours" } else { "connexion" });
    if cx.req.form("confiance") == "1" {
        let (token, hash) = new_token();
        if acc.db().trust_browser(&hash, account.id, cx.now + TRUST_SECS).is_ok() {
            cx.cookies.push(set_cookie(TRUST_COOKIE, &token, Some(TRUST_SECS)));
        }
    }
    // Un nouveau jeton pour la session entière ; la demi-session est fermée.
    cx.open_session(account.id, true);
    device_pages::after_sign_in(cx)
}

// --- Inscription et vérification ---------------------------------------------

fn sign_up(mut cx: Cx<'_>, post: bool) -> Reply {
    let acc = cx.acc;
    let state = if appareil(cx.req).is_some() {
        SignUp::FromApp
    } else if acc.mailer.is_none() {
        SignUp::Closed
    } else {
        SignUp::Open
    };
    if !post || state != SignUp::Open {
        let status = if post { 403 } else { 200 };
        let page = pages::sign_up(&cx.view(), state, "", "", None);
        return cx.page(status, page);
    }
    let name: String = cx.req.form("name").trim().chars().take(80).collect();
    let typed = cx.req.form("email").trim().to_string();
    let password = cx.req.form("password").to_string();
    let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).sign_up(cx.req.ip);
    if let Err(wait) = limit {
        return cx.limited(wait, |cx, n| pages::sign_up(&cx.view(), SignUp::Open, &name, &typed, Some(n)));
    }
    let refuse = |cx: Cx<'_>, n: Notice| {
        let page = pages::sign_up(&cx.view(), SignUp::Open, &name, &typed, Some(n));
        cx.page(200, page)
    };
    if name.is_empty() {
        return refuse(cx, Notice::NameMissing);
    }
    // Lot 1b : la case « conditions et confidentialité ».
    if cx.req.form("conditions") != "1" {
        return refuse(cx, Notice::TermsMissing);
    }
    let Some(email) = normalize_email(&typed) else { return refuse(cx, Notice::BadEmail) };
    if let Err(weak) = password::check_new(&password, &email) {
        return refuse(cx, Notice::Weak(weak));
    }
    let hash = acc.hasher.hash(&password);
    let created = acc.db().create_account(&email, &name, &hash, cx.lang.code(), None, cx.now);
    match created {
        Ok(Some(id)) => {
            cx.audit(Some(id), "inscription");
            let (terms, privacy) = (crate::legal::Doc::Terms.version(), crate::legal::Doc::Privacy.version());
            if let Err(e) = acc.db().set_terms(id, terms, privacy, cx.now) {
                tracing::warn!("conditions acceptées non notées : {e:#}");
            }
            let account = acc.db().account(id).ok().flatten();
            if let Some(account) = account {
                cx.send_verification(&account);
            }
        }
        // Adresse déjà prise : même réponse, et un e-mail au titulaire.
        Ok(None) => cx.send_taken(&email),
        Err(e) => {
            tracing::warn!("inscription : {e:#}");
            return cx.failed();
        }
    }
    cx.set_pending(&email);
    let q = cx.q();
    cx.redirect(format!("/compte/verifier{q}"))
}

/// L'adresse est vérifiée : la session s'ouvre si ce navigateur attendait cette adresse.
fn verified(mut cx: Cx<'_>, account: &Account) -> Reply {
    let db = cx.acc.db();
    let _ = db.set_verified(account.id, cx.now);
    let _ = db.delete_email_tokens(account.id, "verify");
    drop(db);
    cx.audit(Some(account.id), "adresse_verifiee");
    let q = cx.q();
    if cx.pending().as_deref() == Some(account.email.as_str()) {
        cx.cookies.push(clear_cookie(PENDING_COOKIE));
        cx.open_session(account.id, true);
        // Une page retenue avant l'inscription (un lien d'accès) : on y revient.
        if let Some(next) = device_pages::take_next(&mut cx) {
            return cx.redirect(next);
        }
        // L'espace s'ouvre sur le tour d'accueil (docs/comptes.md, « L'espace du compte »).
        return cx.redirect(format!("/compte/bienvenue{q}"));
    }
    let page = pages::sign_in(&cx.view(), &account.email, Some(Notice::Verified), true, true);
    cx.page(200, page)
}

fn verify_get(cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    if let Some(token) = cx.req.query("jeton") {
        let account = account_by_token(acc, token, "verify", cx.now);
        return match account {
            Some(account) => verified(cx, &account),
            None => {
                let page = pages::message(
                    &cx.view(),
                    "/compte/verifier",
                    &cx.tr("Lien expiré", "Link expired"),
                    &cx.tr(
                        "Ce lien a déjà servi ou ne vaut plus. Connectez-vous : un nouvel e-mail partira si l'adresse n'est pas encore vérifiée.",
                        "This link was already used or is no longer valid. Sign in: a new email will be sent if the address isn't verified yet.",
                    ),
                    ("/compte/connexion", &cx.tr("Se connecter", "Sign in")),
                );
                cx.page(400, page)
            }
        };
    }
    let Some(email) = cx.pending() else { return to_sign_in(cx) };
    let change = cx.req.query("changer").is_some();
    let page = pages::verify(&cx.view(), &email, None, change);
    cx.page(200, page)
}

fn verify_post(mut cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let Some(email) = cx.pending() else { return to_sign_in(cx) };
    let unverified = || acc.db().account_by_email(&email).ok().flatten().filter(|a| !a.verified());
    match cx.req.form("action") {
        "renvoyer" => {
            if let Some(account) = unverified() {
                cx.send_verification(&account);
            }
            let page = pages::verify(&cx.view(), &email, Some(Notice::Resent), false);
            cx.page(200, page)
        }
        "changer" => {
            let Some(new_email) = normalize_email(cx.req.form("email")) else {
                let page = pages::verify(&cx.view(), &email, Some(Notice::BadEmail), true);
                return cx.page(200, page);
            };
            let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).sign_up(cx.req.ip);
            if let Err(wait) = limit {
                return cx.limited(wait, |cx, n| pages::verify(&cx.view(), &email, Some(n), true));
            }
            if let Some(account) = unverified() {
                let changed = acc.db().set_email(account.id, &new_email).unwrap_or(false);
                if changed {
                    cx.audit(Some(account.id), "adresse_changee");
                    let account = acc.db().account(account.id).ok().flatten();
                    if let Some(account) = account {
                        cx.send_verification(&account);
                    }
                } else {
                    cx.send_taken(&new_email);
                }
            }
            cx.set_pending(&new_email);
            let page = pages::verify(&cx.view(), &new_email, Some(Notice::Resent), false);
            cx.page(200, page)
        }
        _ => {
            let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).code(cx.req.ip, &format!("verify:{email}"));
            if let Err(wait) = limit {
                return cx.limited(wait, |cx, n| pages::verify(&cx.view(), &email, Some(n), false));
            }
            let code: String = cx.req.form("code").chars().filter(char::is_ascii_digit).collect();
            let account = unverified();
            let check = match &account {
                Some(a) => acc.db().check_email_code(a.id, "verify", &code, cx.now).unwrap_or(CodeCheck::None),
                None => CodeCheck::Wrong,
            };
            match (check, account) {
                (CodeCheck::Ok, Some(account)) => {
                    acc.limits.lock().unwrap_or_else(|e| e.into_inner()).code_ok(cx.req.ip, &format!("verify:{email}"));
                    verified(cx, &account)
                }
                (CodeCheck::None, _) => {
                    let page = pages::verify(&cx.view(), &email, Some(Notice::CodeGone), false);
                    cx.page(200, page)
                }
                _ => {
                    let page = pages::verify(&cx.view(), &email, Some(Notice::WrongCode), false);
                    cx.page(200, page)
                }
            }
        }
    }
}

// --- Mot de passe oublié -----------------------------------------------------

fn forgot(cx: Cx<'_>, post: bool) -> Reply {
    let acc = cx.acc;
    if acc.mailer.is_none() {
        let page = pages::forgot(&cx.view(), Forgot::Unavailable);
        return cx.page(200, page);
    }
    if !post {
        let page = pages::forgot(&cx.view(), Forgot::Form(None));
        return cx.page(200, page);
    }
    let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).forgot(cx.req.ip);
    if let Err(wait) = limit {
        return cx.limited(wait, |cx, n| pages::forgot(&cx.view(), Forgot::Form(Some(n))));
    }
    let Some(email) = normalize_email(cx.req.form("email")) else {
        let page = pages::forgot(&cx.view(), Forgot::Form(Some(Notice::BadEmail)));
        return cx.page(200, page);
    };
    let account = acc.db().account_by_email(&email).ok().flatten();
    if let Some(account) = account
        && cx.may_mail(&account.email)
    {
        let (token, hash) = new_token();
        if acc.db().replace_email_token(account.id, "reset", &hash, None, cx.now + RESET_SECS).is_ok() {
            let lang = lang_of(&account);
            let link = cx.url(&format!("/compte/nouveau-mot-de-passe?jeton={token}&lang={}", lang.code()));
            cx.mail(&account.email, lang, Message::Reset { link: &link });
            cx.audit(Some(account.id), "mot_de_passe_oublie");
        }
    }
    let page = pages::forgot(&cx.view(), Forgot::Sent);
    cx.page(200, page)
}

fn new_password(mut cx: Cx<'_>, post: bool) -> Reply {
    let acc = cx.acc;
    let token = if post { cx.req.form("jeton").to_string() } else { cx.req.query("jeton").unwrap_or("").to_string() };
    let account = account_by_token(acc, &token, "reset", cx.now);
    let Some(account) = account.filter(|_| !token.is_empty()) else {
        let page = pages::message(
            &cx.view(),
            "/compte/oubli",
            &cx.tr("Lien expiré", "Link expired"),
            &cx.tr("Ce lien a déjà servi ou ne vaut plus : demandez-en un autre.", "This link was already used or is no longer valid: ask for another one."),
            ("/compte/oubli", &cx.tr("Mot de passe oublié", "Forgot your password")),
        );
        return cx.page(400, page);
    };
    if !post {
        let page = pages::new_password(&cx.view(), &token, None);
        return cx.page(200, page);
    }
    let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).forgot(cx.req.ip);
    if let Err(wait) = limit {
        return cx.limited(wait, |cx, n| pages::new_password(&cx.view(), &token, Some(n)));
    }
    let password = cx.req.form("password");
    if let Err(weak) = password::check_new(password, &account.email) {
        let page = pages::new_password(&cx.view(), &token, Some(Notice::Weak(weak)));
        return cx.page(200, page);
    }
    let hash = acc.hasher.hash(password);
    {
        let db = acc.db();
        let _ = db.set_password(account.id, &hash);
        let _ = db.delete_email_tokens(account.id, "reset");
        // Le lien prouve l'adresse.
        let _ = db.set_verified(account.id, cx.now);
        // Un nouveau mot de passe ferme les sessions et oublie les navigateurs de confiance.
        let _ = db.delete_sessions(account.id, None);
        // Les appareils connectés aussi (lot 3).
        let _ = db.revoke_devices(account.id);
        let _ = db.forget_trusted_browsers(account.id);
        // Les passkeys ajoutées ces derniers jours partent aussi : un cookie volé a pu en ajouter une (REL-06).
        let _ = db.delete_passkeys_since(account.id, cx.now - RESET_PASSKEY_SECS);
    }
    cx.audit(Some(account.id), "mot_de_passe_change");
    let forgot_link = cx.url(&format!("/compte/oubli?lang={}", lang_of(&account).code()));
    cx.mail(&account.email, lang_of(&account), Message::PasswordChanged { forgot: &forgot_link });
    cx.cookies.push(clear_cookie(SESSION_COOKIE));
    cx.cookies.push(clear_cookie(TRUST_COOKIE));
    let page = pages::sign_in(&cx.view(), &account.email, Some(Notice::PasswordChanged), true, true);
    cx.page(200, page)
}

// --- Double authentification -------------------------------------------------

fn totp_setup(cx: Cx<'_>, post: bool) -> Reply {
    let acc = cx.acc;
    let Some((_, account)) = cx.full_session() else { return to_sign_in(cx) };
    let current = acc.db().totp(account.id).ok().flatten();
    if current.as_ref().is_some_and(|t| t.activated_at.is_some()) {
        let q = cx.q();
        return cx.redirect(format!("/compte{q}"));
    }
    // Un secret en attente garde le même QR code d'un essai à l'autre.
    let secret = match current.and_then(|t| acc.keys.open(account.id, &t.secret)) {
        Some(s) => s,
        None => {
            let s = super::random::<{ totp::SECRET_LEN }>().to_vec();
            if acc.db().set_pending_totp(account.id, &acc.keys.seal(account.id, &s)).is_err() {
                return cx.failed();
            }
            s
        }
    };
    let show = |cx: Cx<'_>, notice: Option<Notice>| {
        let uri = totp::otpauth_uri(&secret, &acc.issuer(), &account.email);
        let qr = qr_svg(&uri);
        let page = pages::totp_setup(&cx.view(), &qr, &totp::grouped(&totp::base32(&secret)), &uri, !cx.recent_auth(), notice);
        cx.page(200, page)
    };
    if !post {
        return show(cx, None);
    }
    let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).code(cx.req.ip, &format!("totp:{}", account.id));
    if let Err(wait) = limit {
        let minutes = wait.0.as_secs().div_ceil(60);
        let mut r = show(cx, Some(Notice::TooMany(minutes)));
        r.status = 429;
        r.retry_after = Some(wait.0.as_secs().max(1));
        return r;
    }
    let Some(step) = totp::verify(&secret, cx.req.form("code"), cx.now, 0) else { return show(cx, Some(Notice::WrongCode)) };
    // Une session ancienne (cookie volé ?) redonne le mot de passe avant d'activer (REL-06).
    if let Err(notice) = cx.reauth(&account) {
        return show(cx, Some(notice));
    }
    acc.limits.lock().unwrap_or_else(|e| e.into_inner()).code_ok(cx.req.ip, &format!("totp:{}", account.id));
    let codes = new_recovery_codes();
    {
        let mut db = acc.db();
        if db.activate_totp(account.id, step, cx.now).is_err() || db.replace_recovery_codes(account.id, &codes.iter().map(|c| token_hash(&normalize_recovery(c))).collect::<Vec<_>>()).is_err() {
            return cx.failed();
        }
    }
    cx.audit(Some(account.id), "double_authentification_activee");
    let link = cx.url(&format!("/compte?lang={}", lang_of(&account).code()));
    cx.mail(&account.email, lang_of(&account), Message::TotpChanged { on: true, account: &link });
    let page = pages::recovery_codes(&cx.view(), &codes, Some(Notice::TotpOn));
    cx.page(200, page)
}

/// Dix codes de secours : `xxxx-xxxx`, 40 bits chacun (base32 en minuscules).
fn new_recovery_codes() -> Vec<String> {
    (0..RECOVERY_CODES)
        .map(|_| {
            let s = totp::base32(&super::random::<5>()).to_lowercase();
            format!("{}-{}", &s[..4], &s[4..])
        })
        .collect()
}

/// Le QR code en SVG, écrit dans la page (aucun fichier, aucun script).
fn qr_svg(uri: &str) -> String {
    match qrcode::QrCode::new(uri.as_bytes()) {
        Ok(code) => {
            let svg = code
                .render::<qrcode::render::svg::Color<'_>>()
                .min_dimensions(200, 200)
                .dark_color(qrcode::render::svg::Color("#05070A"))
                .light_color(qrcode::render::svg::Color("#FFFFFF"))
                .build();
            // Sans la déclaration XML : le SVG est dans la page.
            match svg.find("<svg") {
                Some(i) => svg[i..].to_string(),
                None => svg,
            }
        }
        Err(_) => String::new(),
    }
}

/// Couper la double authentification (`codes = false`) ou refaire les codes de secours, avec le mot de passe.
fn totp_off(cx: Cx<'_>, codes: bool) -> Reply {
    let acc = cx.acc;
    let Some((_, account)) = cx.full_session() else { return to_sign_in(cx) };
    if !totp_active(acc, account.id) {
        let q = cx.q();
        return cx.redirect(format!("/compte{q}"));
    }
    let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).sign_in(cx.req.ip, &account.email);
    if let Err(wait) = limit {
        let minutes = wait.0.as_secs().div_ceil(60);
        let mut r = account_page(cx, Some(Notice::TooMany(minutes)));
        r.status = 429;
        r.retry_after = Some(wait.0.as_secs().max(1));
        return r;
    }
    if !acc.hasher.verify(cx.req.form("password"), Some(&account.password)) {
        return account_page(cx, Some(Notice::BadPassword));
    }
    acc.limits.lock().unwrap_or_else(|e| e.into_inner()).sign_in_ok(cx.req.ip, &account.email);
    if codes {
        let list = new_recovery_codes();
        if acc.db().replace_recovery_codes(account.id, &list.iter().map(|c| token_hash(&normalize_recovery(c))).collect::<Vec<_>>()).is_err() {
            return cx.failed();
        }
        cx.audit(Some(account.id), "codes_de_secours_refaits");
        let page = pages::recovery_codes(&cx.view(), &list, None);
        return cx.page(200, page);
    }
    let _ = acc.db().delete_totp(account.id);
    cx.audit(Some(account.id), "double_authentification_coupee");
    let link = cx.url(&format!("/compte?lang={}", lang_of(&account).code()));
    cx.mail(&account.email, lang_of(&account), Message::TotpChanged { on: false, account: &link });
    account_page(cx, Some(Notice::TotpOff))
}

fn close_session(cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let Some((current, account)) = cx.full_session() else { return to_sign_in(cx) };
    let Ok(id) = cx.req.form("id").parse::<i64>() else { return account_page(cx, None) };
    if acc.db().delete_session(id, account.id).unwrap_or(false) {
        cx.audit(Some(account.id), "session_fermee");
    }
    if id == current.id {
        return to_sign_in(cx);
    }
    account_page(cx, Some(Notice::SessionClosed))
}

mod admin_login;
mod attach_pages;
mod console;
mod device_pages;
mod link_pages;
mod move_pages;
mod org_pages;
// Lot 1b : des modules enfants, pour se servir du contexte ([`Cx`]).
#[path = "passkeys.rs"]
mod passkeys;

#[path = "data.rs"]
mod data;

#[cfg(test)]
mod tests;
