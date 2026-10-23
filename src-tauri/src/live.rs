//! Live collaboration over the local network: a WebSocket relay every Pergel in the session
//! (the host's own window included) connects to. Messages are JSON text — cursors, documents —
//! and the relay forwards each one to everybody but its sender. Off until the user shares; a
//! random token in the link keeps strangers on the network out. MCP stays on 127.0.0.1.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

pub const PORT: u16 = 3856;

#[derive(Clone)]
struct Hub {
    tx: broadcast::Sender<(u64, String)>,
    token: String,
    next: std::sync::Arc<AtomicU64>,
}

struct Running {
    token: String,
    task: tokio::task::JoinHandle<()>,
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

fn token() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let mut x = t.as_nanos() as u64 ^ 0x9e37_79b9_7f4a_7c15 ^ std::process::id() as u64;
    let mut out = String::new();
    for _ in 0..16 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push(char::from_digit((x % 36) as u32, 36).unwrap_or('0'));
    }
    out
}

fn info(token: &str) -> LiveInfo {
    LiveInfo {
        local_url: format!("ws://127.0.0.1:{PORT}/live?token={token}"),
        share_url: format!("ws://{}:{PORT}/live?token={token}", lan_ip()),
    }
}

pub fn router(token: String) -> Router {
    let (tx, _) = broadcast::channel(512);
    let hub = Hub { tx, token, next: std::sync::Arc::new(AtomicU64::new(1)) };
    Router::new().route("/live", get(upgrade)).with_state(hub)
}

async fn upgrade(ws: WebSocketUpgrade, Query(q): Query<Join>, State(hub): State<Hub>) -> Response {
    if q.token.as_deref() != Some(hub.token.as_str()) {
        return (StatusCode::FORBIDDEN, "Wrong or missing session token").into_response();
    }
    // Documents with images are large; 64 MB per message.
    ws.max_message_size(64 << 20).max_frame_size(64 << 20).on_upgrade(move |socket| serve(socket, hub))
}

async fn serve(mut socket: WebSocket, hub: Hub) {
    let me = hub.next.fetch_add(1, Ordering::Relaxed);
    let mut rx = hub.tx.subscribe();
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    let _ = hub.tx.send((me, text.to_string()));
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
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
    let token = token();
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", PORT)).await.map_err(|e| format!("Could not open port {PORT}: {e}"))?;
    let app = router(token.clone());
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    *RUNNING.lock().map_err(|e| e.to_string())? = Some(Running { token: token.clone(), task });
    Ok(info(&token))
}

/// Stops sharing; everyone connected is disconnected.
#[tauri::command]
pub fn live_stop() {
    if let Ok(mut r) = RUNNING.lock()
        && let Some(r) = r.take()
    {
        r.task.abort();
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
}
