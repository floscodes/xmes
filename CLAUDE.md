# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

XMES is a decentralized messaging PWA built with Rust/Dioxus targeting WebAssembly. It uses the XMTP protocol for blockchain-based identity creation and decentralized messaging.

## Commands

```bash
# Start development server with hot reload
dx serve --addr 0.0.0.0 --port 9000

# Build for web
dx build

# Lint
cargo clippy

# Format
cargo fmt

# Run tests
cargo test
```

The `.cargo/config.toml` sets `wasm32-unknown-unknown` as the default build target, so `cargo build` / `cargo clippy` automatically target WASM.

## Architecture

The workspace has two crates:

**`xmes-xmtp-wasm`** — XMTP integration library, WASM target only, no UI. Contains:
- `Identity` struct wrapping an XMTP client with its Ethereum-based keypair
- `Env` enum for switching between XMTP environments (Local / Dev / Production)
- Identity serialization/deserialization via TOML (for local persistence)
- `list_conversations()` and `create_group()` as the primary XMTP operations

**`xmes-pwa`** — Dioxus 0.7 frontend PWA. Contains:
- `main.rs`: entry point — loads or creates an `Identity` via `dioxus-sdk` local storage, provides it to the component tree via context
- `components/conversations/`: UI for listing and displaying conversations

The flow is: `App` initializes identity → provides it as context → `Conversations` component calls `xmes_xmtp_wasm` to fetch data → renders with Dioxus reactivity.

## Dioxus 0.7 Patterns

This project uses Dioxus 0.7. Key API notes (old APIs are **gone**):
- No `cx`, `Scope`, or `use_state` — use `use_signal` instead
- State: `use_signal`, `use_memo`, `use_resource` (async)
- Context: `use_context_provider` / `use_context::<Signal<T>>()`
- Components: `#[component]` macro, props must be owned (`String` not `&str`), must impl `PartialEq + Clone`
- RSX: prefer `for` loops over iterator chains directly in RSX; expressions wrapped in `{}`
- Assets: `asset!("/assets/...")` macro; stylesheets via `document::Stylesheet`

See `xmes-pwa/AGENTS.md` for full Dioxus 0.7 reference with examples.

## XMTP Identity

Identities are Ethereum keypairs generated via `alloy` + `k256`. Each identity has an inbox ID derived from the Ethereum address. Identities are persisted as TOML strings in browser local storage (via `dioxus-sdk`). The `xmes-xmtp-wasm` crate uses `bindings_wasm` (libxmtp WASM bindings sourced from GitHub) for all XMTP protocol operations.
