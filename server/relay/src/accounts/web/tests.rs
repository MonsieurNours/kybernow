//! Les parcours des comptes, sans réseau : un ou plusieurs navigateurs (leurs
//! cookies), une base en mémoire, une boîte d'e-mails en mémoire, une horloge
//! réglable.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use super::*;
use crate::accounts::mail::Outbox;
use crate::accounts::password::Hasher;
use crate::accounts::store::Store;
use crate::accounts::{Clock, Keys};

const T0: i64 = 1_795_392_000;
const PASSWORD: &str = "cheval agrafe pile";

struct World {
    acc: Arc<Accounts>,
    outbox: Arc<Outbox>,
    clock: Arc<AtomicI64>,
}

fn world(smtp: bool) -> World {
    let clock = Arc::new(AtomicI64::new(T0));
    let c = clock.clone();
    let clock_fn: Clock = Arc::new(move || c.load(Ordering::Relaxed));
    let outbox = Arc::new(Outbox::default());
    let mailer: Option<Arc<dyn crate::accounts::mail::Mailer>> = if smtp { Some(outbox.clone()) } else { None };
    let acc = Accounts::with(Store::memory(T0).unwrap(), Keys::new(&[9u8; 32]), mailer, "https://share.test".into(), Hasher::fast(), clock_fn);
    World { acc: Arc::new(acc), outbox, clock }
}

impl World {
    fn browser(&self) -> Browser {
        self.browser_at("192.0.2.10")
    }

    fn browser_at(&self, ip: &str) -> Browser {
        Browser { acc: self.acc.clone(), jar: HashMap::new(), ip: ip.parse().unwrap(), cross_site: false }
    }

    fn advance(&self, secs: i64) {
        self.clock.fetch_add(secs, Ordering::Relaxed);
    }

    fn now(&self) -> i64 {
        self.clock.load(Ordering::Relaxed)
    }

    /// Le dernier e-mail vers cette adresse.
    fn mail(&self, to: &str) -> mail::Mail {
        self.outbox.last_to(to).unwrap_or_else(|| panic!("pas d'e-mail pour {to}"))
    }

    /// Le code TOTP courant d'un compte.
    fn totp_code(&self, email: &str, offset_steps: i64) -> String {
        let db = self.acc.db();
        let account = db.account_by_email(email).unwrap().unwrap();
        let sealed = db.totp(account.id).unwrap().unwrap().secret;
        let secret = self.acc.keys.open(account.id, &sealed).unwrap();
        totp::code(&secret, totp::step_at(self.now()) + offset_steps)
    }
}

/// Un navigateur : ses cookies.
struct Browser {
    acc: Arc<Accounts>,
    jar: HashMap<String, String>,
    ip: IpAddr,
    cross_site: bool,
}

fn after<'a>(text: &'a str, marker: &str) -> &'a str {
    let i = text.find(marker).unwrap_or_else(|| panic!("« {marker} » absent"));
    &text[i + marker.len()..]
}

/// Ce qui suit `marker`, jusqu'à `end`.
fn between<'a>(text: &'a str, marker: &str, end: &str) -> &'a str {
    let rest = after(text, marker);
    &rest[..rest.find(end).unwrap()]
}

impl Browser {
    fn request(&mut self, method: Method, target: &str, form: &[(&str, &str)]) -> Reply {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let mut form: Vec<(String, String)> = form.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        if method == Method::Post && !form.iter().any(|(k, _)| k == "csrf") {
            if !self.jar.contains_key(CSRF_COOKIE) {
                self.request(Method::Get, "/compte/connexion", &[]);
            }
            let csrf = self.acc.keys.sign(format!("csrf:{}", self.jar[CSRF_COOKIE]).as_bytes());
            form.push(("csrf".into(), csrf));
        }
        let query = parse_pairs(query);
        let lang = Lang::of_request(Some(&query.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&")), Some("fr-FR"));
        let req = Request {
            method,
            path: path.into(),
            query,
            cookies: self.jar.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            form,
            ip: self.ip,
            lang,
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0".into(),
            cross_site: self.cross_site,
        };
        let reply = self.acc.handle(&req);
        for c in &reply.cookies {
            let (pair, attrs) = c.split_once(';').unwrap();
            let (name, value) = pair.split_once('=').unwrap();
            if attrs.contains("Max-Age=0") {
                self.jar.remove(name);
            } else {
                self.jar.insert(name.into(), value.into());
            }
        }
        reply
    }

    fn get(&mut self, target: &str) -> Reply {
        self.request(Method::Get, target, &[])
    }

    fn post(&mut self, target: &str, form: &[(&str, &str)]) -> Reply {
        self.request(Method::Post, target, form)
    }

    fn sign_in(&mut self, email: &str, password: &str) -> Reply {
        self.post("/compte/connexion?lang=fr", &[("email", email), ("password", password)])
    }
}

fn html(r: &Reply) -> &str {
    r.html.as_deref().unwrap_or_else(|| panic!("pas de page ({} → {:?})", r.status, r.location))
}

fn location(r: &Reply) -> &str {
    assert_eq!(r.status, 303, "redirection attendue : {:?}", r.html.as_deref().map(|h| &h[h.len().saturating_sub(600)..]));
    r.location.as_deref().unwrap()
}

#[test]
fn inscription_verification_connexion_et_double_authentification() {
    let w = world(true);
    let mut b = w.browser();
    let page = b.get("/compte/inscription?lang=fr");
    assert_eq!(page.status, 200);
    assert!(html(&page).contains("Créer un compte"));
    // Mot de passe trop connu : refusé, les champs restent.
    let r = b.post("/compte/inscription?lang=fr", &[("name", "Ana"), ("email", "Ana@Exemple.org"), ("password", "motdepasse123")]);
    assert!(html(&r).contains("trop connu") && html(&r).contains(r#"value="Ana@Exemple.org""#));
    assert_eq!(w.outbox.0.lock().unwrap().len(), 0);
    let r = b.post("/compte/inscription?lang=fr", &[("name", "Ana"), ("email", "Ana@Exemple.org"), ("password", PASSWORD)]);
    assert_eq!(location(&r), "/compte/verifier?lang=fr");
    assert!(b.jar.contains_key(PENDING_COOKIE) && !b.jar.contains_key(SESSION_COOKIE));
    // L'e-mail : le code et le lien (24 h), en français.
    let mail = w.mail("ana@exemple.org");
    assert!(mail.subject.starts_with("Votre code KyberNow"));
    assert!(mail.text.contains("https://share.test/compte/verifier?jeton="));
    let code: String = between(&mail.text, "    ", "\n").chars().filter(char::is_ascii_digit).collect();
    assert_eq!(code.len(), 6);
    let page = b.get("/compte/verifier?lang=fr");
    assert!(html(&page).contains("<b>ana@exemple.org</b>"));
    // Adresse pas encore vérifiée : la connexion renvoie vers la vérification (et un nouvel e-mail).
    assert_eq!(location(&b.sign_in("ana@exemple.org", PASSWORD)), "/compte/verifier?lang=fr");
    let code: String = between(&w.mail("ana@exemple.org").text, "    ", "\n").chars().filter(char::is_ascii_digit).collect();
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let r = b.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", wrong)]);
    assert!(html(&r).contains("Code incorrect"));
    let r = b.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", &format!("{} {}", &code[..3], &code[3..]))]);
    assert_eq!(location(&r), "/compte/proteger?lang=fr", "« 123 456 » accepté");
    assert!(!b.jar.contains_key(PENDING_COOKIE));
    let protect = b.get("/compte/proteger?lang=fr");
    assert!(html(&protect).contains("Protégez votre compte") && html(&protect).contains("/compte/double-authentification?lang=fr"));
    let page = b.get("/compte?lang=fr");
    assert!(html(&page).contains("ana@exemple.org") && html(&page).contains("Firefox, Windows") && html(&page).contains("Cette session"));

    // Activer la double authentification : QR code, clé, code ; puis les codes de secours.
    let setup = b.get("/compte/double-authentification?lang=fr");
    let page = html(&setup);
    assert!(page.contains("<svg") && page.contains("otpauth://totp/KyberNow%20%28share.test%29:ana%40exemple.org?secret="));
    assert!(!page.contains("<?xml"));
    let again = b.get("/compte/double-authentification?lang=fr");
    assert_eq!(between(page, "class=\"secret\">", "<"), between(html(&again), "class=\"secret\">", "<"), "même secret d'un essai à l'autre");
    let r = b.post("/compte/double-authentification?lang=fr", &[("code", "123")]);
    assert!(html(&r).contains("Code incorrect"));
    let code = w.totp_code("ana@exemple.org", 0);
    let r = b.post("/compte/double-authentification?lang=fr", &[("code", &code)]);
    let page = html(&r).to_string();
    assert!(page.contains("Codes de secours"));
    let codes: Vec<String> = page.split("<li>").skip(1).map(|li| li.split('<').next().unwrap().to_string()).collect();
    assert_eq!(codes.len(), RECOVERY_CODES);
    assert!(codes.iter().all(|c| c.len() == 9 && c.as_bytes()[4] == b'-'));
    assert_eq!(w.mail("ana@exemple.org").subject, "Double authentification activée");
    assert!(html(&b.get("/compte?lang=fr")).contains("10 codes de secours restants"));

    // Se déconnecter, se reconnecter : le code est demandé.
    let r = b.post("/compte/deconnexion?lang=fr", &[]);
    assert_eq!(location(&r), "/compte/connexion?lang=fr");
    assert!(!b.jar.contains_key(SESSION_COOKIE));
    assert_eq!(location(&b.get("/compte")), "/compte/connexion?lang=fr");
    assert_eq!(location(&b.sign_in("ana@exemple.org", PASSWORD)), "/compte/code?lang=fr");
    assert_eq!(location(&b.get("/compte?lang=fr")), "/compte/code?lang=fr", "demi-session : rien d'autre que le code");
    let half = b.jar[SESSION_COOKIE].clone();
    // Le code de l'activation ne repasse pas.
    let r = b.post("/compte/code?lang=fr", &[("code", &code)]);
    assert!(html(&r).contains("Code incorrect"));
    w.advance(30);
    let r = b.post("/compte/code?lang=fr", &[("code", &w.totp_code("ana@exemple.org", 0))]);
    assert_eq!(location(&r), "/compte?lang=fr");
    assert_ne!(b.jar[SESSION_COOKIE], half, "nouveau jeton après la double authentification");
    assert_eq!(w.acc.db().session(&token_hash(&half), w.now()).unwrap(), None, "la demi-session est fermée");
    assert!(!b.jar.contains_key(TRUST_COOKIE));

    // Un code de secours, une seule fois.
    b.post("/compte/deconnexion?lang=fr", &[]);
    b.sign_in("ana@exemple.org", PASSWORD);
    assert!(html(&b.get("/compte/code?lang=fr&secours=1")).contains("name=\"secours\""));
    let r = b.post("/compte/code?lang=fr", &[("secours", &codes[0].to_uppercase())]);
    assert_eq!(location(&r), "/compte?lang=fr");
    b.post("/compte/deconnexion?lang=fr", &[]);
    b.sign_in("ana@exemple.org", PASSWORD);
    let r = b.post("/compte/code?lang=fr", &[("secours", &codes[0])]);
    assert!(html(&r).contains("Code incorrect"), "déjà servi");
    w.advance(30);
    // « Ne plus demander sur cet appareil pendant 30 jours ».
    let r = b.post("/compte/code?lang=fr", &[("code", &w.totp_code("ana@exemple.org", 0)), ("confiance", "1")]);
    assert_eq!(location(&r), "/compte?lang=fr");
    assert!(b.jar.contains_key(TRUST_COOKIE));
    b.post("/compte/deconnexion?lang=fr", &[]);
    assert_eq!(location(&b.sign_in("ana@exemple.org", PASSWORD)), "/compte?lang=fr", "navigateur de confiance");
    // Un autre navigateur, lui, donne le code.
    let mut other = w.browser_at("198.51.100.7");
    assert_eq!(location(&other.sign_in("ana@exemple.org", PASSWORD)), "/compte/code?lang=fr");
    // Trente jours plus tard, le code revient.
    w.advance(TRUST_SECS + 1);
    b.post("/compte/deconnexion?lang=fr", &[]);
    assert_eq!(location(&b.sign_in("ana@exemple.org", PASSWORD)), "/compte/code?lang=fr");
    w.advance(30);
    b.post("/compte/code?lang=fr", &[("code", &w.totp_code("ana@exemple.org", 0))]);

    // Nouveaux codes de secours, puis couper : avec le mot de passe.
    let r = b.post("/compte/codes-de-secours?lang=fr", &[("password", "pas le bon")]);
    assert!(html(&r).contains("Mot de passe incorrect"));
    let r = b.post("/compte/codes-de-secours?lang=fr", &[("password", PASSWORD)]);
    let fresh: Vec<&str> = html(&r).split("<li>").skip(1).map(|li| li.split('<').next().unwrap()).collect();
    assert_eq!(fresh.len(), RECOVERY_CODES);
    assert!(!fresh.contains(&codes[1].as_str()), "les anciens codes ne valent plus");
    let r = b.post("/compte/double-authentification/couper?lang=fr", &[("password", PASSWORD)]);
    assert!(html(&r).contains("Double authentification coupée"));
    assert_eq!(w.mail("ana@exemple.org").subject, "Double authentification coupée");
    b.post("/compte/deconnexion?lang=fr", &[]);
    assert_eq!(location(&b.sign_in("ana@exemple.org", PASSWORD)), "/compte?lang=fr");
    // Le journal du compte garde les étapes, sans adresse e-mail ni jeton.
    let kinds: Vec<String> = w.acc.db().audit_of(1).unwrap().into_iter().map(|(k, _)| k).collect();
    for k in ["inscription", "adresse_verifiee", "double_authentification_activee", "connexion_code_de_secours", "double_authentification_coupee", "deconnexion"] {
        assert!(kinds.iter().any(|x| x == k), "{k} dans le journal");
    }
}

#[test]
fn cookies_de_session() {
    let w = world(true);
    let mut b = w.browser();
    let r = b.get("/compte/connexion");
    let csrf = r.cookies.iter().find(|c| c.starts_with(CSRF_COOKIE)).unwrap();
    assert!(csrf.ends_with("; Path=/; Secure; HttpOnly; SameSite=Lax"));
    w.acc.add_verified("bob@exemple.org", "Bob", PASSWORD).unwrap();
    let r = b.sign_in("bob@exemple.org", PASSWORD);
    let session = r.cookies.iter().find(|c| c.starts_with(SESSION_COOKIE)).unwrap();
    assert!(session.contains("; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=2592000"), "{session}");
    let token = &b.jar[SESSION_COOKIE];
    assert_eq!(token.len(), 43, "256 bits");
    // La base garde l'empreinte, pas le jeton.
    let db = w.acc.db();
    assert!(db.session(token.as_bytes(), w.now()).unwrap().is_none());
    assert!(db.session(&token_hash(token), w.now()).unwrap().is_some());
}

#[test]
fn verification_par_le_lien() {
    let w = world(true);
    let mut a = w.browser();
    a.post("/compte/inscription?lang=en", &[("name", "Ana"), ("email", "ana@exemple.org"), ("password", PASSWORD)]);
    let mail = w.mail("ana@exemple.org");
    assert!(mail.subject.starts_with("Your KyberNow code"), "e-mail dans la langue de la page");
    let link = between(&mail.text, "https://share.test", "\n").to_string();
    assert!(link.starts_with("/compte/verifier?jeton=") && link.ends_with("&lang=en"));
    // Ouvert ailleurs (le téléphone) : l'adresse est vérifiée, pas de session.
    let mut phone = w.browser();
    let r = phone.get(&link);
    assert!(html(&r).contains("Address verified"));
    assert!(!phone.jar.contains_key(SESSION_COOKIE));
    assert!(phone.get(&link).status == 400, "le lien ne sert qu'une fois");
    // Ouvert dans le navigateur de l'inscription : la session s'ouvre.
    let mut b = w.browser();
    b.post("/compte/inscription?lang=fr", &[("name", "Bob"), ("email", "bob@exemple.org"), ("password", PASSWORD)]);
    let link = between(&w.mail("bob@exemple.org").text, "https://share.test", "\n").to_string();
    assert_eq!(location(&b.get(&link)), "/compte/proteger?lang=fr");
    assert!(b.jar.contains_key(SESSION_COOKIE));
    // Un lien expiré.
    let mut c = w.browser();
    c.post("/compte/inscription?lang=fr", &[("name", "Cléo"), ("email", "cleo@exemple.org"), ("password", PASSWORD)]);
    let link = between(&w.mail("cleo@exemple.org").text, "https://share.test", "\n").to_string();
    w.advance(VERIFY_SECS + 1);
    assert_eq!(c.get(&link).status, 400);
}

#[test]
fn renvoyer_et_changer_d_adresse() {
    let w = world(true);
    let mut b = w.browser();
    b.post("/compte/inscription?lang=fr", &[("name", "Ana"), ("email", "ana@exemple.org"), ("password", PASSWORD)]);
    let r = b.post("/compte/verifier?lang=fr", &[("action", "renvoyer")]);
    assert!(html(&r).contains("Un nouvel e-mail vient de partir"));
    assert_eq!(w.outbox.count_to("ana@exemple.org"), 2);
    // Trois e-mails par quart d'heure vers une même adresse, au plus ; la page reste la même.
    b.post("/compte/verifier?lang=fr", &[("action", "renvoyer")]);
    b.post("/compte/verifier?lang=fr", &[("action", "renvoyer")]);
    assert_eq!(w.outbox.count_to("ana@exemple.org"), 3);
    // Changer d'adresse : le code part à la nouvelle.
    assert!(html(&b.get("/compte/verifier?lang=fr&changer=1")).contains("Nouvelle adresse e-mail"));
    let r = b.post("/compte/verifier?lang=fr", &[("action", "changer"), ("email", "ana.m@exemple.org")]);
    assert!(html(&r).contains("<b>ana.m@exemple.org</b>"));
    let code: String = between(&w.mail("ana.m@exemple.org").text, "    ", "\n").chars().filter(char::is_ascii_digit).collect();
    assert_eq!(location(&b.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", &code)])), "/compte/proteger?lang=fr");
    assert!(w.acc.db().account_by_email("ana@exemple.org").unwrap().is_none());
}

#[test]
fn reponses_identiques_qu_un_compte_existe_ou_non() {
    let w = world(true);
    w.acc.add_verified("ana@exemple.org", "Ana", PASSWORD).unwrap();
    let mut b = w.browser();
    // Connexion : même page pour un mauvais mot de passe et une adresse inconnue.
    let wrong = b.sign_in("ana@exemple.org", "pas le bon mot de passe");
    let unknown = b.sign_in("ana@exemple.org".replace("ana", "zoe").as_str(), "pas le bon mot de passe");
    assert_eq!(wrong.status, unknown.status);
    assert_eq!(html(&wrong).replace("ana@", "X@"), html(&unknown).replace("zoe@", "X@"));
    assert!(html(&wrong).contains("Adresse e-mail ou mot de passe incorrect"));
    // Oubli : toujours la même réponse.
    let known = b.post("/compte/oubli?lang=fr", &[("email", "ana@exemple.org")]);
    let unknown = b.post("/compte/oubli?lang=fr", &[("email", "zoe@exemple.org")]);
    assert_eq!(html(&known), html(&unknown));
    assert!(html(&known).contains("Si un compte existe à cette adresse, un lien vient d'y partir."));
    assert_eq!(w.outbox.count_to("ana@exemple.org"), 1);
    assert_eq!(w.outbox.count_to("zoe@exemple.org"), 0);
    // Inscription d'une adresse prise : même suite, et un e-mail au titulaire.
    let mut c = w.browser_at("198.51.100.9");
    let taken = c.post("/compte/inscription?lang=fr", &[("name", "Intrus"), ("email", "ANA@exemple.org"), ("password", "un autre mot de passe")]);
    let mut d = w.browser_at("198.51.100.10");
    let fresh = d.post("/compte/inscription?lang=fr", &[("name", "Zoé"), ("email", "zoe@exemple.org"), ("password", "un autre mot de passe")]);
    assert_eq!(location(&taken), location(&fresh));
    assert_eq!(w.mail("ana@exemple.org").subject, "Votre compte KyberNow existe déjà");
    assert!(html(&c.get("/compte/verifier?lang=fr")).contains("<b>ana@exemple.org</b>"));
    let r = c.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", "123456")]);
    assert!(html(&r).contains("Code incorrect"));
    // Le compte d'Ana n'a pas bougé.
    assert_eq!(location(&b.sign_in("ana@exemple.org", PASSWORD)), "/compte?lang=fr");
}

#[test]
fn mot_de_passe_oublie_ferme_les_autres_sessions() {
    let w = world(true);
    w.acc.add_verified("ana@exemple.org", "Ana", PASSWORD).unwrap();
    let mut laptop = w.browser();
    assert_eq!(location(&laptop.sign_in("ana@exemple.org", PASSWORD)), "/compte?lang=fr");
    let mut phone = w.browser_at("198.51.100.7");
    phone.post("/compte/oubli?lang=fr", &[("email", " Ana@Exemple.org ")]);
    let mail = w.mail("ana@exemple.org");
    assert_eq!(mail.subject, "Nouveau mot de passe KyberNow");
    let link = between(&mail.text, "https://share.test", "\n").to_string();
    assert!(link.starts_with("/compte/nouveau-mot-de-passe?jeton="));
    let token = between(&link, "jeton=", "&").to_string();
    let page = phone.get(&link);
    assert!(html(&page).contains(&format!(r#"name="jeton" value="{token}""#)));
    let r = phone.post("/compte/nouveau-mot-de-passe?lang=fr", &[("jeton", &token), ("password", "court")]);
    assert!(html(&r).contains("10 caractères au moins"));
    let r = phone.post("/compte/nouveau-mot-de-passe?lang=fr", &[("jeton", &token), ("password", "lune verte et sel")]);
    assert!(html(&r).contains("Mot de passe changé"));
    assert_eq!(w.mail("ana@exemple.org").subject, "Mot de passe KyberNow changé");
    // Les autres sessions sont fermées ; l'ancien mot de passe ne vaut plus ; le lien non plus.
    assert_eq!(location(&laptop.get("/compte?lang=fr")), "/compte/connexion?lang=fr");
    assert!(html(&laptop.sign_in("ana@exemple.org", PASSWORD)).contains("incorrect"));
    assert_eq!(location(&laptop.sign_in("ana@exemple.org", "lune verte et sel")), "/compte?lang=fr");
    assert_eq!(phone.get(&link).status, 400);
    // Un lien vaut une heure.
    phone.post("/compte/oubli?lang=fr", &[("email", "ana@exemple.org")]);
    let link = between(&w.mail("ana@exemple.org").text, "https://share.test", "\n").to_string();
    w.advance(RESET_SECS + 1);
    assert_eq!(phone.get(&link).status, 400);
}

#[test]
fn limites_d_essais() {
    let w = world(true);
    w.acc.add_verified("ana@exemple.org", "Ana", PASSWORD).unwrap();
    // 5 essais par compte en 15 minutes : le sixième est refusé, même avec le bon mot de passe.
    let mut b = w.browser();
    for _ in 0..5 {
        assert_eq!(b.sign_in("ana@exemple.org", "mauvais mot de passe").status, 200);
    }
    let r = b.sign_in("ana@exemple.org", PASSWORD);
    assert_eq!(r.status, 429);
    assert!(r.retry_after.is_some() && html(&r).contains("Trop d'essais"));
    // D'une autre adresse IP aussi : la limite est celle du compte.
    assert_eq!(w.browser_at("198.51.100.1").sign_in("ana@exemple.org", PASSWORD).status, 429);
    // Une adresse e-mail sans compte se comporte de même.
    let mut c = w.browser_at("198.51.100.2");
    for _ in 0..5 {
        assert_eq!(c.sign_in("zoe@exemple.org", "mauvais mot de passe").status, 200);
    }
    assert_eq!(c.sign_in("zoe@exemple.org", "mauvais mot de passe").status, 429);
    // 10 essais par adresse IP, toutes adresses e-mail confondues.
    let mut d = w.browser_at("203.0.113.5");
    for i in 0..10 {
        assert_eq!(d.sign_in(&format!("x{i}@exemple.org"), "mauvais mot de passe").status, 200);
    }
    assert_eq!(d.sign_in("y@exemple.org", "mauvais mot de passe").status, 429);
    // Les codes de vérification : 5 erreurs, puis le code ne vaut plus.
    let mut e = w.browser_at("203.0.113.6");
    e.post("/compte/inscription?lang=fr", &[("name", "Léa"), ("email", "lea@exemple.org"), ("password", PASSWORD)]);
    let code: String = between(&w.mail("lea@exemple.org").text, "    ", "\n").chars().filter(char::is_ascii_digit).collect();
    let wrong = if code == "000000" { "111111" } else { "000000" };
    for _ in 0..5 {
        assert!(html(&e.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", wrong)])).contains("Code incorrect"));
    }
    let r = e.post("/compte/verifier?lang=fr", &[("action", "code"), ("code", &code)]);
    assert_eq!(r.status, 429, "cinq essais de code par compte en 15 minutes");
    // Le mot de passe oublié : 10 demandes par adresse IP.
    let mut f = w.browser_at("203.0.113.7");
    for _ in 0..10 {
        assert_eq!(f.post("/compte/oubli?lang=fr", &[("email", "zoe@exemple.org")]).status, 200);
    }
    assert_eq!(f.post("/compte/oubli?lang=fr", &[("email", "zoe@exemple.org")]).status, 429);
}

#[test]
fn csrf_et_formulaires_d_un_autre_site() {
    let w = world(true);
    w.acc.add_verified("ana@exemple.org", "Ana", PASSWORD).unwrap();
    let mut b = w.browser();
    b.get("/compte/connexion");
    let r = b.post("/compte/connexion?lang=fr", &[("email", "ana@exemple.org"), ("password", PASSWORD), ("csrf", "faux")]);
    assert_eq!(r.status, 403);
    assert!(html(&r).contains("Le formulaire a expiré"));
    // Sans le cookie : refusé.
    let csrf = w.acc.keys.sign(format!("csrf:{}", b.jar[CSRF_COOKIE]).as_bytes());
    b.jar.remove(CSRF_COOKIE);
    assert_eq!(b.post("/compte/connexion?lang=fr", &[("email", "ana@exemple.org"), ("password", PASSWORD), ("csrf", &csrf)]).status, 403);
    // D'un autre site : refusé, même avec le bon jeton.
    let mut c = w.browser();
    c.cross_site = true;
    assert_eq!(c.sign_in("ana@exemple.org", PASSWORD).status, 403);
    c.cross_site = false;
    assert_eq!(location(&c.sign_in("ana@exemple.org", PASSWORD)), "/compte?lang=fr");
    // Fermer une autre session.
    let mut other = w.browser_at("198.51.100.3");
    other.sign_in("ana@exemple.org", PASSWORD);
    let page = c.get("/compte?lang=fr");
    let id = between(html(&page), r#"name="id" value=""#, "\"").to_string();
    let r = c.post("/compte/sessions/fermer?lang=fr", &[("id", &id)]);
    assert!(html(&r).contains("Session fermée"));
    assert_eq!(location(&other.get("/compte?lang=fr")), "/compte/connexion?lang=fr");
    // Les sessions d'un autre compte ne se ferment pas d'ici.
    w.acc.add_verified("bob@exemple.org", "Bob", PASSWORD).unwrap();
    let mut bob = w.browser_at("198.51.100.4");
    bob.sign_in("bob@exemple.org", PASSWORD);
    let bob_id = w.acc.db().sessions(2, w.now()).unwrap()[0].id.to_string();
    c.post("/compte/sessions/fermer?lang=fr", &[("id", &bob_id)]);
    assert_eq!(bob.get("/compte?lang=fr").status, 200);
}

#[test]
fn sans_smtp_l_administrateur_cree_les_comptes() {
    let w = world(false);
    let mut b = w.browser();
    let sign_in = b.get("/compte/connexion?lang=fr");
    assert!(!html(&sign_in).contains("Créer un compte") && !html(&sign_in).contains("Mot de passe oublié"));
    assert!(html(&b.get("/compte/inscription?lang=fr")).contains("son administrateur qui crée les comptes"));
    assert_eq!(b.post("/compte/inscription?lang=fr", &[("name", "A"), ("email", "a@exemple.org"), ("password", PASSWORD)]).status, 403);
    assert!(w.acc.db().account_by_email("a@exemple.org").unwrap().is_none());
    assert!(html(&b.get("/compte/oubli?lang=fr")).contains("demandez à son administrateur"));
    // `kybernow-relay accounts add` : un compte vérifié.
    assert!(w.acc.add_verified("a@exemple.org", "A", "court").is_err());
    assert!(w.acc.add_verified("pas une adresse", "A", PASSWORD).is_err());
    w.acc.add_verified("A@Exemple.org", "A", PASSWORD).unwrap();
    assert!(w.acc.add_verified("a@exemple.org", "A", PASSWORD).is_err(), "adresse déjà prise");
    assert_eq!(location(&b.sign_in("a@exemple.org", PASSWORD)), "/compte?lang=fr");
}

#[test]
fn depuis_une_app() {
    let w = world(true);
    let mut b = w.browser();
    let page = b.get("/compte/connexion?lang=fr&appareil=salon-tv");
    assert!(!html(&page).contains("Créer un compte"));
    assert!(html(&page).contains("action=\"/compte/connexion?lang=fr&appareil=salon-tv\""));
    let refused = b.get("/compte/inscription?lang=fr&appareil=salon-tv");
    assert!(html(&refused).contains("dans un navigateur"));
    let r = b.post("/compte/inscription?lang=fr&appareil=salon-tv", &[("name", "A"), ("email", "a@exemple.org"), ("password", PASSWORD)]);
    assert_eq!(r.status, 403);
    assert!(w.acc.db().account_by_email("a@exemple.org").unwrap().is_none());
    // Le nom de l'appareil est encodé dans les liens (lot 3) ; un nom vide ou avec un caractère de contrôle est ignoré.
    let named = b.get("/compte/connexion?lang=fr&appareil=%3Cb%3E%20de%20L%C3%A9a");
    assert!(html(&named).contains("action=\"/compte/connexion?lang=fr&appareil=%3Cb%3E%20de%20L%C3%A9a\"") && !html(&named).contains("<b> de"));
    assert!(html(&b.get("/compte/connexion?lang=fr&appareil=a%0Ab")).contains("Créer un compte"));
    assert!(html(&b.get("/compte/connexion?lang=fr&appareil=%20")).contains("Créer un compte"));
}

#[test]
fn pages_inconnues_et_methodes() {
    let w = world(true);
    let mut b = w.browser();
    assert_eq!(b.get("/compte/rien").status, 404);
    assert_eq!(b.request(Method::Other, "/compte", &[]).status, 405);
    assert_eq!(location(&b.get("/compte/")), "/compte/connexion?lang=fr");
    assert_eq!(location(&b.get("/compte/verifier")), "/compte/connexion?lang=fr", "pas d'adresse en attente");
    assert_eq!(location(&b.get("/compte/double-authentification")), "/compte/connexion?lang=fr");
    assert_eq!(location(&b.get("/compte/code")), "/compte/connexion?lang=fr");
}

#[test]
fn formulaires_et_appareils() {
    assert_eq!(
        parse_pairs("email=ana%40exemple.org&password=a+b%2Bc&vide=&x"),
        vec![("email".into(), "ana@exemple.org".into()), ("password".into(), "a b+c".into()), ("vide".into(), "".into()), ("x".into(), "".into())]
    );
    assert_eq!(parse_pairs("a=%E2%80%AF%zz%4"), vec![("a".into(), "\u{202F}%zz%4".into())]);
    assert_eq!(parse_cookies("a=1; __Host-kn_session=abc ;b"), vec![("a".into(), "1".into()), ("__Host-kn_session".into(), "abc".into())]);
    assert_eq!(device_name("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1"), "Safari, iPhone");
    assert_eq!(device_name("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36 Edg/130.0"), "Edge, Mac");
    assert_eq!(device_name("Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Mobile Safari/537.36"), "Chrome, Android");
    assert_eq!(device_name("curl/8"), "");
}

#[test]
fn les_essais_reussis_ne_comptent_pas() {
    let w = world(true);
    w.acc.add_verified("ana@exemple.org", "Ana", PASSWORD).unwrap();
    let mut b = w.browser();
    for _ in 0..12 {
        assert_eq!(location(&b.sign_in("ana@exemple.org", PASSWORD)), "/compte?lang=fr");
    }
}

// Lot 2 (rattachement) et moitié relais du lot 3 (appareils) : à part, pour ne pas mêler les fusions.
mod lot2;
