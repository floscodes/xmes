# XMES

XMES is an open-source messenger built on the [XMTP](https://xmtp.org) protocol — a decentralized, blockchain-based messaging standard. It targets multiple platforms from a single Rust codebase: web (PWA), desktop, and mobile.

> **Work in progress.** The project is in early development. APIs and features are subject to change.

---

## What is XMTP?

XMTP (Extensible Message Transport Protocol) is an open protocol for secure, decentralized messaging. Identities are Ethereum keypairs — no central server controls your account or your messages.

---

## Repository Structure

This is a Cargo workspace with two crates:

```
xmes/
├── xmes-xmtp-wasm/   # XMTP API wrapper — WASM target only (no UI)
└── xmes-pwa/         # Dioxus frontend — PWA target (WebAssembly)
```

### `xmes-xmtp-wasm`

The XMTP integration layer, compiled exclusively to WebAssembly. It wraps the `libxmtp` WASM bindings and exposes a Rust API for all protocol operations. Responsibilities:

- Ethereum keypair generation and identity management
- Serialization / deserialization of identities (TOML)
- Conversation listing and group creation via XMTP
- Environment switching (Local / Dev / Production)

### `xmes-pwa`

The Progressive Web App frontend built with [Dioxus](https://dioxus.dev) 0.7, compiled to WebAssembly. It uses `xmes-xmtp-wasm` for all protocol operations and `dioxus-sdk` for local storage persistence of identities.

---

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started): `cargo install dioxus-cli`
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`

### Development

```bash
# Start the dev server with hot reload (PWA)
dx serve --addr 0.0.0.0 --port 9000

# Build for web
dx build

# Lint (targets WASM automatically via .cargo/config.toml)
cargo clippy

# Format
cargo fmt

# Run tests
cargo test
```

---

## Roadmap

- [x] XMTP identity creation and persistence
- [x] List conversations
- [ ] Send and receive messages
- [ ] Group conversations
- [ ] Push notifications
- [ ] Desktop target (via Dioxus desktop renderer)
- [ ] Mobile target (via Dioxus mobile renderer)

---

## License

To be determined.
