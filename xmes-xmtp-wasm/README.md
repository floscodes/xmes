# xmes-xmtp-wasm

WebAssembly library that powers the XMTP protocol integration for Xmes. It wraps [libxmtp](https://github.com/xmtp/libxmtp) (the official Rust XMTP implementation) and exposes a browser-friendly API for identity management, group conversations, and messaging — all running in a Dedicated Worker to keep the UI thread unblocked.

---

## Architecture

The crate has two distinct layers:

```
Browser Main Thread
       │
       │  postMessage (typed JS objects)
       ▼
  XmtpHandle  ──────────────────────────────────────────────────────┐
       │                                                              │
       │  callbacks                                                   │
       │  on_identity_update  on_conversations  on_messages  on_group_members
       │                                                              │
       ▼                                                              │
  Dedicated Worker (Blob URL)                                        │
       │                                                              │
       ▼                                                              │
  worker_run() — async message loop  ◄──────────────────────────────┘
       │
       ▼
  Identity  (lib.rs)
       │
       ▼
  bindings_wasm  (libxmtp WASM bindings)
       │
       ▼
  XMTP Network (dev / production)
```

**`src/lib.rs`** — core types and async XMTP operations  
**`src/worker.rs`** — Dedicated Worker bootstrap, message protocol, `XmtpHandle`

---

## Key Types

### `Identity`

Wraps an XMTP client together with the associated Ethereum keypair.

```rust
pub struct Identity {
    pub client:   XmtpClient,
    pub address:  String,   // Ethereum address (0x…)
    pub inbox_id: String,   // XMTP inbox ID
    pub key:      SigningKey,
    pub mnemonic: Option<String>,  // BIP39 phrase, present if freshly generated
}
```

Creating and restoring:

```rust
// New random identity
let identity = Identity::new(env).await?;

// Restore from BIP39 mnemonic (12 words)
let identity = Identity::from_mnemonic("word1 word2 … word12", env).await?;

// Restore from raw private key hex
let identity = Identity::from_key_hex("0xdeadbeef…", Some(mnemonic), env).await?;
```

### `Env`

Selects the XMTP network:

```rust
pub enum Env {
    Local { host: String },
    Dev,          // https://api.dev.xmtp.network:5558
    Production,   // https://api.production.xmtp.network:5558
}
```

### `ConversationSummary`

Lightweight summary for the conversation list:

| Field | Type | Description |
|---|---|---|
| `id` | `String` | Unique conversation (group) ID |
| `name` | `String` | Group name |
| `last_msg` | `String` | Text of the last message |
| `last_sender_address` | `String` | Ethereum address of last sender |
| `last_sender_inbox_id` | `String` | Inbox ID of last sender |
| `pending` | `bool` | `true` if consent is still Unknown (invitation) |

### `MessageInfo`

A single message in a conversation:

| Field | Type | Description |
|---|---|---|
| `id` | `String` | Message ID |
| `text` | `String` | Message text |
| `system_text` | `Option<String>` | Non-`None` for join/leave events |
| `sender_inbox_id` | `String` | Inbox ID of the sender |
| `sent_at_ns` | `i64` | Send timestamp in nanoseconds |
| `delivered` | `bool` | `true` once `DeliveryStatus::Published` |

### `MemberInfo`

A member of a group conversation:

| Field | Type | Description |
|---|---|---|
| `inbox_id` | `String` | XMTP inbox ID |
| `address` | `String` | Resolved Ethereum address |
| `role` | `u8` | 0 = Member, 1 = Admin, 2 = SuperAdmin |

### `IdentityInfo`

Sent to the host thread on every identity list update:

| Field | Type | Description |
|---|---|---|
| `key_hex` | `String` | Serialized private key |
| `inbox_id` | `String` | XMTP inbox ID |
| `primary_address` | `String` | Primary Ethereum address |
| `addresses` | `Vec<String>` | All linked addresses |
| `mnemonic` | `Option<String>` | BIP39 phrase (only on first creation) |

---

## Worker API

Everything the host (main thread) needs goes through `spawn_xmtp_worker` and the returned `XmtpHandle`.

### Spawning the Worker

```rust
let handle = spawn_xmtp_worker(
    env,
    key_hexes,   // Vec<String> — serialized private keys from storage
    mnemonics,   // Vec<String> — corresponding mnemonics (empty string if none)
    on_identity_update,  // Fn(IdentityListUpdate)
    on_conversations,    // Fn(Vec<ConversationSummary>)
    on_messages,         // Fn(String, Vec<MessageInfo>)  — (conv_id, messages)
    on_group_members,    // Fn(Vec<MemberInfo>)
);
```

The worker is spawned from a Blob URL so no extra server-side file is needed. It patches `fetch()` internally to resolve origin-relative paths correctly from within the Blob context.

### `XmtpHandle` — Request Methods

All methods send a fire-and-forget message to the worker. Results arrive via the callbacks provided to `spawn_xmtp_worker`.

#### Identity

| Method | Callback triggered |
|---|---|
| `request_create_identity()` | `on_identity_update` |
| `request_restore_identity(phrase)` | `on_identity_update` |
| `request_remove_identity(idx)` | `on_identity_update` |
| `request_switch_identity(idx)` | `on_identity_update` |
| `request_add_address(idx)` | *(not yet implemented)* |
| `request_remove_address(idx, address)` | *(not yet implemented)* |

#### Conversations

| Method | Callback triggered |
|---|---|
| `request_list()` | `on_conversations` |
| `request_create_group()` | `on_conversations` |
| `request_leave(id)` | `on_conversations` |
| `request_accept_invitation(id)` | `on_conversations` |
| `request_decline_invitation(id)` | `on_conversations` |

#### Members

| Method | Callback triggered |
|---|---|
| `request_list_members(conversation_id)` | `on_group_members` |
| `request_add_members(conversation_id, inbox_ids)` | `on_group_members` |
| `request_remove_member(conversation_id, inbox_id)` | `on_group_members` |
| `request_set_admin(conversation_id, inbox_id, add)` | `on_group_members` |
| `request_set_super_admin(conversation_id, inbox_id, add)` | `on_group_members` |

#### Messages

| Method | Callback triggered |
|---|---|
| `request_list_messages(conversation_id)` | `on_messages` |
| `request_send_message(conversation_id, text)` | `on_messages` |
| `request_update_group_name(conversation_id, name)` | — |

---

## Message Protocol (Worker ↔ Host)

Messages are plain JS objects with a `type` field. You normally do not need to interact with the protocol directly — use `XmtpHandle` instead.

### Host → Worker

| `type` | Payload fields | Description |
|---|---|---|
| `init_dev_env` | `key_hexes`, `mnemonics` | Initialize with dev network |
| `init_production_env` | `key_hexes`, `mnemonics` | Initialize with production network |
| `init_local_env` | `key_hexes`, `mnemonics`, `host` | Initialize with local node |
| `create_identity` | — | Create a new random identity |
| `restore_identity` | `phrase` | Restore from BIP39 mnemonic |
| `remove_identity` | `idx` | Remove identity at index |
| `switch_identity` | `idx` | Set active identity |
| `list` | — | Fetch conversation list |
| `create_group` | — | Create new group |
| `leave` | `conversation_id` | Leave a group |
| `accept_invitation` | `conversation_id` | Accept pending group |
| `decline_invitation` | `conversation_id` | Decline pending group |
| `list_messages` | `conversation_id` | Fetch messages |
| `send_message` | `conversation_id`, `text` | Send a message |
| `list_members` | `conversation_id` | Fetch member list |
| `add_members` | `conversation_id`, `inbox_ids` | Add members |
| `remove_member` | `conversation_id`, `inbox_id` | Remove a member |
| `set_admin` | `conversation_id`, `inbox_id`, `add` | Toggle admin |
| `set_super_admin` | `conversation_id`, `inbox_id`, `add` | Toggle super-admin |
| `update_group_name` | `conversation_id`, `name` | Rename a group |

### Worker → Host

| `type` | Payload fields | Description |
|---|---|---|
| `worker_ready` | — | Worker initialized and ready |
| `identity_list` | `identities`, `active_index` | Identity list update |
| `conversations` | `conversations` | Conversation list |
| `messages` | `conversation_id`, `messages` | Message list for one conversation |
| `group_members` | `conversation_id`, `members` | Member list for one conversation |
| `error` | `msg` | Something went wrong |

---

## Identity Serialization

Identities are persisted in the host application as pairs of `(key_hex, mnemonic)` strings in browser local storage. `key_hex` is the raw k256 private key encoded as hex. On startup, pass the stored values to `spawn_xmtp_worker` — the worker calls `Identity::from_key_hex` for each pair and registers them with the XMTP network.

```rust
// Serialization
let hex  = identity.to_key_hex();
let mn   = identity.mnemonic();   // Option<String>

// Deserialization (happens inside the worker)
let identity = Identity::from_key_hex(&hex, mn.as_deref(), env).await?;
```

---

## Role System

Group roles map to `u8` values in `MemberInfo`:

| Value | Role | Permissions |
|---|---|---|
| 0 | Member | Read & send messages |
| 1 | Admin | + Add/remove members, rename group |
| 2 | SuperAdmin | + Promote/demote admins |

---

## Known Limitations

- **Address linking is not yet implemented.** `request_add_address` and `request_remove_address` are wired up end-to-end but return an error at the worker level because the required upstream API is not yet available in the current libxmtp WASM bindings.
- **BIP39 mnemonics are only returned once.** After the initial `create_identity` call, the mnemonic is no longer accessible from the `Identity` struct — the host must persist it immediately via `IdentityInfo.mnemonic`.
- **libxmtp is sourced directly from GitHub.** The `bindings_wasm` dependency points to `https://github.com/xmtp/libxmtp` and does not come from crates.io. A stable internet connection (or a pre-populated Cargo cache) is required to build.

---

## Building

The workspace-level `.cargo/config.toml` sets the default target to `wasm32-unknown-unknown`, so a plain `cargo build` inside the workspace already produces a WASM binary. Use [dioxus-cli (`dx`)](https://dioxuslabs.com/learn/0.6/getting_started) to build the full PWA:

```sh
# From the workspace root
dx build

# Development server with hot reload
dx serve --addr 0.0.0.0 --port 9000 --cross-origin-policy
```

---

## Dependencies

| Crate | Purpose |
|---|---|
| [`bindings_wasm`](https://github.com/xmtp/libxmtp) | Official libxmtp WASM bindings |
| [`alloy`](https://github.com/alloy-rs/alloy) | Ethereum signing and address management |
| [`k256`](https://github.com/RustCrypto/elliptic-curves) | secp256k1 key operations |
| [`bip39`](https://github.com/rust-bitcoin/rust-bip39) | Mnemonic phrase generation and derivation |
| [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen) | Rust ↔ JavaScript interop |
| [`wasm-bindgen-futures`](https://github.com/rustwasm/wasm-bindgen) | `async`/`await` in WASM |
| [`web-sys`](https://github.com/rustwasm/wasm-bindgen) | Browser Web APIs (Worker, Blob, fetch…) |
| [`js-sys`](https://github.com/rustwasm/wasm-bindgen) | JavaScript built-ins from Rust |
| [`anyhow`](https://github.com/dtolnay/anyhow) | Ergonomic error handling |
| [`hex`](https://github.com/KokaKiwi/rust-hex) | Key hex encoding/decoding |
| [`sha3`](https://github.com/RustCrypto/hashes) | Keccak-256 for address derivation |
| [`zstd`](https://github.com/gyscos/zstd-rs) | Compression (WASM feature) |

---

## License

MIT — Copyright (c) 2026 Florian Petautschnig. See [LICENSE](../LICENSE) for details.
