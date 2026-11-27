//! L'agent des écrans d'une Station Mac (`kybernow-display`) ou Linux
//! (`kybernow-display-linux`) : un processus à part, à qui Admin parle par
//! lignes JSON, une requête puis sa réponse.
//!
//! Chaque réponse est attendue avec un délai (STMAC-08) : la sortie de l'agent
//! est lue par un fil à part, qui passe les lignes par un canal. Un agent qui
//! ne répond pas à temps (bloqué dans CoreGraphics ou le compositeur, à la
//! sortie de veille par exemple) est arrêté : ses écrans virtuels partent avec
//! lui, ceux qu'il avait éteints sont rallumés par l'agent suivant, et le
//! gestionnaire d'écrans d'Admin ne se fige plus derrière lui.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

/// Délai d'une requête qui change les écrans (créer, placer, retirer, rendre).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// Délai de la veille (`state`, chaque seconde) : courte, elle repassera.
pub const OBSERVE_TIMEOUT: Duration = Duration::from_secs(5);

struct Running {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl Running {
    /// Arrêté pour de bon : tué s'il vit encore, et attendu.
    fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct AgentPipe {
    path: PathBuf,
    running: Mutex<Option<Running>>,
}

impl AgentPipe {
    pub fn new(path: PathBuf) -> Self {
        AgentPipe { path, running: Mutex::new(None) }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// L'agent tourne-t-il (lancé, et pas arrêté depuis) ?
    pub fn running(&self) -> bool {
        let mut running = self.running.lock().unwrap();
        if let Some(r) = running.as_mut()
            && r.child.try_wait().ok().flatten().is_some()
        {
            running.take();
        }
        running.is_some()
    }

    /// Une requête, et sa réponse dans `timeout`. `start` : lancer l'agent s'il ne tourne pas
    /// (sinon, sans agent, il n'y a rien à faire et la réponse est `None`). Un agent arrêté, ou
    /// muet au-delà du délai, est arrêté et oublié : la requête suivante en relance un.
    pub fn call(&self, req: &Value, start: bool, timeout: Duration) -> Result<Option<Value>> {
        let mut running = self.running.lock().unwrap();
        if let Some(r) = running.as_mut()
            && r.child.try_wait().ok().flatten().is_some()
        {
            tracing::warn!("agent des écrans arrêté ({})", self.path.display());
            running.take();
        }
        if running.is_none() {
            if !start {
                return Ok(None);
            }
            *running = Some(self.spawn()?);
        }
        let r = running.as_mut().expect("agent lancé");
        let sent = writeln!(r.stdin, "{req}").and_then(|()| r.stdin.flush());
        let answer = match sent.map_err(|_| RecvTimeoutError::Disconnected).and_then(|()| r.lines.recv_timeout(timeout)) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => {
                running.take().expect("agent lancé").kill();
                tracing::warn!("agent des écrans muet depuis {} s : arrêté ({})", timeout.as_secs(), req["cmd"]);
                bail!("l'agent des écrans ne répond pas (plus de {} s)", timeout.as_secs());
            }
            Err(RecvTimeoutError::Disconnected) => {
                // L'agent s'est arrêté : le système a déjà défait ses écrans.
                running.take().expect("agent lancé").kill();
                bail!("l'agent des écrans ne répond plus");
            }
        };
        let v: Value = serde_json::from_str(&answer).context("réponse de l'agent illisible")?;
        if v["ok"] != json!(true) {
            bail!("{}", v["error"].as_str().unwrap_or("échec de l'agent"));
        }
        Ok(Some(v))
    }

    fn spawn(&self) -> Result<Running> {
        let mut child = Command::new(&self.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("lancement de {}", self.path.display()))?;
        let stdin = child.stdin.take().context("entrée de l'agent")?;
        let out = BufReader::new(child.stdout.take().context("sortie de l'agent")?);
        let (tx, lines) = channel();
        std::thread::Builder::new()
            .name("agent-ecrans".into())
            .spawn(move || {
                for line in out.lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            })
            .context("fil de lecture de l'agent")?;
        tracing::info!("agent des écrans lancé ({})", self.path.display());
        Ok(Running { child, stdin, lines })
    }

    /// Laisse l'agent finir de lui-même (il s'arrête après sa réponse quand il n'a plus d'écran),
    /// au plus `within`, puis l'arrête. (Mac : l'agent Linux reste en marche tant qu'Admin tourne.)
    pub fn finish(&self, within: Duration) {
        let Some(mut r) = self.running.lock().unwrap().take() else { return };
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if r.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        tracing::warn!("agent des écrans toujours là après {} s : arrêté", within.as_secs());
        r.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un faux agent : `{"ok":true}` à chaque ligne, sauf `dors` (muet 30 s) et `fin` (s'arrête).
    fn fake_agent() -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("kn-agent-ecrans-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("agent.sh");
        std::fs::write(
            &path,
            "#!/bin/sh\nwhile read l; do case \"$l\" in *dors*) sleep 30;; *fin*) echo '{\"ok\":true}'; exit 0;; *rate*) echo '{\"ok\":false,\"error\":\"raté\"}';; *) echo '{\"ok\":true,\"n\":1}';; esac; done\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        (dir, path)
    }

    #[test]
    fn reponse_delai_et_relance() {
        let (dir, path) = fake_agent();
        let pipe = AgentPipe::new(path);
        // Sans agent et sans `start` : rien.
        assert!(pipe.call(&json!({ "cmd": "state" }), false, OBSERVE_TIMEOUT).unwrap().is_none());
        assert_eq!(pipe.call(&json!({ "cmd": "create" }), true, CALL_TIMEOUT).unwrap().unwrap()["n"], 1);
        assert!(pipe.running());
        assert_eq!(pipe.call(&json!({ "cmd": "rate" }), false, CALL_TIMEOUT).unwrap_err().to_string(), "raté");
        // Muet : la réponse n'est pas attendue au-delà du délai, et l'agent est arrêté.
        let t = Instant::now();
        let err = pipe.call(&json!({ "cmd": "dors" }), false, Duration::from_millis(300)).unwrap_err();
        assert!(t.elapsed() < Duration::from_secs(5), "délai tenu");
        assert!(err.to_string().contains("ne répond pas"), "{err}");
        assert!(!pipe.running());
        // La requête suivante en relance un.
        assert!(pipe.call(&json!({ "cmd": "state" }), false, OBSERVE_TIMEOUT).unwrap().is_none());
        assert!(pipe.call(&json!({ "cmd": "create" }), true, CALL_TIMEOUT).unwrap().is_some());
        // Il s'arrête de lui-même après sa réponse : `finish` le voit partir.
        assert!(pipe.call(&json!({ "cmd": "fin" }), false, CALL_TIMEOUT).unwrap().is_some());
        let t = Instant::now();
        pipe.finish(Duration::from_secs(5));
        assert!(t.elapsed() < Duration::from_secs(4));
        assert!(!pipe.running());
        // Un agent qui ne part pas : arrêté au bout du délai.
        pipe.call(&json!({ "cmd": "create" }), true, CALL_TIMEOUT).unwrap();
        let t = Instant::now();
        pipe.finish(Duration::from_millis(200));
        assert!(t.elapsed() < Duration::from_secs(3));
        assert!(!pipe.running());
        let _ = std::fs::remove_dir_all(dir);
    }
}
