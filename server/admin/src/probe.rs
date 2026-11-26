//! Test du profil Auto des apps (docs/profil-auto.md, « Côté Station ») :
//! `GET /api/stream/ping`, une réponse vide immédiate (aller-retour), et
//! `GET /api/stream/probe?ms=…`, une rafale d'octets aléatoires envoyés aussi
//! vite que la connexion les prend, pendant `ms` millisecondes (au plus
//! 6 000), au plus 2 Gbit/s. Une seule rafale à la fois (429 sinon).
//!
//! La rafale passe par HTTPS (TCP), par le réseau ou le tunnel du relais :
//! elle mesure le chemin vers l'appareil, pas l'encodeur.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use http_body::Frame;
use serde::Deserialize;
use tokio::time::{Instant, Sleep};

use crate::AppState;
use crate::api::{ApiError, ApiResult, stream_user};
use crate::i18n::Msg;

/// Durée la plus longue d'une rafale.
pub const MAX_MS: u64 = 6000;
/// Débit le plus haut d'une rafale, en octets par seconde (2 Gbit/s).
const MAX_BYTES_PER_SEC: f64 = 2e9 / 8.0;
/// Taille d'un morceau envoyé.
const CHUNK: usize = 64 * 1024;

static BUSY: AtomicBool = AtomicBool::new(false);

/// `GET /api/stream/ping` : rien, tout de suite.
pub async fn ping() -> impl IntoResponse {
    (StatusCode::NO_CONTENT, [(header::CACHE_CONTROL, "no-store")])
}

#[derive(Deserialize)]
pub struct ProbeQuery {
    ms: Option<u64>,
}

/// `GET /api/stream/probe?ms=4000` : la rafale (droit Session, comme
/// `/api/stream/info`).
pub async fn stream_probe(State(s): State<AppState>, headers: HeaderMap, Query(q): Query<ProbeQuery>) -> ApiResult<Response> {
    stream_user(&s, &headers)?;
    if BUSY.swap(true, Ordering::AcqRel) {
        return Err(ApiError::msg(StatusCode::TOO_MANY_REQUESTS, Msg::new("api.probeBusy")));
    }
    let ms = q.ms.unwrap_or(4000).clamp(250, MAX_MS);
    let body = Burst::new(Duration::from_millis(ms));
    Ok((
        [(header::CONTENT_TYPE, "application/octet-stream"), (header::CACHE_CONTROL, "no-store")],
        Body::new(body),
    )
        .into_response())
}

/// Le corps de la rafale : le même morceau aléatoire, envoyé jusqu'à la fin
/// de la durée, sans dépasser [`MAX_BYTES_PER_SEC`].
struct Burst {
    chunk: Bytes,
    started: Instant,
    until: Instant,
    sent: u64,
    wait: Option<Pin<Box<Sleep>>>,
}

impl Burst {
    fn new(duration: Duration) -> Self {
        let mut data = vec![0u8; CHUNK];
        rand::fill(&mut data[..]);
        let now = Instant::now();
        Self { chunk: Bytes::from(data), started: now, until: now + duration, sent: 0, wait: None }
    }

    /// Quand le prochain morceau peut partir sans dépasser le plafond.
    fn next_at(&self) -> Instant {
        self.started + Duration::from_secs_f64(self.sent as f64 / MAX_BYTES_PER_SEC)
    }
}

impl Drop for Burst {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

impl http_body::Body for Burst {
    type Data = Bytes;
    type Error = std::convert::Infallible;

    fn poll_frame(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        if let Some(wait) = self.wait.as_mut() {
            if wait.as_mut().poll(cx).is_pending() {
                return Poll::Pending;
            }
            self.wait = None;
        }
        let now = Instant::now();
        if now >= self.until {
            return Poll::Ready(None);
        }
        let at = self.next_at();
        if at > now {
            let mut sleep = Box::pin(tokio::time::sleep_until(at.min(self.until)));
            if sleep.as_mut().poll(cx).is_pending() {
                self.wait = Some(sleep);
                return Poll::Pending;
            }
        }
        self.sent += CHUNK as u64;
        Poll::Ready(Some(Ok(Frame::data(self.chunk.clone()))))
    }
}

use std::future::Future;

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn la_rafale_dure_ce_qui_est_demande() {
        let started = std::time::Instant::now();
        let body = Burst::new(Duration::from_millis(300));
        let bytes = body.collect().await.unwrap().to_bytes();
        let elapsed = started.elapsed();
        assert!(elapsed >= Duration::from_millis(290) && elapsed < Duration::from_millis(1500), "{elapsed:?}");
        assert!(!bytes.is_empty());
        // Au plus 2 Gbit/s (avec un morceau d'avance).
        assert!(bytes.len() as f64 <= MAX_BYTES_PER_SEC * 0.3 + CHUNK as f64 * 2.0, "{}", bytes.len());
        assert!(!BUSY.load(Ordering::Acquire), "rafale finie : une autre peut partir");
    }
}
