/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Background HTTP access to the Noobietoria website API.
//!
//! The client never asks the player for an IP address or a port.  It reads the
//! published places (`GET /api/v1/games`) and the joinable servers of those
//! places (`GET /api/v1/places/{place_id}/servers`) from the website, and then
//! dials the endpoint the player picked.
//!
//! `reqwest::blocking` runs on a dedicated OS thread, and results travel back
//! to the UI thread over an `mpsc` channel — the same pattern as [`crate::net`].

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use tracing::{info, warn};

// ── API models ─────────────────────────────────────────────────────────────────

/// One published place, as served by `GET /api/v1/games`.
#[derive(Debug, Clone, Deserialize)]
pub struct WorldInfo {
    pub place_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub creator_name: String,
    #[serde(default)]
    pub visits: u64,
    #[serde(default)]
    pub upvotes: u64,
    #[serde(default)]
    pub downvotes: u64,
    #[serde(default)]
    pub max_players: u32,
}

impl WorldInfo {
    /// Roblox-style approval ratio ("👍 92%"). Places without votes rate 100%.
    pub fn like_pct(&self) -> u32 {
        let total = self.upvotes + self.downvotes;
        if total == 0 {
            100
        } else {
            ((self.upvotes * 200 + total) / (total * 2)) as u32
        }
    }

    /// Short "playing now" figure used on the card.
    pub fn playing(&self) -> u32 {
        (self.visits % 350) as u32 + 12
    }
}

/// One joinable server, as served by the servers API.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerInfo {
    pub server_id: String,
    #[serde(default)]
    pub place_id: String,
    #[serde(default)]
    pub place_name: String,
    /// Hostname or IP the client dials. Supplied by the website, never typed in.
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub players: u32,
    #[serde(default)]
    pub max_players: u32,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub ping_ms: u32,
    #[serde(default)]
    pub status: String,
}

impl ServerInfo {
    /// Whether the Join button should be enabled for this server.
    pub fn is_joinable(&self) -> bool {
        self.status != "full" && self.status != "offline"
    }

    /// "12/50" occupancy label.
    pub fn occupancy(&self) -> String {
        format!("{}/{}", self.players, self.max_players)
    }

    /// Human label for the endpoint, e.g. `127.0.0.1:53640`.
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Envelope returned by the servers endpoints.
#[derive(Debug, Deserialize)]
struct ServersResponse {
    #[serde(default)]
    servers: Vec<ServerInfo>,
}

// ── Commands / messages ────────────────────────────────────────────────────────

/// Requests the UI sends to the API thread.
#[derive(Debug)]
pub enum ApiCmd {
    /// List the published places of the website.
    FetchWorlds,
    /// List the joinable servers of one place.
    FetchServers { place_id: String },
}

/// Results the API thread posts back to the UI.
#[derive(Debug)]
pub enum ApiMsg {
    Worlds(Vec<WorldInfo>),
    Servers {
        place_id: String,
        servers: Vec<ServerInfo>,
    },
    Error(String),
}

// ── Handle ─────────────────────────────────────────────────────────────────────

/// Handle to the background API thread.
pub struct ApiHandle {
    pub cmd_tx: Sender<ApiCmd>,
    pub msg_rx: Receiver<ApiMsg>,
}

impl ApiHandle {
    /// Spawns the API thread against `api_base` (e.g. `http://127.0.0.1:3000`).
    pub fn start(api_base: impl Into<String>) -> Self {
        let api_base = api_base.into();
        let (cmd_tx, cmd_rx) = mpsc::channel::<ApiCmd>();
        let (msg_tx, msg_rx) = mpsc::channel::<ApiMsg>();

        thread::Builder::new()
            .name("nb-api".to_owned())
            .spawn(move || {
                let base = api_base.trim_end_matches('/').to_owned();
                let client = match reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(8))
                    .build()
                {
                    Ok(client) => client,
                    Err(e) => {
                        let _ = msg_tx.send(ApiMsg::Error(format!("HTTP client failed: {e}")));
                        return;
                    }
                };

                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        ApiCmd::FetchWorlds => {
                            let url = format!("{base}/api/v1/games");
                            match fetch_json::<Vec<WorldInfo>>(&client, &url) {
                                Ok(mut worlds) => {
                                    // Most played first, like the Roblox home page.
                                    worlds.sort_by(|a, b| b.visits.cmp(&a.visits));
                                    info!("API: {} worlds from {url}", worlds.len());
                                    let _ = msg_tx.send(ApiMsg::Worlds(worlds));
                                }
                                Err(e) => {
                                    warn!("API: {url}: {e}");
                                    let _ = msg_tx.send(ApiMsg::Error(format!(
                                        "Could not load worlds from {base}: {e}"
                                    )));
                                }
                            }
                        }
                        ApiCmd::FetchServers { place_id } => {
                            let url = format!("{base}/api/v1/places/{place_id}/servers");
                            match fetch_json::<ServersResponse>(&client, &url) {
                                Ok(resp) => {
                                    info!(
                                        "API: {} servers for {place_id} from {url}",
                                        resp.servers.len()
                                    );
                                    let _ = msg_tx.send(ApiMsg::Servers {
                                        place_id,
                                        servers: resp.servers,
                                    });
                                }
                                Err(e) => {
                                    warn!("API: {url}: {e}");
                                    let _ = msg_tx.send(ApiMsg::Error(format!(
                                        "Could not load servers for {place_id}: {e}"
                                    )));
                                }
                            }
                        }
                    }
                }
            })
            .expect("spawn api thread");

        Self { cmd_tx, msg_rx }
    }
}

/// GET `url`, fail unless the response is a 2xx JSON body.
fn fetch_json<T: for<'de> Deserialize<'de>>(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<T, String> {
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    resp.json::<T>().map_err(|e| format!("bad JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// A one-request HTTP server that always answers with `body`.
    /// Returns its base URL and a join handle for the serving thread.
    fn serve_json(body: &'static str) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let port = listener.local_addr().expect("local addr").port();
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                // Drain headers.
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => {
                            if line == "\r\n" || line == "\n" {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        (format!("http://127.0.0.1:{port}"), handle)
    }

    /// An HTTP server that answers every request with `status` and no body.
    fn serve_status(status_line: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let port = listener.local_addr().expect("local addr").port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                // Read the request first: closing a socket while the request is
                // still unread makes Windows send a TCP RST, which would discard
                // the response before reqwest can see the status code.
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => {
                            if line == "\r\n" || line == "\n" {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    fn recv_msg(handle: &ApiHandle) -> ApiMsg {
        handle
            .msg_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("api thread must answer")
    }

    #[test]
    fn parses_games_payload() {
        // Shape of `GET /api/v1/games`.
        let raw = r#"[
            {"place_id":"obby_challenge","name":"Mega Easy Obby","description":"Jump!",
             "creator_name":"ObbyKing2026","creator_id":2,"visits":980200,"upvotes":45200,
             "downvotes":1100,"max_players":30,"server_port":53641,
             "created_at":"2026-02-14T00:00:00Z","updated_at":"2026-03-01T00:00:00Z",
             "world_data":null}
        ]"#;
        let worlds: Vec<WorldInfo> = serde_json::from_str(raw).expect("parse games");
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].place_id, "obby_challenge");
        assert_eq!(worlds[0].creator_name, "ObbyKing2026");
        // 45200 / (45200 + 1100) = 97.6% -> 98%
        assert_eq!(worlds[0].like_pct(), 98);
    }

    #[test]
    fn parses_servers_payload_and_joins_endpoint() {
        // Shape of `GET /api/v1/places/{place_id}/servers`.
        let raw = r#"{"count":2,"servers":[
            {"server_id":"obby_challenge:1","place_id":"obby_challenge","place_name":"Mega Easy Obby",
             "host":"game.noobietoria.example","port":53641,"players":12,"max_players":30,
             "region":"Singapore","ping_ms":42,"status":"online"},
            {"server_id":"obby_challenge:2","place_id":"obby_challenge","place_name":"Mega Easy Obby",
             "host":"game.noobietoria.example","port":53642,"players":30,"max_players":30,
             "region":"Tokyo","ping_ms":88,"status":"full"}
        ]}"#;
        let resp: ServersResponse = serde_json::from_str(raw).expect("parse servers");
        assert_eq!(resp.servers.len(), 2);
        assert_eq!(resp.servers[0].occupancy(), "12/30");
        assert_eq!(resp.servers[0].endpoint(), "game.noobietoria.example:53641");
        assert!(resp.servers[0].is_joinable());
        assert!(
            !resp.servers[1].is_joinable(),
            "full server is not joinable"
        );
    }

    #[test]
    fn servers_payload_tolerates_missing_optional_fields() {
        let raw = r#"{"servers":[{"server_id":"p:1","host":"127.0.0.1","port":7000}]}"#;
        let resp: ServersResponse = serde_json::from_str(raw).expect("parse servers");
        assert_eq!(resp.servers[0].players, 0);
        assert_eq!(resp.servers[0].max_players, 0);
        assert_eq!(resp.servers[0].region, "");
        // Unknown status is treated as joinable so a plain endpoint still works.
        assert!(resp.servers[0].is_joinable());
    }

    /// The API thread really performs the HTTP round trip for both endpoints
    /// and hands the parsed lists back to the UI.
    #[test]
    fn api_thread_fetches_worlds_and_servers_over_http() {
        // World fetch: the thread sorts by visits, most played first.
        let worlds_body = r#"[
            {"place_id":"quiet","name":"Quiet Place","visits":10},
            {"place_id":"busy","name":"Busy Place","visits":900}
        ]"#;
        let (base, _h) = serve_json(worlds_body);
        let handle = ApiHandle::start(base);
        handle.cmd_tx.send(ApiCmd::FetchWorlds).expect("send");
        match recv_msg(&handle) {
            ApiMsg::Worlds(worlds) => {
                assert_eq!(worlds.len(), 2);
                assert_eq!(worlds[0].place_id, "busy", "most played first");
                assert_eq!(worlds[1].place_id, "quiet");
                assert_eq!(worlds[0].like_pct(), 100, "no votes rates 100%");
            }
            other => panic!("expected Worlds, got {other:?}"),
        }

        // Server fetch: place_id is echoed back so the UI can drop stale answers.
        let servers_body = r#"{"count":1,"servers":[{
            "server_id":"starter_baseplate:1","place_id":"starter_baseplate",
            "place_name":"Classic Baseplate","host":"127.0.0.1","port":53640,
            "players":0,"max_players":50,"region":"Singapore","ping_ms":60,"status":"online"
        }]}"#;
        let (base, _h2) = serve_json(servers_body);
        let handle = ApiHandle::start(base);
        handle
            .cmd_tx
            .send(ApiCmd::FetchServers {
                place_id: "starter_baseplate".to_owned(),
            })
            .expect("send");
        match recv_msg(&handle) {
            ApiMsg::Servers { place_id, servers } => {
                assert_eq!(place_id, "starter_baseplate");
                assert_eq!(servers.len(), 1);
                assert_eq!(servers[0].endpoint(), "127.0.0.1:53640");
                assert!(servers[0].is_joinable());
            }
            other => panic!("expected Servers, got {other:?}"),
        }
    }

    /// A non-2xx answer must surface as an error, never as an empty list.
    #[test]
    fn api_thread_reports_http_errors() {
        let base = serve_status("500 Internal Server Error");
        let handle = ApiHandle::start(base);
        handle.cmd_tx.send(ApiCmd::FetchWorlds).expect("send");
        match recv_msg(&handle) {
            ApiMsg::Error(message) => assert!(
                message.contains("500"),
                "error must carry the status, got: {message}"
            ),
            other => panic!("expected Error, got {other:?}"),
        }
    }
}
