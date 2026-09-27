# Noobietoria

**Noobietoria** is an open-source, User-Generated Content (UGC) platform built with **Godot Engine 4.7 (C#)**. It empowers players and creators to build, share, and experience custom games and avatar items.

## Key Features
* **User-Generated Games:** Create and host custom experiences using our dedicated C# tools.
* **Avatar Customization:** Design and trade virtual clothing and cosmetics.
* **Powered by Godot 4.7:** Leveraging modern C# performance for client, studio, and server architecture.

## Project Layout

| Folder | What it is |
| --- | --- |
| `Client/` | The playable Godot client: main menu, connection flow, avatar movement with name tags, and chat. |
| `DedicatedServer/` | Headless authoritative relay: handshake validation, player roster, chat relay. |
| `Studio/` | UGC map editor: snap-to-grid block building with JSON save/load under `Studio/maps/`. |
| `Shared/` | Protocol constants (ports, RPC method names) compiled into every module. |
| `Client/tests/` | Headless end-to-end networking test (`E2E.tscn`). |

## Building & Running

Requirements: [.NET SDK 8+](https://dotnet.microsoft.com/download) and [Godot 4.7 (.NET edition)](https://godotengine.org/download).

```bash
# Build every module (no Godot install needed for this step)
dotnet build Noobietoria.sln
```

* **Dedicated server**
  ```bash
  godot --path DedicatedServer --headless -- --port 24565
  ```
* **Client** — open `Client/` in Godot 4.7 (or run `godot --path Client`), pick a name, and connect to `127.0.0.1`. WASD to move, Space to jump, Enter to chat.
* **Studio** — open `Studio/` (or run `godot --path Studio`). Left-click places a block, right-click removes one, right-drag rotates the camera. Save/load maps as JSON in `Studio/maps/` — try the bundled `sample-map`.

### End-to-end networking test

```bash
godot --path DedicatedServer --headless -- --port 24599 &
sleep 3
E2E_ROLE=a E2E_PORT=24599 godot --path Client --headless res://tests/E2E.tscn &   # Alpha
sleep 2
E2E_ROLE=b E2E_PORT=24599 godot --path Client --headless res://tests/E2E.tscn     # Beta, exits 0 on pass
```

## Architecture Notes

* RPC method names live in `Shared/Protocol.cs` as strings, so the three separate Godot assemblies stay in sync.
* Godot requires an `[Rpc]` configuration on **both** the sending and receiving nodes; protocol methods therefore exist on both sides (stub bodies where a side never executes them).
* The server mirrors the client node layout (`/root/Main/Players/Player-N`) so avatar state RPCs resolve on every peer.
* Movement sync is client-authoritative (v0): the server owns roster and chat; server-side movement validation can later hook into `DedicatedServer/scripts/PlayerRelay.cs`.

