//! Le système d'une Station, d'après `os` de son `GET /api/stream/info`
//! (« Windows 11 Pro 24H2 », « macOS 26.1 », le `PRETTY_NAME` d'une
//! distribution Linux : « Ubuntu 24.04.1 LTS », « Fedora Linux 41… ») :
//! ce qui en dépend dans les apps (« Face à une Station Mac », avertissement
//! du prêt, LINAPP-54). Avant, Android, Windows et Linux le reconnaissaient
//! chacun à leur façon.

/// Le système d'une Station.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationSystem {
    Windows,
    Mac,
    /// Une Station Linux (tout autre système nommé : il n'y a que trois
    /// Stations).
    Linux,
    /// Rien de dit (Station d'avant `os`, info pas encore lue).
    Unknown,
}

impl StationSystem {
    /// D'après `os` : « Windows… », « macOS… » (casse et espaces du début
    /// ignorés), un autre nom, rien.
    pub fn of(os: &str) -> Self {
        let os = os.trim_start();
        let starts = |p: &str| os.get(..p.len()).is_some_and(|s| s.eq_ignore_ascii_case(p));
        if os.trim().is_empty() {
            Self::Unknown
        } else if starts("windows") {
            Self::Windows
        } else if starts("macos") {
            Self::Mac
        } else {
            Self::Linux
        }
    }

    /// Nom stable pour les ponts et les interfaces : `windows`, `mac`,
    /// `linux`, `unknown`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Mac => "mac",
            Self::Linux => "linux",
            Self::Unknown => "unknown",
        }
    }
}

/// [`StationSystem::of`].
pub fn station_system(os: &str) -> StationSystem {
    StationSystem::of(os)
}

/// La Station est un Mac (« Face à une Station Mac », docs/link-ui.md).
pub fn station_is_mac(os: &str) -> bool {
    StationSystem::of(os) == StationSystem::Mac
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemes() {
        assert_eq!(station_system("Windows 11 Pro 24H2"), StationSystem::Windows);
        assert_eq!(station_system(" windows 10"), StationSystem::Windows);
        assert_eq!(station_system("macOS 26.1"), StationSystem::Mac);
        assert_eq!(station_system(" MacOS 15"), StationSystem::Mac);
        assert_eq!(station_system("Ubuntu 24.04.1 LTS"), StationSystem::Linux);
        assert_eq!(station_system("Fedora Linux 41 (Workstation Edition)"), StationSystem::Linux);
        assert_eq!(station_system(""), StationSystem::Unknown);
        assert_eq!(station_system("  "), StationSystem::Unknown);
        // « mac » seul n'est pas « macOS » ; une lettre accentuée ne fait pas paniquer.
        assert_eq!(station_system("mac"), StationSystem::Linux);
        assert_eq!(station_system("é"), StationSystem::Linux);
        assert!(station_is_mac("macOS 15.5") && !station_is_mac("Windows 11") && !station_is_mac(""));
        assert_eq!(StationSystem::Unknown.key(), "unknown");
    }
}
