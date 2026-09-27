# Noobietoria Platform Backend (closed source)

This folder defines the **contract** between the open-source game runtime
(Client, DedicatedServer, Studio) and the **Noobietoria platform backend**.
The backend itself is **closed source** and lives outside this repository.

## What the backend is

A Roblox-style platform control plane: centralized accounts, the game
catalog, place/session orchestration and matchmaking routing. Game servers
(the VPS fleet) and clients are open source; they only talk to the backend
through the contract in [`openapi.yaml`](./openapi.yaml).

## Cloudflare stack mapping

| Concern | Service | Notes |
| --- | --- | --- |
| API edge (accounts, catalog, join routing) | **Workers** | Stateless HTTP API defined by `openapi.yaml` |
| Accounts (username/password, tokens) | **D1** (SQLite) | Password hashing server-side only |
| Game catalog + place registry | **D1** | Places register/heartbeat from the VPS fleet |
| Session tickets (short-lived join grants) | **KV** | HMAC fleet secrets distributed to game servers |
| Avatars, thumbnails, game assets | **R2** | Served through the CDN |
| Game servers ("máy chủ con") | **VPS fleet** | Runs the exported `NoobietoriaServer` binaries; 1 process hosts many places |

## Join flow

1. Client logs in → `POST /v1/accounts/login` → account token.
2. Client lists games → `GET /v1/games`.
3. Client presses Play → `POST /v1/games/{gameId}/join` → the platform
   assigns a place from the fleet and returns `{ host, port, ticket, worldId }`.
4. Client connects to `host:port` and presents the ticket in the
   multiplayer handshake.
5. The place verifies the ticket **offline** (HMAC-SHA256, see
   `Shared/Platform/JoinTicket.cs`) using the fleet secret it received from
   the platform — no backend call on the hot path.

## Closed-source boundary

Everything in this repository is open. The backend implementation
(Workers code, D1 schemas, fleet provisioning) is closed and must satisfy
`openapi.yaml` exactly. The open repo ships an **offline dev mode**
(`PlatformClient` with no base URL + the dev ticket secret) so the client
and server remain fully testable without the backend.
