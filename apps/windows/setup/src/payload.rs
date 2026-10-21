//! Les deux versions de l'app, accolées à la fin de `KyberNow-Client-Setup.exe`.
//!
//! Disposition : `[exécutable][archive x64][archive arm64][pied de 64 octets]`.
//! Chaque archive est un tar compressé en zstd. Le pied donne, pour chaque
//! version, la taille de l'archive, la taille décompressée et le nombre de
//! fichiers, puis la signature `KNCLNT01`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, bail};

pub const MAGIC: &[u8; 8] = b"KNCLNT01";
pub const FOOTER_LEN: u64 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    X64,
    Arm64,
}

impl Arch {
    pub fn label(self) -> &'static str {
        match self {
            Arch::X64 => "x64",
            Arch::Arm64 => "ARM64",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Part {
    pub archive_len: u64,
    pub unpacked_bytes: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Footer {
    pub x64: Part,
    pub arm64: Part,
}

impl Footer {
    pub fn to_bytes(self) -> [u8; FOOTER_LEN as usize] {
        let mut b = [0u8; FOOTER_LEN as usize];
        let fields = [
            self.x64.archive_len,
            self.x64.unpacked_bytes,
            self.x64.files,
            self.arm64.archive_len,
            self.arm64.unpacked_bytes,
            self.arm64.files,
            0,
        ];
        for (i, v) in fields.iter().enumerate() {
            b[i * 8..i * 8 + 8].copy_from_slice(&v.to_le_bytes());
        }
        b[56..64].copy_from_slice(MAGIC);
        b
    }

    fn part(&self, arch: Arch) -> Part {
        match arch {
            Arch::X64 => self.x64,
            Arch::Arm64 => self.arm64,
        }
    }
}

/// Lit le pied et renvoie aussi la taille de l'exécutable seul (sans contenu).
pub fn read_footer(path: &Path) -> Result<(Footer, u64)> {
    let mut f = File::open(path).with_context(|| format!("ouverture de {}", path.display()))?;
    let len = content_end(&mut f)?;
    if len < FOOTER_LEN {
        bail!("installateur incomplet");
    }
    f.seek(SeekFrom::Start(len - FOOTER_LEN))?;
    let mut b = [0u8; FOOTER_LEN as usize];
    f.read_exact(&mut b)?;
    if &b[56..64] != MAGIC {
        bail!("l'app est introuvable dans l'installateur (fichier abîmé ?)");
    }
    let n = |i: usize| u64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap());
    let footer = Footer {
        x64: Part { archive_len: n(0), unpacked_bytes: n(1), files: n(2) },
        arm64: Part { archive_len: n(3), unpacked_bytes: n(4), files: n(5) },
    };
    let archives = footer.x64.archive_len.checked_add(footer.arm64.archive_len);
    let exe_len = archives.and_then(|a| (len - FOOTER_LEN).checked_sub(a)).context("installateur abîmé")?;
    Ok((footer, exe_len))
}

/// Fin du contenu : une signature Authenticode (à venir) s'ajoute après le
/// pied, avec au plus 7 octets nuls d'alignement. La table des certificats de
/// l'en-tête PE dit où elle commence.
fn content_end(f: &mut File) -> Result<u64> {
    let len = f.metadata()?.len();
    let read_u32 = |f: &mut File, at: u64| -> Option<u32> {
        let mut b = [0u8; 4];
        f.seek(SeekFrom::Start(at)).ok()?;
        f.read_exact(&mut b).ok()?;
        Some(u32::from_le_bytes(b))
    };
    let cert = (|| {
        let pe = read_u32(f, 0x3C)? as u64;
        if read_u32(f, pe)? != 0x0000_4550 {
            return None; // « PE\0\0 »
        }
        let optional = pe + 24;
        let magic = read_u32(f, optional)? & 0xFFFF;
        let dirs = optional + if magic == 0x20B { 112 } else { 96 };
        let (at, size) = (read_u32(f, dirs + 4 * 8)? as u64, read_u32(f, dirs + 4 * 8 + 4)? as u64);
        (at != 0 && size != 0 && at + size == len).then_some(at)
    })();
    let Some(mut end) = cert else { return Ok(len) };
    // Octets d'alignement avant la signature.
    for _ in 0..7 {
        let mut b = [0u8; 1];
        f.seek(SeekFrom::Start(end - 1))?;
        f.read_exact(&mut b)?;
        if b[0] != 0 {
            break;
        }
        end -= 1;
    }
    Ok(end)
}

/// Taille décompressée de la version `arch` (pour la place sur le disque).
pub fn unpacked_bytes(path: &Path, arch: Arch) -> Result<u64> {
    Ok(read_footer(path)?.0.part(arch).unpacked_bytes)
}

/// Décompresse la version `arch` dans `dest`, en signalant la progression (0.0 → 1.0).
pub fn extract(path: &Path, arch: Arch, dest: &Path, mut progress: impl FnMut(f32, &str)) -> Result<u64> {
    let (footer, exe_len) = read_footer(path)?;
    let part = footer.part(arch);
    if part.archive_len == 0 {
        bail!("cet installateur ne contient pas la version {} de KyberNow", arch.label());
    }
    let start = match arch {
        Arch::X64 => exe_len,
        Arch::Arm64 => exe_len + footer.x64.archive_len,
    };
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(start))?;
    let decoder = zstd::stream::read::Decoder::new(f.take(part.archive_len))?;
    let mut tar = tar::Archive::new(decoder);
    std::fs::create_dir_all(dest)?;

    let (mut done_bytes, mut count) = (0u64, 0u64);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let size = entry.header().size().unwrap_or(0);
        let file = entry.header().entry_type().is_file();
        let name = entry.path()?.display().to_string();
        if !entry.unpack_in(dest)? {
            bail!("chemin refusé dans le contenu : {name}");
        }
        // Les dossiers (dont « ./ ») ne comptent pas parmi les fichiers.
        if !file {
            continue;
        }
        done_bytes += size;
        count += 1;
        if count % 4 == 0 {
            progress((done_bytes as f32 / part.unpacked_bytes.max(1) as f32).min(1.0), &name);
        }
    }
    if count != part.files {
        bail!("contenu incomplet : {count} fichiers sur {}", part.files);
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

    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        // Comme `tar -C dossier .` : une entrée pour le dossier lui-même.
        let mut dir = tar::Header::new_ustar();
        dir.set_entry_type(tar::EntryType::Directory);
        dir.set_size(0);
        dir.set_mode(0o755);
        dir.set_cksum();
        tar.append_data(&mut dir, "./", std::io::empty()).unwrap();
        for (name, body) in files {
            let mut header = tar::Header::new_ustar();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, name, body.as_bytes()).unwrap();
        }
        zstd::encode_all(&tar.into_inner().unwrap()[..], 3).unwrap()
    }

    #[test]
    fn each_arch_gets_its_own_files() {
        let dir = std::env::temp_dir().join(format!("kn-client-payload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let x = archive(&[("./kybernow.exe", "x64"), ("./kynput.dll", "dll-x64")]);
        let a = archive(&[("./kybernow.exe", "arm64")]);
        let footer = Footer {
            x64: Part { archive_len: x.len() as u64, unpacked_bytes: 10, files: 2 },
            arm64: Part { archive_len: a.len() as u64, unpacked_bytes: 5, files: 1 },
        };
        let setup = dir.join("setup.exe");
        let mut bytes = b"MZ-stub".to_vec();
        bytes.extend_from_slice(&x);
        bytes.extend_from_slice(&a);
        bytes.extend_from_slice(&footer.to_bytes());
        std::fs::write(&setup, bytes).unwrap();

        assert_eq!(read_footer(&setup).unwrap(), (footer, 7));
        let mut last = 0.0;
        assert_eq!(extract(&setup, Arch::Arm64, &dir.join("arm"), |p, _| last = p).unwrap(), 1);
        assert_eq!(last, 1.0);
        assert_eq!(std::fs::read_to_string(dir.join("arm/kybernow.exe")).unwrap(), "arm64");
        assert!(!dir.join("arm/kynput.dll").exists());
        assert_eq!(extract(&setup, Arch::X64, &dir.join("x"), |_, _| {}).unwrap(), 2);
        assert_eq!(std::fs::read_to_string(dir.join("x/kynput.dll")).unwrap(), "dll-x64");

        copy_stub(&setup, &dir.join("uninstall.exe")).unwrap();
        assert_eq!(std::fs::read(dir.join("uninstall.exe")).unwrap(), b"MZ-stub");
        assert!(read_footer(&dir.join("uninstall.exe")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Une signature Authenticode ajoutée après le pied (avec alignement) ne gêne pas.
    #[test]
    fn signed_installer_still_reads() {
        let dir = std::env::temp_dir().join(format!("kn-client-signed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let x = archive(&[("./kybernow.exe", "x64")]);
        let footer = Footer { x64: Part { archive_len: x.len() as u64, unpacked_bytes: 3, files: 1 }, ..Default::default() };
        // En-tête PE minimal (PE32+) : table des certificats dans le 5e répertoire.
        let mut stub = vec![0u8; 0x100];
        stub[..2].copy_from_slice(b"MZ");
        stub[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        stub[0x40..0x44].copy_from_slice(b"PE\0\0");
        stub[0x58..0x5A].copy_from_slice(&0x20Bu16.to_le_bytes());
        let mut bytes = stub.clone();
        bytes.extend_from_slice(&x);
        bytes.extend_from_slice(&footer.to_bytes());
        let unsigned_len = bytes.len();
        bytes.extend_from_slice(&[0, 0, 0]);
        let cert_at = bytes.len() as u32;
        bytes.extend_from_slice(&[7u8; 16]);
        bytes[0xE8..0xEC].copy_from_slice(&cert_at.to_le_bytes());
        bytes[0xEC..0xF0].copy_from_slice(&16u32.to_le_bytes());
        let setup = dir.join("signed.exe");
        std::fs::write(&setup, &bytes).unwrap();
        let (read, exe_len) = read_footer(&setup).unwrap();
        assert_eq!((read, exe_len), (footer, 0x100));
        assert!(unsigned_len > 0);
        assert_eq!(extract(&setup, Arch::X64, &dir.join("x"), |_, _| {}).unwrap(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Installateur construit : `KN_SETUP=…/KyberNow-Client-Setup.exe cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn built_installer_extracts() {
        let setup = std::path::PathBuf::from(std::env::var("KN_SETUP").expect("KN_SETUP"));
        let dir = std::env::temp_dir().join(format!("kn-client-built-{}", std::process::id()));
        for arch in [Arch::X64, Arch::Arm64] {
            let dest = dir.join(arch.label());
            assert!(extract(&setup, arch, &dest, |_, _| {}).unwrap() >= 5);
            assert!(dest.join("kybernow.exe").exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
