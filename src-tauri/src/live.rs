//! Live collaboration over the local network: a WebSocket relay every Pergel in the session
//! (the host's own window included) connects to. Messages are JSON text — cursors, documents —
//! and the relay forwards each one to everybody but its sender. Off until the user shares; a
//! random 128-bit token in the link keeps strangers on the network out. It listens on this
//! machine and its local network address only, never on every interface, and limits how many
//! people join and how fast each one sends. MCP stays on 127.0.0.1.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

pub const PORT: u16 = 3856;
/// People in one session at most (each window is one connection).
const MAX_PEOPLE: usize = 32;
/// Documents with images are large.
const MAX_MESSAGE: usize = 64 << 20;
/// Messages a person may send per second on average, and in a burst. Cursors go out 25 times a
/// second; more than this is dropped, not relayed.
const RATE: f64 = 100.0;
const BURST: f64 = 400.0;
/// Windows say they are here every 3 seconds; one silent for this long has gone.
const SILENCE: Duration = Duration::from_secs(45);

#[derive(Clone)]
struct Hub {
    tx: broadcast::Sender<(u64, String)>,
    token: String,
    next: std::sync::Arc<AtomicU64>,
    people: std::sync::Arc<AtomicUsize>,
}

struct Running {
    token: String,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LiveInfo {
    /// What this window connects to.
    pub local_url: String,
    /// The link to give to others on the network.
    pub share_url: String,
}

#[derive(Deserialize)]
struct Join {
    token: Option<String>,
}

/// This machine's address on the local network (no packet is sent: UDP connect only picks the
/// route).
fn lan_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("10.255.255.255:1")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

/// 128 random bits from the operating system, as hex.
fn token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("No randomness for the session token: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compares in time independent of where the strings differ, so the token can't be guessed
/// byte by byte from response times.
fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn info(token: &str) -> LiveInfo {
    LiveInfo {
        local_url: format!("ws://127.0.0.1:{PORT}/live?token={token}"),
        share_url: format!("ws://{}:{PORT}/live?token={token}", lan_ip()),
    }
}

pub fn router(token: String) -> Router {
    let (tx, _) = broadcast::channel(512);
    let hub = Hub { tx, token, next: std::sync::Arc::new(AtomicU64::new(1)), people: std::sync::Arc::new(AtomicUsize::new(0)) };
    Router::new().route("/live", get(upgrade)).with_state(hub)
}

async fn upgrade(ws: WebSocketUpgrade, Query(q): Query<Join>, State(hub): State<Hub>) -> Response {
    if !q.token.as_deref().is_some_and(|t| same_secret(t, &hub.token)) {
        return (StatusCode::FORBIDDEN, "Wrong or missing session token").into_response();
    }
    if hub.people.load(Ordering::Relaxed) >= MAX_PEOPLE {
        return (StatusCode::SERVICE_UNAVAILABLE, "The session is full").into_response();
    }
    ws.max_message_size(MAX_MESSAGE).max_frame_size(MAX_MESSAGE).on_upgrade(move |socket| serve(socket, hub))
}

/// Counts a connection for as long as it lives.
struct Present(std::sync::Arc<AtomicUsize>);

impl Drop for Present {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

async fn serve(mut socket: WebSocket, hub: Hub) {
    let _present = {
        hub.people.fetch_add(1, Ordering::Relaxed);
        Present(hub.people.clone())
    };
    let me = hub.next.fetch_add(1, Ordering::Relaxed);
    let mut rx = hub.tx.subscribe();
    // Token bucket for this person's messages.
    let (mut allowance, mut last) = (BURST, Instant::now());
    let mut heard = Instant::now();
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    heard = Instant::now();
                    allowance = (allowance + heard.duration_since(last).as_secs_f64() * RATE).min(BURST);
                    last = heard;
                    if allowance >= 1.0 {
                        allowance -= 1.0;
                        let _ = hub.tx.send((me, text.to_string()));
                    }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => heard = Instant::now(),
            },
            _ = tokio::time::sleep_until((heard + SILENCE).into()) => break,
            outgoing = rx.recv() => match outgoing {
                Ok((from, text)) if from != me => {
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                // A slow client misses messages; the next document catches it up.
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }
}

/// Starts sharing (or returns the running session's links).
#[tauri::command]
pub async fn live_start() -> Result<LiveInfo, String> {
    if let Some(r) = RUNNING.lock().map_err(|e| e.to_string())?.as_ref() {
        return Ok(info(&r.token));
    }
    let token = token()?;
    let app = router(token.clone());
    // This window connects over loopback; the others over the local network address.
    let mut addresses = vec![std::net::IpAddr::from([127, 0, 0, 1])];
    if let Ok(lan) = lan_ip().parse::<std::net::IpAddr>()
        && !lan.is_loopback()
    {
        addresses.push(lan);
    }
    let mut tasks = Vec::new();
    for ip in addresses {
        let listener = match tokio::net::TcpListener::bind((ip, PORT)).await {
            Ok(l) => l,
            Err(e) => {
                tasks.iter().for_each(|t: &tokio::task::JoinHandle<()>| t.abort());
                return Err(format!("Could not open port {PORT} on {ip}: {e}"));
            }
        };
        let app = app.clone();
        tasks.push(tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        }));
    }
    *RUNNING.lock().map_err(|e| e.to_string())? = Some(Running { token: token.clone(), tasks });
    Ok(info(&token))
}

/// Stops sharing; everyone connected is disconnected.
#[tauri::command]
pub fn live_stop() {
    if let Ok(mut r) = RUNNING.lock()
        && let Some(r) = r.take()
    {
        r.tasks.iter().for_each(|t| t.abort());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    #[tokio::test]
    async fn relays_to_everyone_but_the_sender_and_checks_the_token() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router("secret".into())).await.unwrap() });
        let url = |t: &str| format!("ws://{addr}/live?token={t}");
        assert!(tokio_tungstenite::connect_async(url("wrong")).await.is_err(), "a wrong token is refused");
        let (mut a, _) = tokio_tungstenite::connect_async(url("secret")).await.unwrap();
        let (mut b, _) = tokio_tungstenite::connect_async(url("secret")).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        a.send(WsMessage::text("{\"type\":\"cursor\"}")).await.unwrap();
        let got = tokio::time::timeout(std::time::Duration::from_secs(2), b.next()).await.unwrap().unwrap().unwrap();
        assert_eq!(got.into_text().unwrap().as_str(), "{\"type\":\"cursor\"}");
        // The sender does not get its own message back.
        assert!(tokio::time::timeout(std::time::Duration::from_millis(200), a.next()).await.is_err());
    }

    #[test]
    fn tokens_are_random_and_compared_whole() {
        let (a, b) = (token().unwrap(), token().unwrap());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
        assert!(same_secret(&a, &a.clone()));
        assert!(!same_secret(&a, &b));
        assert!(!same_secret(&a, &a[..31]));
    }
}
