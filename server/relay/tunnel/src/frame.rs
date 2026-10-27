//! Trames et datagrammes (docs/relais.md) :
//!
//! - une trame : longueur sur 4 octets gros-boutistes ([`MAX_FRAME`] au plus),
//!   puis un [`Msg`] en JSON ;
//! - un datagramme : `[numéro de flux, entier variable de QUIC][paquet UDP]` ;
//! - un enregistrement du repli TCP : `[longueur de la suite, 2 octets][datagramme]`.

use std::io;

use bytes::{BufMut, Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::msg::Msg;

/// Taille d'une trame, au plus.
pub const MAX_FRAME: usize = 65_536;
/// Plus grand entier variable de QUIC.
pub const MAX_VARINT: u64 = (1 << 62) - 1;

/// La trame d'un message, longueur comprise.
pub fn encode(msg: &Msg) -> Vec<u8> {
    let body = serde_json::to_vec(msg).expect("un message se met toujours en JSON");
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

/// Écrit un message (et vide le tampon).
pub async fn write_msg<W: AsyncWrite + Unpin + ?Sized>(w: &mut W, msg: &Msg) -> io::Result<()> {
    let frame = encode(msg);
    if frame.len() - 4 > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "trame trop longue"));
    }
    w.write_all(&frame).await?;
    w.flush().await
}

/// Lit un message ; `None` si le flux se ferme proprement avant une trame.
pub async fn read_msg<R: AsyncRead + Unpin + ?Sized>(r: &mut R) -> io::Result<Option<Msg>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "trame trop longue"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    serde_json::from_slice(&body).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Longueur d'un entier variable de QUIC.
pub fn varint_len(v: u64) -> usize {
    match v {
        0..=63 => 1,
        64..=16_383 => 2,
        16_384..=1_073_741_823 => 4,
        _ => 8,
    }
}

/// Écrit un entier variable de QUIC (RFC 9000, § 16).
pub fn put_varint(buf: &mut impl BufMut, v: u64) {
    assert!(v <= MAX_VARINT, "entier variable trop grand");
    match varint_len(v) {
        1 => buf.put_u8(v as u8),
        2 => buf.put_u16(0x4000 | v as u16),
        4 => buf.put_u32(0x8000_0000 | v as u32),
        _ => buf.put_u64(0xc000_0000_0000_0000 | v),
    }
}

/// Lit un entier variable de QUIC : la valeur et sa longueur.
pub fn get_varint(buf: &[u8]) -> Option<(u64, usize)> {
    let first = *buf.first()?;
    let len = 1usize << (first >> 6);
    if buf.len() < len {
        return None;
    }
    let mut v = u64::from(first & 0x3f);
    for b in &buf[1..len] {
        v = (v << 8) | u64::from(*b);
    }
    Some((v, len))
}

/// Un datagramme : le numéro de flux puis le paquet.
pub fn encode_datagram(flow: u64, payload: &[u8]) -> Bytes {
    let mut buf = BytesMut::with_capacity(varint_len(flow) + payload.len());
    put_varint(&mut buf, flow);
    buf.put_slice(payload);
    buf.freeze()
}

/// Le numéro de flux et le paquet d'un datagramme.
pub fn decode_datagram(datagram: &Bytes) -> Option<(u64, Bytes)> {
    let (flow, n) = get_varint(datagram)?;
    Some((flow, datagram.slice(n..)))
}

/// Un enregistrement du repli TCP : longueur sur 2 octets, puis le datagramme.
pub fn encode_record(datagram: &[u8]) -> Option<Bytes> {
    let len = u16::try_from(datagram.len()).ok()?;
    let mut buf = BytesMut::with_capacity(2 + datagram.len());
    buf.put_u16(len);
    buf.put_slice(datagram);
    Some(buf.freeze())
}

/// Lit un enregistrement du repli TCP ; `None` à la fin du flux.
pub async fn read_record<R: AsyncRead + Unpin + ?Sized>(r: &mut R) -> io::Result<Option<Bytes>> {
    let mut len = [0u8; 2];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let mut body = vec![0u8; u16::from_be_bytes(len) as usize];
    r.read_exact(&mut body).await?;
    Ok(Some(Bytes::from(body)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entiers_variables_de_quic() {
        // Exemples de la RFC 9000, annexe A.1.
        for (bytes, value) in [
            (&[0xc2, 0x19, 0x7c, 0x5e, 0xff, 0x14, 0xe8, 0x8c][..], 151_288_809_941_952_652u64),
            (&[0x9d, 0x7f, 0x3e, 0x7d][..], 494_878_333),
            (&[0x7b, 0xbd][..], 15_293),
            (&[0x25][..], 37),
        ] {
            assert_eq!(get_varint(bytes), Some((value, bytes.len())));
            let mut out = Vec::new();
            put_varint(&mut out, value);
            assert_eq!(out, bytes);
        }
        assert_eq!(get_varint(&[0x40]), None);
        assert_eq!(get_varint(&[]), None);
    }

    #[test]
    fn datagramme_aller_retour() {
        let d = encode_datagram(300, b"paquet");
        assert_eq!(d.len(), 2 + 6);
        let (flow, payload) = decode_datagram(&d).unwrap();
        assert_eq!(flow, 300);
        assert_eq!(&payload[..], b"paquet");
        assert!(encode_record(&vec![0u8; 70_000]).is_none());
    }

    #[tokio::test]
    async fn trames_aller_retour() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        write_msg(&mut a, &Msg::Ping { n: 7 }).await.unwrap();
        write_msg(&mut a, &Msg::Release { code: "123456".into() }).await.unwrap();
        drop(a);
        assert_eq!(read_msg(&mut b).await.unwrap(), Some(Msg::Ping { n: 7 }));
        assert_eq!(read_msg(&mut b).await.unwrap(), Some(Msg::Release { code: "123456".into() }));
        assert_eq!(read_msg(&mut b).await.unwrap(), None);
    }

    #[tokio::test]
    async fn trame_trop_longue_refusee() {
        let (mut a, mut b) = tokio::io::duplex(64);
        tokio::spawn(async move {
            let _ = a.write_all(&(MAX_FRAME as u32 + 1).to_be_bytes()).await;
        });
        assert!(read_msg(&mut b).await.is_err());
    }
}
