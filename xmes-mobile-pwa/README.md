<p align="center">
  <img src="../Xmes.svg" alt="xmes" width="80" height="80" />
</p>

<h2 align="center">xmes-pwa</h2>

<p align="center">
  Progressive Web App frontend for xmes — pure Dioxus, no JS interop.
</p>

---

The `xmes-pwa` crate is the UI layer of xmes. It is compiled to WebAssembly and runs as a Progressive Web App in the browser. All XMTP protocol operations and worker infrastructure are encapsulated in [`xmes-xmtp-wasm`](../xmes-xmtp-wasm) — this crate only contains Dioxus components and signal management.

### Dependencies

| Crate | Purpose |
|---|---|
| `dioxus` | UI framework (web renderer) |
| `dioxus-sdk` | `use_persistent` for localStorage |
| `dioxus-primitives` | UI primitives |
| `xmes-xmtp-wasm` | XMTP logic + worker infrastructure |

### Assets

```
assets/
├── icons/            # PWA icons (16–512 px, iOS + Android)
├── manifest.webmanifest
├── register-sw.js    # Service worker registration
├── styling/
│   └── main.css      # Design system (CSS custom properties)
└── tailwind.css      # Tailwind utility classes (auto-generated)

public/
└── sw.js             # Service worker (served at /sw.js)
```

### Running

```bash
dx serve --addr 0.0.0.0 --port 9000
```
