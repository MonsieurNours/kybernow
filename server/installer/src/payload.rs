//! Contenu à installer, accolé à la fin de `KyberNow-Station-Setup.exe`.
//!
//! Disposition : `[exécutable][archive tar.zst][pied de 32 octets]`.
//! Le pied contient la taille de l'archive, la taille décompressée, le nombre
//! de fichiers, puis la signature `KNPAYLD1`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, bail};

pub const MAGIC: &[u8; 8] = b"KNPAYLD1";
pub const FOOTER_LEN: u64 = 32;

#[derive(Debug, Clone, Copy)]
pub struct Footer {
    pub archive_len: u64,
    pub unpacked_bytes: u64,
    pub files: u64,
}

impl Footer {
    pub fn to_bytes(self) -> [u8; FOOTER_LEN as usize] {
        let mut b = [0u8; FOOTER_LEN as usize];
        b[0..8].copy_from_slice(&self.archive_len.to_le_bytes());
        b[8..16].copy_from_slice(&self.unpacked_bytes.to_le_bytes());
        b[16..24].copy_from_slice(&self.files.to_le_bytes());
        b[24..32].copy_from_slice(MAGIC);
        b
    }
}

/// Lit le pied et renvoie aussi la taille de l'exécutable seul (sans contenu).
pub fn read_footer(path: &Path) -> Result<(Footer, u64)> {
    let mut f = File::open(path).with_context(|| format!("ouverture de {}", path.display()))?;
    let len = f.metadata()?.len();
    if len < FOOTER_LEN {
        bail!(crate::i18n::t("install.payloadIncomplete", &[]));
    }
    f.seek(SeekFrom::Start(len - FOOTER_LEN))?;
    let mut b = [0u8; FOOTER_LEN as usize];
    f.read_exact(&mut b)?;
    if &b[24..32] != MAGIC {
        bail!(crate::i18n::t("install.payloadMissing", &[]));
    }
    let n = |r: std::ops::Range<usize>| u64::from_le_bytes(b[r].try_into().unwrap());
    let footer = Footer { archive_len: n(0..8), unpacked_bytes: n(8..16), files: n(16..24) };
    let exe_len = len - FOOTER_LEN - footer.archive_len;
    Ok((footer, exe_len))
}

/// Décompresse le contenu dans `dest`, en signalant la progression (0.0 → 1.0).
/// Les entrées pour lesquelles `skip` (chemin relatif, sans `./`) répond vrai
/// ne sont pas écrites.
pub fn extract(path: &Path, dest: &Path, skip: impl Fn(&Path) -> bool, mut progress: impl FnMut(f32, &str)) -> Result<u64> {
    let (footer, exe_len) = read_footer(path)?;
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(exe_len))?;
    let archive = f.take(footer.archive_len);
    let decoder = zstd::stream::read::Decoder::new(archive)?;
    let mut tar = tar::Archive::new(decoder);
    std::fs::create_dir_all(dest)?;

    let mut done_bytes: u64 = 0;
    let mut count: u64 = 0;
    for entry in tar.entries()? {
        let mut entry = entry?;
        let size = entry.header().size().unwrap_or(0);
        let relative = entry.path()?.into_owned();
        let relative = relative.strip_prefix(".").unwrap_or(&relative).to_path_buf();
        let name = relative.display().to_string();
        done_bytes += size;
        if skip(&relative) {
            continue;
        }
        if !entry.unpack_in(dest)? {
            bail!("chemin refusé dans le contenu : {name}");
        }
        count += 1;
        if count % 8 == 0 || done_bytes == footer.unpacked_bytes {
            progress((done_bytes as f32 / footer.unpacked_bytes.max(1) as f32).min(1.0), &name);
        }
    }
    progress(1.0, "");
    Ok(count)
}

/// Copie l'exécutable seul (sans contenu) : il sert de désinstalleur.
pub fn copy_stub(path: &Path, dest: &Path) -> Result<()> {
    let (_, exe_len) = read_footer(path)?;
    let mut src = File::open(path)?.take(exe_len);
    let mut out = File::create(dest)?;
    std::io::copy(&mut src, &mut out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faux installateur : un « exécutable », l'archive, le pied.
    fn fake_setup(dir: &Path) -> std::path::PathBuf {
        let mut tar = tar::Builder::new(Vec::new());
        for (name, body) in [("./kyber/kyber_config.toml", "origine"), ("./kyber/kyavserver.exe", "nouveau")] {
            let mut header = tar::Header::new_ustar();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, name, body.as_bytes()).unwrap();
        }
        let archive = zstd::encode_all(&tar.into_inner().unwrap()[..], 3).unwrap();
        let footer = Footer { archive_len: archive.len() as u64, unpacked_bytes: 14, files: 2 };
        let path = dir.join("setup.exe");
        let mut bytes = b"MZ-exe".to_vec();
        bytes.extend_from_slice(&archive);
        bytes.extend_from_slice(&footer.to_bytes());
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn kept_files_are_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("kybernow-payload-{}", std::process::id()));
        let dest = dir.join("dest");
        std::fs::create_dir_all(dest.join("kyber")).unwrap();
        std::fs::write(dest.join("kyber/kyber_config.toml"), "mes réglages").unwrap();
        let setup = fake_setup(&dir);
        let keep = |p: &Path| p == Path::new("kyber/kyber_config.toml");
        let mut last = 0.0;
        assert_eq!(extract(&setup, &dest, keep, |p, _| last = p).unwrap(), 1);
        assert_eq!(last, 1.0);
        assert_eq!(std::fs::read_to_string(dest.join("kyber/kyber_config.toml")).unwrap(), "mes réglages");
        assert_eq!(std::fs::read_to_string(dest.join("kyber/kyavserver.exe")).unwrap(), "nouveau");
        assert_eq!(extract(&setup, &dest, |_| false, |_, _| {}).unwrap(), 2);
        assert_eq!(std::fs::read_to_string(dest.join("kyber/kyber_config.toml")).unwrap(), "origine");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
