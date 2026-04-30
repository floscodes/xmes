<p align="center">
  <img src="./Xmes.svg" alt="xmes" width="96" height="96" />
</p>

<h1 align="center">xmes</h1>

<p align="center">
  Decentralised, end-to-end encrypted messaging — built with Rust and XMTP.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-early%20development-orange?style=flat-square" alt="status" />
  <img src="https://img.shields.io/badge/rust-2024%20edition-b7410e?style=flat-square&logo=rust" alt="rust" />
  <img src="https://img.shields.io/badge/target-WebAssembly-6366f1?style=flat-square&logo=webassembly" alt="wasm" />
</p>

---

## What is xmes?

xmes is an open-source messenger built on the [XMTP](https://xmtp.org) protocol — a decentralised, blockchain-based messaging standard. Identities are Ethereum keypairs: no central server controls your account or your messages.

The project compiles a single Rust codebase to WebAssembly, targeting web (PWA), desktop, and mobile via [Dioxus](https://dioxus.dev).

---

## Repository structure

```
xmes/
├── xmes-xmtp-wasm/   # XMTP integration layer — WASM only, no UI
└── xmes-mobile-pwa/         # Dioxus Mobile PWA frontend — WebAssembly
```

### `xmes-xmtp-wasm`

The XMTP integration layer, compiled exclusively to WebAssembly. Wraps the `libxmtp` WASM bindings and exposes a clean Rust API. Responsibilities:

- Ethereum keypair generation and identity management
- Private key serialisation (hex) for local persistence
- Conversation listing and group creation via XMTP
- **Worker infrastructure** — spawns a Dedicated Worker so the XMTP SQLite database can use the OPFS Sync Access Handle VFS (browser main thread restriction workaround)
- Environment switching (Local / Dev / Production)

### `xmes-mobile-pwa`

The Progressive Web App frontend fpr mobile devices built with [Dioxus](https://dioxus.dev) 0.7, compiled to WebAssembly. A pure UI crate: no JS interop, no wasm-bindgen direct dependency — only Dioxus and `xmes-xmtp-wasm`.

### `landing-page`

The landing page of [Xmes](https://xmes.org).

---

## Getting started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started): `cargo install dioxus-cli`
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`

### Development

```bash
# Start the dev server (PWA) with hot reload
dx serve --addr 0.0.0.0 --port 9000

# Build for web
dx build

# Lint (automatically targets WASM via .cargo/config.toml)
cargo clippy

# Format
cargo fmt
```

---

## License

MIT — see [LICENSE](./LICENSE) for details.
