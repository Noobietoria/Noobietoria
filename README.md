<p align="center">
  <img src="https://raw.githubusercontent.com/Noobietoria/Noobietoria/refs/heads/main/logofullwhite.svg" alt="Noobietoria Logo" width="400">
</p>

# Noobietoria

![Issues](https://img.shields.io/github/issues-raw/Noobietoria/Noobietoria?color=cd7a7b&label=issues&style=for-the-badge)
![Pull Requests](https://img.shields.io/github/issues-pr-raw/Noobietoria/Noobietoria?color=cd7a7b&label=PRs&style=for-the-badge)
![Contributors](https://img.shields.io/github/contributors/Noobietoria/Noobietoria?color=cd7a7b&label=contributors&style=for-the-badge)
![Lines of Code](https://img.shields.io/endpoint?url=https://loctopus.creeperkatze.dev/github/Noobietoria/Noobietoria/badge?style=flat&logoColor=white&color=cd7a7b&style=for-the-badge)
![Commit Activity](https://img.shields.io/github/commit-activity/m/Noobietoria/Noobietoria?color=cd7a7b&label=commits&style=for-the-badge)
![Last Commit](https://img.shields.io/github/last-commit/Noobietoria/Noobietoria?color=cd7a7b&label=last%20commit&style=for-the-badge)

**Noobietoria** is an open-source, User-Generated Content (UGC) platform built with **Godot Engine 4.7 (C#)**. It empowers players and creators to build, share, and experience custom games and avatar items.

## Key Features
* **User-Generated Games:** Create and host custom experiences using our dedicated C# tools.
* **Avatar Customization:** Design and trade virtual clothing and cosmetics.
* **Powered by Godot 4.7:** Leveraging modern C# performance for client, studio, and server architecture.

## Documentation & API Reference
Full documentation, Instance hierarchy, and API details are available at the **[Noobietoria Documentation Site](https://noobietoria.github.io/Docs/en/)**:

* **[Instance Reference](https://noobietoria.github.io/Docs/en/instance/):** Explore creatable engine objects and hierarchies.
* **[API Reference](https://noobietoria.github.io/Docs/en/api/):** Complete class and method definitions.

### Core Services
* **Players & Social:** `PlayerService`, `BanService`, `PartyService`, `MatchmakingService`, `ChatService`
* **Items & Economy:** `InventoryService`, `MarketplaceService`, `TradingService`, `CurrencyService`
* **World & Physics:** `InstanceService`, `PhysicsService`, `LightingService`, `CollisionService`, `TweenService`
* **Gameplay & AI:** `QuestService`, `LeaderboardService`, `PathfindingService`, `DialogueService`
* **Data & Networking:** `DataStoreService`, `NetworkService`, `HttpService`, `AnalyticsService`

## Project Layout

| Folder | What it is |
| --- | --- |
| `Client/` | The playable Godot client: main menu, connection flow, avatar movement with name tags, and chat. |
| `DedicatedServer/` | Headless authoritative relay (ENet transport + handshake + roster + chat relay), hosting the `ServerDedicatedServer` domain layer. |
| `Studio/` | UGC tooling: `Studio/Core` holds the `Instance`/`InstanceService` engine tree; `Studio/scenes` is a snap-to-grid block map editor with JSON save/load under `Studio/maps/`. |
| `Studio.Tests/` | xUnit tests for the Core sources (run in any standard .NET environment, no Godot needed). |
| `Shared/` | Networking protocol constants (ports, RPC method names) compiled into the Client and DedicatedServer. |
| `Client/tests/` | Headless end-to-end networking test (`E2E.tscn`). |

## Building & Running

Requirements: [.NET SDK 8+](https://dotnet.microsoft.com/download) and [Godot 4.7 (.NET edition)](https://godotengine.org/download).

```bash
# Build every module (no Godot install needed for this step)
dotnet build Noobietoria.sln

# Run the Core unit tests
dotnet test Studio.Tests/Studio.Tests.csproj
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

### Continuous integration

Every push and pull request runs `.github/workflows/ci.yml`:

1. **Strict .NET build** (ubuntu + windows) — warnings are errors, unit tests, vulnerable-package audit.
2. **Real headless Godot 4.7.1** (checksum-pinned) — resource import gated on errors, scene validation for every `.tscn` (missing/broken references), smoke runs of each main scene, and the two-client E2E over real ENet. Failure logs are uploaded as artifacts.

Run the same checks locally with `tools/validate_scenes.gd` inside any of the three projects:

```bash
godot --headless --path Client --script res://tools/validate_scenes.gd
```

## Architecture Notes

* `Studio/Core` is the engine-agnostic Instance tree; the dedicated server hosts an authoritative `InstanceService` through `ServerDedicatedServer` (domain layer, covered by `Studio.Tests`), while `DedicatedServer/scripts/ServerMain.cs` is the Godot ENet transport that feeds joins/leaves into it.
* RPC method names live in `Shared/Protocol.cs` as strings, so the separate Godot assemblies stay in sync.
* Godot requires an `[Rpc]` configuration on **both** the sending and receiving nodes; protocol methods therefore exist on both sides (stub bodies where a side never executes them).
* The server mirrors the client node layout (`/root/Main/Players/Player-N`) so avatar state RPCs resolve on every peer.
* Movement sync is client-authoritative (v0): the server owns roster and chat; server-side movement validation can later hook into `DedicatedServer/scripts/PlayerRelay.cs`.
