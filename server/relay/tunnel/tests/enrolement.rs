//! L'enrôlement d'une Station (docs/comptes.md, « Enrôlement ») avec le
//! relais simulé : `enroll` part sur la connexion de contrôle, la réponse
//! `attached` (ou le refus) revient à celui qui l'attend.

use std::sync::Arc;
use std::time::Duration;

use kybernow_tunnel::msg::{AttachOwner, Attached, ErrorCode, Msg};
use kybernow_tunnel::testing::FakeRelay;
use kybernow_tunnel::{Agent, AgentConfig, StationIdentity, TunnelError};

fn answer(msg: &Msg) -> Option<Msg> {
    match msg {
        Msg::Enroll { req, token, .. } if token == "kne_bon" => Some(Msg::Attached(Attached {
            req: Some(*req),
            relay: "relais.test".into(),
            account: "acc_1".into(),
            owner: AttachOwner { kind: "organization".into(), id: "org_1".into(), name: "Les Lignes Claires".into(), email: None },
            keys: Vec::new(),
        })),
        Msg::Enroll { req, .. } => Some(Msg::error(Some(*req), ErrorCode::EnrollRefused, Some("revoked".into()))),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn enrolement_par_la_connexion_de_controle() {
    let relay = FakeRelay::start(answer).await;
    let identity = Arc::new(StationIdentity::generate().unwrap());
    let (agent, mut incoming) = Agent::start(AgentConfig {
        relay: relay.addr.clone(),
        identity,
        admin_fp: format!("sha256:{}", ["ab"; 32].join(":")),
        version: "essai".into(),
        roots: None,
        force_tcp: false,
        browser_tls: false,
    });

    let a = agent.enroll("kne_bon", "GRAPH-01").await.unwrap();
    assert_eq!(a.owner.id, "org_1");
    assert_eq!(a.relay, "relais.test");
    // La réponse revient à l'appel, pas dans le courrier d'Admin.
    assert!(tokio::time::timeout(Duration::from_millis(200), incoming.recv()).await.is_err());

    match agent.enroll("kne_faux", "GRAPH-01").await {
        Err(TunnelError::Refused(ErrorCode::EnrollRefused, Some(why))) if why == "revoked" => {}
        other => panic!("refus attendu : {other:?}"),
    }
    let sent: Vec<Msg> = relay.received().into_iter().filter(|m| matches!(m, Msg::Enroll { .. })).collect();
    assert_eq!(sent.len(), 2);
    assert!(matches!(&sent[0], Msg::Enroll { token, name, .. } if token == "kne_bon" && name == "GRAPH-01"));
}

#[tokio::test(flavor = "multi_thread")]
async fn un_relais_qui_ne_connait_pas_l_enrolement_ne_repond_pas() {
    let relay = FakeRelay::start(|_| None).await;
    let (agent, _incoming) = Agent::start(AgentConfig {
        relay: relay.addr.clone(),
        identity: Arc::new(StationIdentity::generate().unwrap()),
        admin_fp: format!("sha256:{}", ["ab"; 32].join(":")),
        version: "essai".into(),
        roots: None,
        force_tcp: false,
        browser_tls: false,
    });
    assert!(matches!(agent.enroll("kne_bon", "PC").await, Err(TunnelError::Timeout)));
}
