//! Les quelques messages protobuf de Takion dont la session a besoin
//! (`lib/protobuf/takion.proto` de chiaki-ng), écrits et lus à la main :
//! entiers (varint) et champs de longueur (octets, texte, sous-messages).

/// `TakionMessage.PayloadType`.
pub mod kind {
    pub const BIG: u64 = 0;
    pub const BANG: u64 = 1;
    pub const HEARTBEAT: u64 = 3;
    pub const CORRUPTFRAME: u64 = 5;
    pub const DISCONNECT: u64 = 8;
    pub const STREAMINFO: u64 = 13;
    pub const STREAMINFOACK: u64 = 14;
    pub const CONNECTIONQUALITY: u64 = 16;
    pub const CONTROLLERCONNECTION: u64 = 21;
    pub const IDRREQUEST: u64 = 25;
}

/// `ControllerConnectionPayload.ControllerType`.
pub const CONTROLLER_DUALSHOCK4: u64 = 2;
pub const CONTROLLER_DUALSENSE: u64 = 6;

/// Écriture d'un message.
#[derive(Default)]
pub struct Writer(pub Vec<u8>);

fn varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

impl Writer {
    pub fn uint(mut self, field: u32, v: u64) -> Self {
        varint(&mut self.0, (field as u64) << 3);
        varint(&mut self.0, v);
        self
    }

    pub fn bytes(mut self, field: u32, b: &[u8]) -> Self {
        varint(&mut self.0, ((field as u64) << 3) | 2);
        varint(&mut self.0, b.len() as u64);
        self.0.extend_from_slice(b);
        self
    }

    pub fn message(self, field: u32, m: Writer) -> Self {
        self.bytes(field, &m.0)
    }

    pub fn finish(self) -> Vec<u8> {
        self.0
    }
}

/// Un champ lu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value<'a> {
    Uint(u64),
    Bytes(&'a [u8]),
}

/// Les champs d'un message, dans l'ordre. `None` si le message est illisible.
pub fn fields(mut b: &[u8]) -> Option<Vec<(u32, Value<'_>)>> {
    fn read_varint(b: &mut &[u8]) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let (&byte, rest) = b.split_first()?;
            *b = rest;
            v |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
    let mut out = Vec::new();
    while !b.is_empty() {
        let key = read_varint(&mut b)?;
        let field = (key >> 3) as u32;
        match key & 7 {
            0 => out.push((field, Value::Uint(read_varint(&mut b)?))),
            2 => {
                let len = read_varint(&mut b)? as usize;
                if len > b.len() {
                    return None;
                }
                let (v, rest) = b.split_at(len);
                out.push((field, Value::Bytes(v)));
                b = rest;
            }
            5 => b = b.get(4..)?,
            1 => b = b.get(8..)?,
            _ => return None,
        }
    }
    Some(out)
}

/// Premier entier du champ `field`.
pub fn uint(f: &[(u32, Value<'_>)], field: u32) -> Option<u64> {
    f.iter().find_map(|(n, v)| match v {
        Value::Uint(x) if *n == field => Some(*x),
        _ => None,
    })
}

/// Premier champ de longueur `field`.
pub fn bytes<'a>(f: &[(u32, Value<'a>)], field: u32) -> Option<&'a [u8]> {
    f.iter().find_map(|(n, v)| match v {
        Value::Bytes(x) if *n == field => Some(*x),
        _ => None,
    })
}

/// Tous les champs de longueur `field` (champs répétés).
pub fn all_bytes<'a>(f: &[(u32, Value<'a>)], field: u32) -> Vec<&'a [u8]> {
    f.iter()
        .filter_map(|(n, v)| match v {
            Value::Bytes(x) if *n == field => Some(*x),
            _ => None,
        })
        .collect()
}

/// `TakionMessage` sans contenu : seulement son type.
pub fn simple(kind: u64) -> Vec<u8> {
    Writer::default().uint(1, kind).finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let m = Writer::default()
            .uint(1, kind::BIG)
            .message(2, Writer::default().uint(1, 12).bytes(2, b"abc").bytes(5, &[0u8; 200]))
            .finish();
        let f = fields(&m).unwrap();
        assert_eq!(uint(&f, 1), Some(0));
        let inner = fields(bytes(&f, 2).unwrap()).unwrap();
        assert_eq!(uint(&inner, 1), Some(12));
        assert_eq!(bytes(&inner, 2), Some(&b"abc"[..]));
        assert_eq!(bytes(&inner, 5).unwrap().len(), 200);
    }

    #[test]
    fn heartbeat_is_two_bytes() {
        // Ce qu'envoie chiaki-ng (nanopb) : le type seulement.
        assert_eq!(simple(kind::HEARTBEAT), vec![0x08, 0x03]);
        assert_eq!(simple(kind::STREAMINFOACK), vec![0x08, 0x0e]);
    }

    #[test]
    fn rejects_truncated() {
        assert_eq!(fields(&[0x12, 0x05, 1, 2]), None);
        assert_eq!(fields(&[0xff]), None);
    }
}
