//! Essais des comptes KyberNow dans le cœur : un faux relais qui répond
//! comme `server/relay/src/accounts/devices.rs`.

use super::*;

const STATION: &str = "abcdefghijklmnopqrstuvwxyz";

/// Un appel reçu par le faux relais.
#[derive(Debug, Clone)]
struct Seen {
    relay: String,
    method: String,
    path: String,
    bearer: Option<String>,
    body: Option<Value>,
}

type Answer = Box<dyn Fn(&Seen, usize) -> Result<Reply, String> + Send + Sync>;

/// Le faux relais : `answer(appel, rang de l'appel)`.
struct Fake {
    seen: Mutex<Vec<Seen>>,
    answer: Answer,
}

impl Fake {
    fn new(answer: impl Fn(&Seen, usize) -> Result<Reply, String> + Send + Sync + 'static) -> Arc<Fake> {
        Arc::new(Fake { seen: Mutex::new(Vec::new()), answer: Box::new(answer) })
    }

    fn calls(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

impl RelayHttp for Fake {
    fn call(&self, relay: &RelayAddr, method: &str, path: &str, bearer: Option<&str>, body: Option<&Value>) -> Result<Reply, String> {
        let s = Seen { relay: relay.to_string(), method: method.into(), path: path.into(), bearer: bearer.map(str::to_owned), body: body.cloned() };
        let n = {
            let mut seen = self.seen.lock().unwrap();
            seen.push(s.clone());
            seen.len() - 1
        };
        (self.answer)(&s, n)
    }
}

fn reply(status: u16, body: Value) -> Result<Reply, String> {
    Ok(Reply { status, body, retry_after: None })
}

fn opened_body() -> Value {
    json!({
        "token": "session-de-l-appareil",
        "device": { "id": "dev_ab", "name": "iPad de Léa" },
        "account": { "id": "acc_lea", "name": "Léa Martin", "email": "lea@exemple.fr" },
        "relay": "share.kybernow.app",
    })
}

fn account() -> Account {
    Account {
        relay: "share.kybernow.app".into(),
        token: "session-de-l-appareil".into(),
        account: AccountUser { id: "acc_lea".into(), name: "Léa Martin".into(), email: "lea@exemple.fr".into() },
        device: AccountDevice { id: "dev_ab".into(), name: "iPad de Léa".into() },
    }
}

/// Une Station : son identifiant, sa clé et sa signature de `admin_fp`.
fn station(admin_fp: &str) -> (String, String, String) {
    use base64::Engine;
    use ring::signature::KeyPair;
    let rng = ring::rand::SystemRandom::new();
    let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    let mut spki = vec![0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];
    spki.extend_from_slice(pair.public_key().as_ref());
    let sig = pair.sign(&kybernow_tunnel::lookup::sig_message(admin_fp));
    let b64 = base64::engine::general_purpose::STANDARD;
    (kybernow_tunnel::id::station_id(&spki), b64.encode(&spki), b64.encode(sig.as_ref()))
}

#[test]
fn le_jeton_ne_paraît_pas_dans_les_journaux() {
    let a = account();
    let shown = format!("{a:?}");
    assert!(!shown.contains("session-de-l-appareil") && shown.contains("acc_lea"));
    let s = a.summary();
    assert_eq!(s.key, "share.kybernow.app|acc_lea");
    assert_eq!((s.relay_label.as_str(), s.console.as_str()), ("share.kybernow.app", "https://share.kybernow.app/compte"));
    assert!(!serde_json::to_string(&s).unwrap().contains("session-de-l-appareil"), "le résumé n'a pas le jeton");
    // Le compte entier, lui, se garde avec son jeton (coffre du système).
    let back: Account = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
    assert_eq!(back, a);
}

#[test]
fn relais_sous_leur_forme_normale() {
    assert_eq!(normalize_relay("").unwrap(), "share.kybernow.app");
    assert_eq!(normalize_relay(" https://Share.Kybernow.app:443/ ").unwrap(), "share.kybernow.app");
    assert_eq!(normalize_relay("relais.exemple.org:9443").unwrap(), "relais.exemple.org:9443");
    assert_eq!(normalize_relay("a b"), Err(AccountError::BadRelay));
    let pinned = "relais.exemple.org:9443#sha256:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB";
    let n = normalize_relay(pinned).unwrap();
    assert!(n.contains("#sha256:ab"), "empreinte gardée : {n}");
    assert_eq!(relay_label(&n), "relais.exemple.org:9443", "montré sans l'empreinte");
    assert_eq!(console_url(&n), "https://relais.exemple.org:9443/compte");
}

#[test]
fn fenetre_du_systeme() {
    let s = sign_in("", "iPad de Léa").unwrap();
    assert_eq!((s.relay.as_str(), s.callback_scheme.as_str()), ("share.kybernow.app", "kybernow"));
    assert_eq!(s.state.len(), 32);
    assert!(s.state.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(
        s.url,
        format!(
            "https://share.kybernow.app/compte/connexion?appareil=iPad%20de%20L%C3%A9a&retour=kybernow%3A%2F%2Fcompte&etat={}&code_challenge={}&code_challenge_method=S256",
            s.state,
            pkce_challenge(&s.verifier)
        )
    );
    // PKCE (REL-09) : vérificateur de 43 signes base64url, défi S256.
    assert_eq!(s.verifier.len(), 43);
    assert!(s.verifier.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
    assert_eq!(pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", "vecteur de la RFC 7636");
    assert_ne!(sign_in("", "x").unwrap().state, s.state, "un état par demande");
    // Le retour : le code échangé contre la session d'appareil.
    let verifier = s.verifier.clone();
    let fake = Fake::new(move |c, _| {
        assert_eq!((c.method.as_str(), c.path.as_str(), c.bearer.as_deref()), ("POST", "/v1/devices/token", None));
        assert_eq!(c.body, Some(json!({ "code": "c0de/+x", "code_verifier": verifier })));
        reply(200, opened_body())
    });
    let back = format!("kybernow://compte?code=c0de%2F%2Bx&etat={}", s.state);
    assert!(is_callback(&back) && is_callback("KYBERNOW://compte/?code=1") && !is_callback("kybernow://j/123456"));
    let a = finish_sign_in_with(&*fake, &s, &back).unwrap();
    assert_eq!(a, account());
    assert_eq!(fake.calls()[0].relay, "share.kybernow.app");
    // Un autre état, « Annuler », pas de code : rien n'est demandé au relais.
    let other = format!("kybernow://compte?code=x&etat={}", "0".repeat(32));
    assert_eq!(finish_sign_in_with(&*fake, &s, &other), Err(AccountError::BadCode));
    assert_eq!(finish_sign_in_with(&*fake, &s, &format!("kybernow://compte?erreur=annule&etat={}", s.state)), Err(AccountError::Cancelled));
    assert_eq!(finish_sign_in_with(&*fake, &s, &format!("kybernow://compte/?etat={}", s.state)), Err(AccountError::BadCode));
    assert_eq!(finish_sign_in_with(&*fake, &s, "kybernow://ailleurs?code=x"), Err(AccountError::BadCode));
    assert_eq!(fake.calls().len(), 1);
    // Code échu ou déjà servi.
    let refused = Fake::new(|_, _| reply(400, json!({ "error": "bad_code" })));
    assert_eq!(finish_sign_in_with(&*refused, &s, &back), Err(AccountError::BadCode));
    // Un `SignIn` gardé par une app d'avant PKCE (JSON sans `verifier`) :
    // l'échange part sans vérificateur, comme avant.
    let old: SignIn = serde_json::from_value(json!({ "relay": s.relay, "url": s.url, "state": s.state, "callback_scheme": "kybernow" })).unwrap();
    assert!(old.verifier.is_empty());
    let plain = Fake::new(|c, _| {
        assert_eq!(c.body, Some(json!({ "code": "c0de/+x" })));
        reply(200, opened_body())
    });
    assert!(finish_sign_in_with(&*plain, &old, &back).is_ok());
}

#[test]
fn nom_de_l_appareil() {
    assert_eq!(device_name("  "), "KyberNow Link");
    assert_eq!(device_name("PC\n de Léa "), "PC de Léa");
    assert_eq!(device_name(&"x".repeat(100)).len(), 64);
}

#[test]
fn encodage_des_adresses() {
    assert_eq!(encode("a b/é"), "a%20b%2F%C3%A9");
    assert_eq!(crate::url::percent_decode("a%20b%2F%C3%A9+c"), "a b/é c");
    assert_eq!(crate::url::percent_decode("100%"), "100%", "pourcent seul gardé");
    assert_eq!(crate::url::percent_decode("%zz%4"), "%zz%4");
}

fn tv_body() -> Value {
    json!({ "code": "KQ7M-4XPA", "poll": "secret-de-demande", "expires": 600, "interval": 5,
            "url": "https://share.kybernow.app/tv", "url_complete": "https://share.kybernow.app/compte/appareil?code=KQ7M-4XPA" })
}

#[test]
fn code_de_televiseur() {
    let fake = Fake::new(|c, _| {
        assert_eq!((c.path.as_str(), c.body.as_ref().unwrap()["name"].as_str()), ("/v1/devices/code", Some("Apple TV du salon")));
        reply(200, tv_body())
    });
    let code = tv_code_with(&*fake, "share.kybernow.app", "Apple TV du salon").unwrap();
    assert_eq!((code.code.as_str(), code.url_label.as_str(), code.expires, code.interval), ("KQ7M-4XPA", "share.kybernow.app/tv", 600, 5));
    assert_eq!(code.url_complete, "https://share.kybernow.app/compte/appareil?code=KQ7M-4XPA");
    let t = code.issued_at;
    assert_eq!((code.minutes_left(t), code.minutes_left(t + 61), code.minutes_left(t + 599), code.minutes_left(t + 600)), (10, 9, 1, 0));
    assert!(code.expired(t + 600) && !code.expired(t + 599));
    // Les réponses d'une demande.
    let answers = [reply(202, json!({ "pending": true, "interval": 5 })), reply(200, opened_body()), reply(403, json!({ "error": "denied" })), reply(410, json!({ "error": "expired" })), reply(404, json!({ "error": "unknown_code" }))];
    let poll = Fake::new(move |c, n| {
        assert_eq!(c.body, Some(json!({ "poll": "secret-de-demande" })));
        answers[n].clone()
    });
    assert_eq!(tv_poll_with(&*poll, &code), Ok(TvPoll::Pending));
    assert_eq!(tv_poll_with(&*poll, &code), Ok(TvPoll::Approved(Account { device: AccountDevice { id: "dev_ab".into(), name: "iPad de Léa".into() }, ..account() })));
    assert_eq!(tv_poll_with(&*poll, &code), Err(AccountError::Denied));
    assert_eq!(tv_poll_with(&*poll, &code), Err(AccountError::Expired));
    assert_eq!(tv_poll_with(&*poll, &code), Err(AccountError::Expired));
    // Trop de codes demandés.
    let many = Fake::new(|_, _| Ok(Reply { status: 429, body: json!({ "error": "too_many" }), retry_after: Some(300) }));
    let e = tv_code_with(&*many, "", "TV").unwrap_err();
    assert_eq!(e, AccountError::TooMany { seconds: Some(300) });
    assert_eq!(e.text(Lang::Fr), "Trop d'essais : réessayez dans 5 min.");
}

#[test]
fn attente_du_code() {
    let mut code = tv_code_with(&*Fake::new(|_, _| reply(200, tv_body())), "", "TV").unwrap();
    code.interval = 0;
    // Pending, relais muet, puis approuvé.
    let fake = Fake::new(|_, n| match n {
        0 => reply(202, json!({ "pending": true })),
        1 => Err("connexion refusée".into()),
        _ => reply(200, opened_body()),
    });
    let cancel = AtomicBool::new(false);
    // Une demande par intervalle au moins (5 s pour de vrai, 200 ms ici).
    let min = Duration::from_millis(200);
    let started = Instant::now();
    let a = tv_wait_with(&*fake, &code, &cancel, Duration::from_millis(20), min).unwrap();
    assert_eq!(a.account.id, "acc_lea");
    assert_eq!(fake.calls().len(), 3);
    assert!(started.elapsed() >= min * 3, "pas plus d'une demande par intervalle");
    // « Annuler » : tout de suite, sans demande.
    let fake = Fake::new(|_, _| reply(202, json!({})));
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(tv_wait_with(&*fake, &code, &cancel, Duration::from_millis(10), min), Err(AccountError::Cancelled));
    assert!(fake.calls().is_empty());
    // Refusé sur le téléphone.
    cancel.store(false, Ordering::Relaxed);
    let fake = Fake::new(|_, _| reply(403, json!({ "error": "denied" })));
    assert_eq!(tv_wait_with(&*fake, &code, &cancel, Duration::from_millis(20), min), Err(AccountError::Denied));
}

#[test]
fn ordinateurs_du_compte() {
    let (id, key, sig) = station("sha256:aa:bb");
    let (other, _, _) = station("sha256:cc");
    let body = json!({ "computers": [
        { "station": id, "name": "PC du salon", "rights": ["session", "lend", "share", "admin"], "owner": { "kind": "person", "id": "acc_lea", "name": "Léa Martin" },
          "mine": true, "online": true, "attached_at": 1795000000, "station_key": key, "admin_fp": "sha256:aa:bb", "sig": sig },
        // Signature qui ne tient pas : l'empreinte est écartée, l'ordinateur reste.
        { "station": other, "name": "MacBook Pro", "rights": ["session"], "mine": true, "online": true, "station_key": key, "admin_fp": "sha256:faux", "sig": sig },
        { "station": STATION, "name": "Hors ligne", "rights": ["session"], "mine": true, "online": false },
        { "station": "pas-une-station", "name": "Illisible" },
    ]});
    let fake = Fake::new(move |c, _| {
        assert_eq!((c.method.as_str(), c.path.as_str(), c.bearer.as_deref()), ("GET", "/v1/computers", Some("session-de-l-appareil")));
        reply(200, body.clone())
    });
    let list = computers_with(&*fake, &account()).unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!((list[0].name.as_str(), list[0].admin_fp.as_deref(), list[0].online), ("PC du salon", Some("sha256:aa:bb"), true));
    assert!(list[0].can("admin") && list[0].mine && list[0].account == "share.kybernow.app|acc_lea");
    // « Regarder » : le droit `view` ; seul, il remplace « Se connecter ».
    assert!(!list[0].can_watch() && !list[0].watch_only());
    let viewer = AccountComputer { rights: vec!["view".into()], ..list[0].clone() };
    assert!(viewer.can_watch() && viewer.watch_only());
    let both = AccountComputer { rights: vec!["view".into(), "session".into()], ..list[0].clone() };
    assert!(both.can_watch() && !both.watch_only());
    assert_eq!(list[1].admin_fp, None);
    assert_eq!((list[2].station.as_str(), list[2].online), (STATION, false));
    // L'empreinte vue une fois est gardée quand la Station se déconnecte.
    let mut fresh = vec![AccountComputer { online: false, admin_fp: None, ..list[0].clone() }];
    remember_fingerprints(&mut fresh, &list);
    assert_eq!(fresh[0].admin_fp.as_deref(), Some("sha256:aa:bb"));
    // Session d'appareil fermée ; relais sans comptes.
    let closed = Fake::new(|_, _| reply(401, json!({ "error": "bad_session" })));
    assert_eq!(computers_with(&*closed, &account()), Err(AccountError::SignedOut));
    let old = Fake::new(|_, _| reply(404, Value::Null));
    assert_eq!(computers_with(&*old, &account()), Err(AccountError::NoAccounts));
    let down = Fake::new(|_, _| Err("délai dépassé".into()));
    assert!(matches!(computers_with(&*down, &account()), Err(AccountError::Unreachable(_))));
}

#[test]
fn pastilles_d_origine() {
    let c = AccountComputer { relay: "relais.exemple.org:9443".into(), mine: true, owner: Owner { kind: "person".into(), id: "acc_lea".into(), name: "Léa Martin".into() }, ..Default::default() };
    assert_eq!(c.origin(Lang::Fr, false), "Compte");
    assert_eq!(c.origin(Lang::Fr, true), "Compte · relais.exemple.org:9443");
    assert_eq!(c.origin(Lang::En, true), "Account · relais.exemple.org:9443");
    let lent = AccountComputer { mine: false, ..c.clone() };
    assert_eq!(lent.origin(Lang::Fr, false), "Ouvert par Léa");
    assert_eq!(lent.origin(Lang::En, false), "Opened by Léa");
    let org = AccountComputer { owner: Owner { kind: "organization".into(), id: "org_1".into(), name: "Les Lignes Claires".into() }, ..c };
    assert_eq!(org.origin(Lang::Fr, true), "Les Lignes Claires");
    assert_eq!((origin_paired(Lang::Fr), origin_paired(Lang::En)), ("Appairé".to_string(), "Paired".to_string()));
}

#[test]
fn un_ordinateur_une_seule_fois() {
    let c = |station: &str, fp: Option<&str>, account: &str| AccountComputer {
        account: account.into(),
        relay: "share.kybernow.app".into(),
        station: station.into(),
        name: station.into(),
        admin_fp: fp.map(str::to_owned),
        ..Default::default()
    };
    let paired = vec![
        Paired { id: "salon".into(), fingerprint: Some("SHA256:AA:BB ".into()), station: None },
        Paired { id: "bureau".into(), fingerprint: Some("sha256:dd".into()), station: Some("station-bureau".into()) },
        Paired { id: "vieux".into(), fingerprint: None, station: None },
    ];
    let list = vec![
        c("station-salon", Some("sha256:aa:bb"), "a"),
        // Même Station que l'appairé « bureau », empreinte pas encore vue.
        c("station-bureau", None, "a"),
        c("station-atelier", Some("sha256:ee"), "a"),
        // Le même ordinateur par un autre compte : une fois.
        c("station-atelier", Some("sha256:ee"), "b"),
        c("station-cave", None, "b"),
    ];
    let m = merge(&paired, &list);
    assert_eq!(m.linked.iter().map(|(id, c)| (id.as_str(), c.station.as_str())).collect::<Vec<_>>(), vec![("salon", "station-salon"), ("bureau", "station-bureau")]);
    assert_eq!(m.added.iter().map(|c| (c.station.as_str(), c.account.as_str())).collect::<Vec<_>>(), vec![("station-atelier", "a"), ("station-cave", "b")]);
    // Sans appairé, rien n'est relié : quatre ordinateurs, l'atelier une fois.
    assert_eq!(merge(&[], &list).added.len(), 4);
    // MAC-20 : la même règle que les apps, une empreinte écrite sans préfixe
    // ni deux-points (invitation, ancien appairage) est la même.
    let paired = vec![Paired { id: "salon".into(), fingerprint: Some("aabb".into()), station: None }];
    assert_eq!(merge(&paired, &list).linked.first().map(|(id, c)| (id.as_str(), c.station.as_str())), Some(("salon", "station-salon")));
    let paired = vec![Paired { id: "vide".into(), fingerprint: Some(" ".into()), station: None }];
    assert!(merge(&paired, &[c("x", Some(" "), "a")]).linked.is_empty(), "vide : jamais la même");
}

fn access(renew: Duration, fake: &Arc<Fake>) -> Arc<StationAccess> {
    StationAccess::with_http(AccessParams { account: account(), station: STATION.into() }, fake.clone(), renew)
}

fn access_reply(n: usize, expires: u64) -> Result<Reply, String> {
    reply(200, json!({ "token": format!("jeton-{n}"), "station": STATION, "rights": ["session"], "expires": expires, "lifetime": 300 }))
}

#[test]
fn jeton_d_acces_garde_quatre_minutes() {
    let fake = Fake::new(|c, n| {
        assert_eq!((c.method.as_str(), c.path.as_str(), c.bearer.as_deref()), ("POST", "/v1/access", Some("session-de-l-appareil")));
        assert_eq!(c.body, Some(json!({ "station": STATION })));
        access_reply(n, now_secs() + 300)
    });
    let a = access(RENEW_AFTER, &fake);
    assert_eq!(a.token().unwrap(), "jeton-0");
    assert_eq!(a.token().unwrap(), "jeton-0", "le même pendant 4 minutes");
    assert_eq!(fake.calls().len(), 1);
    // Redémarrer : un jeton à part, pas gardé.
    assert_eq!(a.single_use().unwrap(), "jeton-1");
    assert_eq!(a.token().unwrap(), "jeton-0");
    // Refusé par Admin (`token_expired`) : le suivant est neuf.
    a.invalidate();
    assert_eq!(a.token().unwrap(), "jeton-2");
    // Passé le délai de renouvellement : le même sert encore, un autre est
    // demandé à côté, sans faire attendre l'appel (COEUR-34).
    let b = access(Duration::ZERO, &fake);
    assert_eq!(b.token().unwrap(), "jeton-3");
    assert_eq!(b.token().unwrap(), "jeton-3", "servi pendant le renouvellement");
    let started = std::time::Instant::now();
    while b.cache.lock().unwrap().as_ref().map(|c| c.token.clone()).as_deref() == Some("jeton-3") {
        assert!(started.elapsed() < Duration::from_secs(5), "pas de renouvellement");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(b.cache.lock().unwrap().as_ref().map(|c| c.token.clone()).as_deref(), Some("jeton-4"));
}

#[test]
fn jeton_d_acces_echu_selon_sa_duree() {
    // Un jeton de 20 secondes : déjà à moins de 30 secondes de son échéance
    // (horloge murale de l'appareil, qui compte aussi son sommeil).
    let fake = Fake::new(|_, n| reply(200, json!({ "token": format!("jeton-{n}"), "station": STATION, "rights": ["session"], "expires": now_secs() + 20, "lifetime": 20 })));
    let a = access(RENEW_AFTER, &fake);
    assert_eq!(a.token().unwrap(), "jeton-0");
    assert_eq!(a.token().unwrap(), "jeton-1", "à moins de 30 secondes de l'échéance");
}

#[test]
fn jeton_d_acces_horloge_de_l_appareil_en_avance() {
    // L'horloge de l'appareil a 10 minutes d'avance sur le relais : l'échéance
    // donnée par le relais paraît passée, mais le jeton vaut ses 5 minutes.
    let fake = Fake::new(|_, n| access_reply(n, now_secs().saturating_sub(600) + 300));
    let a = access(RENEW_AFTER, &fake);
    assert_eq!(a.token().unwrap(), "jeton-0");
    assert_eq!(a.token().unwrap(), "jeton-0", "gardé : sa durée de vie compte, pas l'heure du relais");
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn jeton_d_acces_de_deux_minutes() {
    // Le relais donne des jetons de 2 minutes (docs/comptes.md, « Sessions ») :
    // gardé 90 secondes, encore valable une bonne minute de plus pour la Station.
    let fake = Fake::new(|_, n| {
        reply(200, json!({ "token": format!("jeton-{n}"), "station": STATION, "rights": ["session"], "expires": now_secs() + 120, "lifetime": 120 }))
    });
    let a = access(RENEW_AFTER, &fake);
    assert_eq!(a.token().unwrap(), "jeton-0");
    assert_eq!(a.token().unwrap(), "jeton-0", "gardé tant qu'il vaut plus de 30 secondes");
    let renew = a.cache.lock().unwrap().as_ref().map(|c| c.renew);
    assert_eq!(renew, Some(Duration::from_secs(90)));
    // Un relais plus ancien (5 minutes) : 4 minutes, comme avant.
    let old = Fake::new(|_, n| access_reply(n, now_secs() + 300));
    let b = access(RENEW_AFTER, &old);
    b.token().unwrap();
    assert_eq!(b.cache.lock().unwrap().as_ref().map(|c| c.renew), Some(RENEW_AFTER));
}

#[test]
fn jeton_d_acces_refuse() {
    let answers = [reply(404, json!({ "error": "unknown_station" })), reply(401, json!({ "error": "bad_session" })), reply(200, json!({ "station": STATION }))];
    let fake = Fake::new(move |_, n| answers[n].clone());
    let a = access(RENEW_AFTER, &fake);
    assert_eq!(a.token(), Err(AccountError::UnknownComputer));
    assert_eq!(a.token(), Err(AccountError::SignedOut));
    assert!(matches!(a.token(), Err(AccountError::Other(_))), "sans jeton");
}

#[test]
fn se_deconnecter() {
    let fake = Fake::new(|c, n| {
        assert_eq!((c.path.as_str(), c.bearer.as_deref()), ("/v1/devices/revoke", Some("session-de-l-appareil")));
        if n == 0 { reply(200, json!({ "ok": true })) } else { reply(401, json!({ "error": "bad_session" })) }
    });
    assert_eq!(sign_out_with(&*fake, &account()), Ok(()));
    assert_eq!(sign_out_with(&*fake, &account()), Ok(()), "déjà fermée");
}

#[test]
fn textes_et_codes() {
    let all = [
        AccountError::SignedOut,
        AccountError::UnknownComputer,
        AccountError::Denied,
        AccountError::Expired,
        AccountError::BadCode,
        AccountError::Cancelled,
        AccountError::TooMany { seconds: None },
        AccountError::NoAccounts,
        AccountError::BadRelay,
        AccountError::Unreachable("x".into()),
        AccountError::Other("x".into()),
    ];
    let codes: Vec<i32> = all.iter().map(AccountError::code).collect();
    assert_eq!(codes, (70..=80).collect::<Vec<_>>());
    for e in &all {
        for lang in [Lang::Fr, Lang::En] {
            let t = e.text(lang);
            assert!(!t.is_empty() && !t.starts_with("account."), "{} : {t}", e.kind());
        }
    }
    assert_eq!(AccountError::SignedOut.text(Lang::Fr), "Ce compte a été déconnecté de cet appareil : reconnectez-vous.");
    assert_eq!(AccountError::Denied.text(Lang::En), "Sign-in declined on the phone.");
    assert_eq!(AccountError::TooMany { seconds: None }.text(Lang::Fr), "Trop d'essais : réessayez dans 15 min.");
}

// MARK: - Réglages synchronisés

fn caps_all() -> crate::profile::Capabilities {
    crate::profile::Capabilities { hevc: true, hevc444: true, av1: true, hdr: true, pyrowave: true }
}

#[test]
fn reglages_synchronises_lus() {
    let body = json!({ "settings": [
        { "station": STATION, "updated_at": 1795000000, "values": {
            "profile": "bureau", "codec": "hevc", "yuv444": true, "mbps": 90, "hdr": false, "immersive": "always", "futur": { "x": 1 } } },
        { "station": "zyxwvutsrqponmlkjihgfedcba", "updated_at": 1795000100, "values": {
            "profile": null, "codec": "vp9", "mbps": 80.4, "immersive": "plus-tard" } },
        { "station": "pas-une-station", "values": {} },
        { "pas": "lisible" },
    ]});
    let fake = Fake::new(move |c, _| {
        assert_eq!((c.method.as_str(), c.path.as_str(), c.bearer.as_deref()), ("GET", "/v1/settings", Some("session-de-l-appareil")));
        reply(200, body.clone())
    });
    let list = settings::fetch_settings_with(&*fake, &account()).unwrap();
    assert_eq!(list.len(), 2);
    let a = settings::find(&list, STATION).unwrap();
    assert_eq!(a.updated_at, 1795000000);
    let k = a.known();
    assert_eq!(k.profile, Some(Some("bureau".to_string())));
    assert_eq!(k.profile(), Some(Some(crate::profile::Profile::Bureau)));
    assert_eq!((k.codec.as_deref(), k.yuv444, k.mbps, k.hdr, k.immersive.as_deref()), (Some("hevc"), Some(true), Some(90), Some(false), Some("always")));
    // Personnalisé ; format et mode immersif inconnus : absents, gardés au relais.
    let b = settings::find(&list, "zyxwvutsrqponmlkjihgfedcba").unwrap().known();
    assert_eq!((b.profile.clone(), b.codec.clone(), b.mbps, b.immersive.clone()), (Some(None), None, Some(80), None));
    // En JSON pour les ponts : « profile » nul dit personnalisé, les absentes sont omises.
    assert_eq!(serde_json::to_value(&b).unwrap(), json!({ "profile": null, "mbps": 80 }));
    let back: SyncedValues = serde_json::from_value(json!({ "profile": null, "mbps": 80 })).unwrap();
    assert_eq!(back, b);
    let none: SyncedValues = serde_json::from_value(json!({ "mbps": 80 })).unwrap();
    assert_eq!(none.profile, None);
    // Session fermée ; relais d'avant les réglages synchronisés.
    let closed = Fake::new(|_, _| reply(401, json!({ "error": "bad_session" })));
    assert_eq!(settings::fetch_settings_with(&*closed, &account()), Err(AccountError::SignedOut));
    let old = Fake::new(|_, _| reply(404, Value::Null));
    assert_eq!(settings::fetch_settings_with(&*old, &account()), Err(AccountError::NoAccounts));
}

#[test]
fn reglages_synchronises_envoyes() {
    let fake = Fake::new(|c, n| match n {
        0 => {
            assert_eq!((c.method.as_str(), c.path.as_str(), c.bearer.as_deref()), ("PUT", format!("/v1/settings/{STATION}").as_str(), Some("session-de-l-appareil")));
            // Les clés inconnues de l'app restent ; les siennes changent.
            assert_eq!(
                c.body,
                Some(json!({ "values": { "profile": null, "codec": "av1", "yuv444": false, "mbps": 80, "hdr": true, "immersive": "never", "futur": 3 } }))
            );
            reply(200, json!({ "updated_at": 1795000500 }))
        }
        1 => reply(404, json!({ "error": "unknown_station" })),
        2 => reply(413, json!({ "error": "too_large" })),
        _ => reply(400, json!({ "error": "bad_values" })),
    });
    let last = SyncedSettings {
        station: STATION.into(),
        updated_at: 1,
        values: json!({ "profile": "jeu", "codec": "hevc", "immersive": "always", "futur": 3 }).as_object().unwrap().clone(),
    };
    let custom = crate::profile::StreamSettings { profile: None, codec: crate::profile::Codec::Av1, yuv444: false, mbps: 80, reliable: false, hdr: true, wired: false, adaptive: true, encoder_max: None };
    let values = SyncedValues::from_stream(&custom).with_immersive(Some("never"));
    let new = settings::put_settings_with(&*fake, &account(), STATION, Some(&last), &values).unwrap();
    assert_eq!(new.updated_at, 1795000500);
    assert_eq!(new.values.get("futur"), Some(&json!(3)));
    assert_eq!(new.known().profile, Some(None));
    assert_eq!(settings::put_settings_with(&*fake, &account(), STATION, Some(&new), &values), Err(AccountError::UnknownComputer));
    assert!(matches!(settings::put_settings_with(&*fake, &account(), STATION, None, &values), Err(AccountError::Other(e)) if e.contains("too_large")));
    assert!(matches!(settings::put_settings_with(&*fake, &account(), STATION, None, &values), Err(AccountError::Other(e)) if e.contains("bad_values")));
    // Un identifiant illisible ne part pas.
    assert_eq!(settings::put_settings_with(&*fake, &account(), "pc", None, &values), Err(AccountError::UnknownComputer));
    // Trop grand pour le relais : refusé sans appel (COEUR-43, `MAX_BYTES`).
    let calls = fake.calls().len();
    let huge = SyncedSettings { station: STATION.into(), updated_at: 1, values: json!({ "futur": "x".repeat(settings::MAX_BYTES) }).as_object().unwrap().clone() };
    assert!(matches!(settings::put_settings_with(&*fake, &account(), STATION, Some(&huge), &values), Err(AccountError::Other(e)) if e.contains("too_large")));
    assert_eq!(fake.calls().len(), calls, "rien d'envoyé");
    assert_eq!(fake.calls().len(), 4);
    // Les réglages d'un autre ordinateur ne servent pas de base.
    let mut other = last.clone();
    other.station = "zyxwvutsrqponmlkjihgfedcba".into();
    let only = Fake::new(|c, _| {
        assert_eq!(c.body.as_ref().and_then(|b| b["values"].get("futur")), None);
        reply(200, json!({ "updated_at": 2 }))
    });
    settings::put_settings_with(&*only, &account(), STATION, Some(&other), &SyncedValues { hdr: Some(false), ..Default::default() }).unwrap();
}

#[test]
fn reglages_synchronises_appliques() {
    use crate::profile::{Capabilities, Codec, Profile, StreamSettings};
    let caps = caps_all();
    let jeu = StreamSettings::for_profile(Profile::Jeu, caps);
    let base = StreamSettings { hdr: true, wired: true, ..jeu.clone() };
    // Un profil : ses réglages sur cet appareil, pas ceux reçus.
    let v = SyncedValues { profile: Some(Some("bureau".into())), codec: Some("av1".into()), mbps: Some(12), ..Default::default() };
    let s = v.apply_to(&base, caps);
    let bureau = StreamSettings::for_profile(Profile::Bureau, caps);
    assert_eq!((s.profile, s.codec, s.yuv444, s.mbps, s.reliable), (Some(Profile::Bureau), bureau.codec, bureau.yuv444, bureau.mbps, bureau.reliable));
    assert!(s.hdr && s.wired, "HDR et câble gardés");
    // Personnalisé : format décodable, 4:4:4 s'il existe, débit ; HDR reçu.
    let v = SyncedValues { profile: Some(None), codec: Some("hevc".into()), yuv444: Some(true), mbps: Some(150), hdr: Some(false), ..Default::default() };
    let s = v.apply_to(&base, caps);
    assert_eq!((s.profile, s.codec, s.yuv444, s.mbps, s.hdr), (None, Codec::Hevc, true, 150, false));
    // Un appareil qui ne décode ni l'AV1 ni le 4:4:4 : le format d'avant, 4:2:0.
    let h264 = Capabilities::default();
    let small = StreamSettings::for_profile(Profile::Jeu, h264);
    let v = SyncedValues { profile: Some(None), codec: Some("av1".into()), yuv444: Some(true), mbps: Some(40), ..Default::default() };
    let s = v.apply_to(&small, h264);
    assert_eq!((s.profile, s.codec, s.yuv444, s.mbps), (None, Codec::H264, false, 40));
    // Profil absent : la base reste (son profil), seul le HDR change.
    let s = SyncedValues { hdr: Some(false), mbps: Some(5), ..Default::default() }.apply_to(&base, caps);
    assert_eq!((s.profile, s.mbps, s.hdr), (Some(Profile::Jeu), jeu.mbps, false));
    // Aller-retour : les réglages d'une app donnent les mêmes réglages ailleurs
    // (le transport n'est pas synchronisé).
    let custom = StreamSettings { profile: None, codec: Codec::Hevc, yuv444: true, mbps: 120, reliable: true, hdr: false, wired: false, adaptive: true, encoder_max: None };
    assert_eq!(SyncedValues::from_stream(&custom).apply_to(&base, caps).to_json(), StreamSettings { wired: true, reliable: base.reliable, ..custom.clone() }.to_json(), "transport et câble : ceux de l'appareil");
    assert_eq!(StreamSettings::from_json(&custom.to_json().to_string()).unwrap().to_json(), custom.to_json());
    // Textes de l'interrupteur.
    assert_eq!(
        sync_settings_texts(Lang::Fr),
        (
            "Synchroniser les réglages des ordinateurs".to_string(),
            "Le profil, le format, le débit, le HDR et le mode immersif de chaque ordinateur vous suivent sur vos appareils.".to_string()
        )
    );
    assert_eq!(sync_settings_texts(Lang::En).0, "Sync computer settings");
    assert_eq!(sync_settings_texts(Lang::En).1, "Each computer's profile, format, bitrate, HDR and immersive mode follow you on your devices.");
    assert_eq!(SyncedValues::default().with_immersive(Some("parfois")).immersive, None);
    // TRANS-41 : un débit à la main reçu hors des bornes communes y est ramené.
    let high = SyncedValues { profile: Some(None), codec: Some("hevc".into()), mbps: Some(4294), ..Default::default() };
    assert_eq!(high.apply_to(&base, caps).mbps, 500);
    let pyro = SyncedValues { profile: Some(None), codec: Some("pyrowave".into()), mbps: Some(4294), ..Default::default() };
    assert_eq!(pyro.apply_to(&base, Capabilities { pyrowave: true, ..caps }).mbps, 1500);
    let low = SyncedValues { profile: Some(None), mbps: Some(1), ..Default::default() };
    assert_eq!(low.apply_to(&base, caps).mbps, 5);
}
