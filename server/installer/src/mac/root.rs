//! La seule étape en administrateur : un script shell lancé par AppleScript
//! (`do shell script … with administrator privileges`), pour que macOS
//! demande le mot de passe du Mac une fois, dans sa propre fenêtre.
//!
//! Le script est écrit dans le dossier temporaire de l'utilisateur
//! (`$TMPDIR`, réservé à son compte), sans aucune donnée venue de l'extérieur
//! autrement que citée (`quote`).

use std::path::Path;

use anyhow::{Result, bail};

/// Cite `s` pour le shell : entre apostrophes, les apostrophes fermées et reprises.
pub fn quote(s: impl AsRef<str>) -> String {
    format!("'{}'", s.as_ref().replace('\'', r"'\''"))
}

pub fn quote_path(p: &Path) -> String {
    quote(p.to_string_lossy())
}

/// Chaîne AppleScript entre guillemets.
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Lance `script` (lignes de shell, `set -e`) en administrateur, avec `prompt`
/// dans la fenêtre du mot de passe. Rend la sortie du script ; l'erreur est
/// [`Cancelled`] si l'utilisateur a annulé.
pub fn run(script: &str, prompt: &str) -> Result<String> {
    let dir = std::env::temp_dir().join(format!("kybernow-root-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let path = dir.join("etape-administrateur.sh");
    std::fs::write(&path, format!("#!/bin/sh\nset -e\n{script}\n"))?;
    let command = format!("/bin/sh {}", quote_path(&path));
    let apple = format!(
        "do shell script {} with prompt {} with administrator privileges",
        applescript_string(&command),
        applescript_string(prompt)
    );
    let out = std::process::Command::new("/usr/bin/osascript").args(["-e", &apple]).output();
    let _ = std::fs::remove_dir_all(&dir);
    let out = out?;
    let text = super::output_text(&out);
    if out.status.success() {
        return Ok(text);
    }
    // -128 : « User canceled. »
    if text.contains("(-128)") {
        return Err(Cancelled.into());
    }
    bail!("{text}")
}

/// Mot de passe refusé : « Annuler » dans la fenêtre de macOS.
#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("mot de passe du Mac refusé")
    }
}

impl std::error::Error for Cancelled {}

pub fn is_cancelled(e: &anyhow::Error) -> bool {
    e.is::<Cancelled>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(quote("/Applications/KyberNow Station.app"), "'/Applications/KyberNow Station.app'");
        assert_eq!(quote("l'app"), r"'l'\''app'");
    }

    #[test]
    fn quotes_for_applescript() {
        assert_eq!(applescript_string(r#"/bin/sh '/tmp/a "b"'"#), r#""/bin/sh '/tmp/a \"b\"'""#);
    }
}
