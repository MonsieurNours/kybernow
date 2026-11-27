//! Les ports d'une console PlayStation que le tunnel fait suivre (docs/relais.md,
//! « Consoles PlayStation »), une seule fois pour le relais, Admin et le cœur des
//! apps : Admin ne joint que ceux-là, le cœur n'ouvre que ceux-là, et les essais
//! du relais font suivre les mêmes.

/// Association, demande de session et canal de contrôle (TCP), recherche de l'association (UDP).
pub const PORT_RP: u16 = 9295;
/// Takion, l'image et les entrées (UDP).
pub const PORT_TAKION: u16 = 9296;
/// État et réveil d'une PS5 (UDP).
pub const PORT_PS5: u16 = 9302;
/// État et réveil d'une PS4 (UDP).
pub const PORT_PS4: u16 = 987;
/// Dialogue de contrôle d'une PS4 (`host-request-port`, TCP).
pub const PORT_PS4_REQUEST: u16 = 997;

/// Ports TCP qu'un appareil peut joindre par le relais.
pub const TCP_PORTS: [u16; 2] = [PORT_RP, PORT_PS4_REQUEST];
/// Ports UDP qu'un appareil peut joindre par le relais.
pub const UDP_PORTS: [u16; 4] = [PORT_RP, PORT_TAKION, PORT_PS5, PORT_PS4];

/// Ce port d'une console se joint-il par le relais, en TCP (`tcp`) ou en UDP ?
pub fn allowed(port: u16, tcp: bool) -> bool {
    if tcp { TCP_PORTS.contains(&port) } else { UDP_PORTS.contains(&port) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_permis() {
        assert!(allowed(9295, true) && allowed(997, true));
        assert!(!allowed(9296, true), "Takion : UDP seulement");
        assert!(allowed(9296, false) && allowed(9302, false) && allowed(987, false) && allowed(9295, false));
        assert!(!allowed(997, false) && !allowed(22, true) && !allowed(9443, false));
    }
}
