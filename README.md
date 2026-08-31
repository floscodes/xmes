

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

The Progressive Web App frontend for mobile devices built with [Dioxus](https://dioxus.dev) 0.7, compiled to WebAssembly. A pure UI crate: no JS interop, no wasm-bindgen direct dependency — only Dioxus and `xmes-xmtp-wasm`.

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

---

## Legal Disclaimer

This software is provided "as is", without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose, and non-infringement. This software is licensed under the MIT License.

1. **Educational and Research Purpose:** Xmes is an open-source, decentralized messaging client application built utilizing the XMTP protocol ecosystem. It is developed and provided strictly for research, educational, and demonstration purposes.

2. **Independence from Infrastructure (XMTP Ecosystem):** Xmes functions solely as a client-side interface (frontend). The developer(s) of Xmes do not own, operate, control, or maintain the underlying XMTP network, its nodes, its smart contracts, or any associated communication routing infrastructure. The developer(s) have no affiliation with the operational management of the XMTP network. Xmes does not operate any central servers and does not collect, store, intercept, or process personal data, communication metadata, or cryptographic keys.

3. **Exclusive Client-Side Management and Data Storage:** All cryptographic keys, identity credentials, and message payloads are managed exclusively client-side by the end-user. Any user-generated data, including cryptographic identities and local address aliases/contact names, is stored strictly within the user's local device storage (e.g. browser localStorage). No such data is ever transmitted to, stored by, or accessible to the developer(s). As no personal data is processed by the developer(s), the developer(s) do not act as a data controller or data processor within the meaning of the General Data Protection Regulation (GDPR / EU 2016/679). Users bear sole responsibility for ensuring that their own deployment and utilization of this software complies with all applicable local, national, and international laws, including but not limited to the GDPR.

4. **No Financial Service:** This software does not constitute a financial service, payment service, cryptocurrency wallet, exchange, or investment product of any kind. Any interaction with blockchain-based infrastructure (such as Ethereum-compatible addresses used by the XMTP protocol) is solely for the purpose of decentralized identity and messaging. The developer(s) provide no financial advice and assume no liability for any financial loss.

5. **No Guarantee of Availability:** The availability and functionality of Xmes depends on third-party infrastructure, including but not limited to the XMTP network and its nodes, over which the developer(s) have no control. The developer(s) make no guarantee regarding the continuous availability, reliability, or integrity of such third-party services, and assume no liability for any disruption, data loss, or inaccessibility resulting therefrom.

6. **Limitation of Liability:** In no event shall the developer(s) or copyright holders be liable for any claim, damages, or other liability, whether in an action of contract, tort, or otherwise, arising from, out of, or in connection with the software, its deployment, or the use or other dealings in the software.
