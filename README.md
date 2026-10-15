<div align="center">

<picture>
    <source media="(prefers-color-scheme: light)" srcset=".github/assets/logofullwhite.svg">
    <img alt="Noobietoria Monorepo Cover" src=".github/assets/logofullwhite.svg" width="600px">
</picture>

# Noobietoria Monorepo

*A modular Rust-based game platform and world simulation ecosystem.*

[![Issues](https://img.shields.io/github/issues-raw/noobietoria/noobietoria?color=cd7a7b&label=issues&style=for-the-badge)](https://github.com/noobietoria/noobietoria/issues)
[![Pull Requests](https://img.shields.io/github/issues-pr-raw/noobietoria/noobietoria?color=cd7a7b&label=PRs&style=for-the-badge)](https://github.com/noobietoria/noobietoria/pulls)
[![Contributors](https://img.shields.io/github/contributors/noobietoria/noobietoria?color=cd7a7b&label=contributors&style=for-the-badge)](https://github.com/noobietoria/noobietoria/graphs/contributors)
[![Lines of Code](https://img.shields.io/endpoint?url=https://loctopus.creeperkatze.dev/github/noobietoria/noobietoria/badge?style=flat&logoColor=white&color=cd7a7b&style=for-the-badge)](https://github.com/noobietoria/noobietoria)
[![Commit Activity](https://img.shields.io/github/commit-activity/m/noobietoria/noobietoria?color=cd7a7b&label=commits&style=for-the-badge)](https://github.com/noobietoria/noobietoria/commits)
[![Last Commit](https://img.shields.io/github/last-commit/noobietoria/noobietoria?color=cd7a7b&label=last%20commit&style=for-the-badge)](https://github.com/noobietoria/noobietoria/commits)

</div>

---

## 📖 Overview

**Noobietoria** is a modular Rust-based game platform and world simulation project. It is organized as a Cargo workspace and separates concerns across core types, world simulation, networking, rendering, scripting, API layers, and storage. 

The project is designed to support multiplayer experiences, gameplay scripting, and extensible world logic while maintaining a clean, layered architecture encompassing a game client, dedicated authoritative server, web frontend, and studio toolchain.

## 🏗 Project Structure & Architecture

```text
noobietoria/
├── .github/          # GitHub workflows, CI/CD, and asset files
├── _sigs/            # Game system specifications (economy, player, system, ui)
├── apps/             # End-user applications and services
│   ├── noobietoria-client     # Game client to browse & join published places
│   ├── noobietoria-dedicated  # Authoritative host server for multiplayer sessions
│   ├── noobietoria-studio     # Creator tooling & level editing suite
│   └── noobietoria-web        # Web frontend interface
├── crates/           # Reusable engine crates and shared libraries
│   ├── nb-types      # Core shared data structures and types
│   ├── nb-world      # World simulation, map data, and logic
│   ├── nb-store      # Data persistence layer
│   ├── nb-api        # API layer specification
│   ├── nb-script     # Scripting engine and runtime (Lua/Luau integration)
│   ├── nb-net        # Networking, communication protocols, and streams
│   ├── nb-render     # Graphics rendering engine (wgpu-based)
│   └── nb-http       # HTTP service and web API routing (axum)
└── spec/             # API signatures, JSON schemas, and documentation tools
```

## ⚙️ Tech Stack & Dependencies

* **Language:** Rust (Edition 2021, target `1.85+`)
* **Architecture:** Cargo Workspace (Monorepo)
* **Async Runtime:** `tokio`
* **Rendering:** `wgpu`
* **UI Framework:** `egui` / `eframe` (Desktop & Studio UI)
* **Networking & Web:** `axum`, `tower-http`
* **Scripting:** `mlua` (Lua/Luau runtime)
* **Utilities & Security:** `serde`, `serde_json`, `uuid`, `argon2`, `reqwest`, `tracing`

## 🚀 Getting Started

### Prerequisites

Ensure you have **Rust 1.85+** and Cargo installed on your system (managed via `rust-toolchain.toml`).

1. **Clone the repository:**
   ```bash
   git clone https://github.com/noobietoria/noobietoria.git
   cd noobietoria
   ```

2. **Build the workspace:**
   ```bash
   cargo build --workspace
   ```

### Running Applications

* **Run the Game Client:**
  ```bash
   cargo run -p noobietoria-client
   ```

* **Run the Dedicated Game Server:**
  ```bash
   cargo run -p noobietoria-dedicated
   ```

* **Run the Studio Tooling:**
  ```bash
   cargo run -p noobietoria-studio
   ```

## 🤝 Contributing

Contributions, issues, and feature requests are welcome! Feel free to check out the issues or submit a Pull Request.

## 📄 License

This project is open-source. Check the repository files for licensing information.