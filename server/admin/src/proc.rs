//! Lancer un outil et attendre sa fin, avec un délai (STWIN-44, STCOM-07) :
//! sans fenêtre sous Windows, sorties lues au fil de l'eau dans des fils (un
//! tube plein bloquerait l'outil, et lire après la fin perdait ce qu'il a dit
//! avant d'être tué), outil arrêté puis attendu au-delà du délai (pas de
//! zombie). Une seule copie pour l'encodeur, le micro, la caméra, PyroWave,
//! les manettes de la Station Mac et la capture de l'écran sous Linux.

use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// `CREATE_NO_WINDOW` : un outil en console lancé par le service n'ouvre
/// pas de fenêtre sur le bureau.
#[cfg_attr(not(windows), allow(dead_code))]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Pas de fenêtre de console pour `cmd` (Windows ; rien ailleurs).
pub fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Ce que l'outil a rendu.
#[derive(Debug)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn code(&self) -> Option<i32> {
        self.status.code()
    }
}

#[derive(Debug)]
pub enum Failure {
    /// L'outil ne se lance pas.
    Spawn(std::io::Error),
    /// Délai dépassé : arrêté. Ce qu'il avait dit avant.
    Timeout { stderr: String },
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Spawn(e) => write!(f, "ne se lance pas ({e})"),
            Failure::Timeout { .. } => write!(f, "délai dépassé"),
        }
    }
}

fn reader(stream: Option<impl Read + Send + 'static>) -> Option<JoinHandle<String>> {
    stream.map(|mut s| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        })
    })
}

/// Lance `cmd` (entrée fermée, sorties lues) et attend sa fin, `timeout` au plus.
pub fn run(mut cmd: Command, timeout: Duration) -> Result<Output, Failure> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    no_window(&mut cmd);
    let mut child = cmd.spawn().map_err(Failure::Spawn)?;
    let (out, err) = (reader(child.stdout.take()), reader(child.stderr.take()));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let join = |h: Option<JoinHandle<String>>| h.and_then(|h| h.join().ok()).unwrap_or_default();
    let (stdout, stderr) = (join(out), join(err));
    match status {
        Some(status) => Ok(Output { status, stdout, stderr }),
        None => Err(Failure::Timeout { stderr }),
    }
}

/// Le code de sortie et la sortie d'erreur (sans espaces autour) ; `None`
/// si l'outil ne se lance pas ou dépasse son délai.
pub fn code_and_stderr(cmd: Command, timeout: Duration) -> Option<(Option<i32>, String)> {
    run(cmd, timeout).ok().map(|o| (o.code(), o.stderr.trim().to_owned()))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", script]);
        cmd
    }

    #[test]
    fn sortie_code_et_delai() {
        let out = run(sh("echo dit; echo erreur >&2; exit 3"), Duration::from_secs(10)).unwrap();
        assert_eq!((out.code(), out.stdout.trim(), out.stderr.trim()), (Some(3), "dit", "erreur"));
        // Plus que la taille d'un tube : lu au fil de l'eau, l'outil ne bloque pas.
        let big = run(sh("head -c 300000 /dev/zero | tr '\\0' x >&2"), Duration::from_secs(10)).unwrap();
        assert_eq!(big.stderr.len(), 300_000);
        // Délai dépassé : arrêté, ce qu'il a dit avant gardé.
        let started = Instant::now();
        match run(sh("echo avant >&2; exec sleep 30"), Duration::from_millis(300)) {
            Err(Failure::Timeout { stderr }) => assert_eq!(stderr.trim(), "avant"),
            other => panic!("{other:?}"),
        }
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(matches!(run(Command::new("/nexiste/pas"), Duration::from_secs(1)), Err(Failure::Spawn(_))));
        assert_eq!(code_and_stderr(sh("echo ' prêt ' >&2"), Duration::from_secs(10)), Some((Some(0), "prêt".into())));
        assert_eq!(code_and_stderr(sh("exec sleep 5"), Duration::from_millis(200)), None);
    }
}
