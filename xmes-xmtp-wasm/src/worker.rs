//! Worker infrastructure: runs on both sides of the Worker boundary.
//!
//! **Worker side** (`is_worker_context()` → true):
//!   Call `init_worker_mode()` from `main()` to start the XMTP handler loop.
//!
//! **Host side** (main browser thread / Dioxus):
//!   Call `spawn_xmtp_worker()` to create the worker and get an `XmtpHandle`.

use js_sys::Reflect;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{ConversationSummary, Env, Identity};

type IdentityRef = Rc<RefCell<Option<Identity>>>;

const XMTP_HOST: &str = "https://xmtp-dev.floscodes.net";

/// Minimal JS bootstrap loaded inside the Dedicated Worker.
/// Patches `fetch` so origin-relative paths resolve against the page
/// origin (Blob-URL workers have no origin of their own).
const WORKER_BOOTSTRAP: &str = r#"
self.addEventListener('message', async function(e) {
    if (e.data.type !== 'wasm_init') return;
    const { scriptUrl, pageOrigin } = e.data;
    const origFetch = self.fetch.bind(self);
    self.fetch = function(input, init) {
        if (typeof input === 'string' && input.startsWith('/')) {
            input = pageOrigin + input;
        }
        return origFetch(input, init);
    };
    await import(scriptUrl);
}, { once: true });
"#;

// ── worker-side ──────────────────────────────────────────────────────────────

/// Returns `true` when this WASM instance is running inside a Dedicated Worker.
pub fn is_worker_context() -> bool {
    js_sys::global()
        .dyn_ref::<web_sys::DedicatedWorkerGlobalScope>()
        .is_some()
}

/// Spawn the XMTP event loop. Call this from `main()` after
/// detecting a worker context with `is_worker_context()`.
pub fn init_worker_mode() {
    spawn_local(worker_run());
}

async fn worker_run() {
    let scope: web_sys::DedicatedWorkerGlobalScope =
        js_sys::global().dyn_into().unwrap_throw();

    let identity: IdentityRef = Rc::new(RefCell::new(None));

    let scope_cb = scope.clone();
    let identity_cb = identity.clone();

    let handler = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
        let data = e.data();
        let msg_type = str_field(&data, "type");
        let scope = scope_cb.clone();
        let identity = identity_cb.clone();

        match msg_type.as_str() {
            "init" => {
                let key_hex = opt_str_field(&data, "key_hex");
                spawn_local(handle_init(scope, identity, key_hex));
            }
            "list" => spawn_local(handle_list(scope, identity)),
            "create_group" => spawn_local(handle_create_group(scope, identity)),
            "leave" => {
                let id = str_field(&data, "id");
                spawn_local(handle_leave(scope, identity, id));
            }
            _ => {}
        }
    }) as Box<dyn Fn(web_sys::MessageEvent)>);

    scope.set_onmessage(Some(handler.as_ref().unchecked_ref()));
    handler.forget();

    let msg = typed_obj("worker_ready");
    scope.post_message(&msg).unwrap_throw();
}

async fn handle_init(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: IdentityRef,
    key_hex: Option<String>,
) {
    let identity = match key_hex {
        Some(hex) => match Identity::from_key_hex(hex, Env::Dev(Some(XMTP_HOST.to_string()))).await {
            Ok(id) => Some(id),
            Err(_) => new_identity().await,
        },
        None => new_identity().await,
    };

    match identity {
        Some(id) => {
            let key_hex = id.to_key_hex();
            *state.borrow_mut() = Some(id);
            let msg = typed_obj("ready");
            set_str(&msg, "key_hex", &key_hex);
            scope.post_message(&msg).unwrap_throw();
        }
        None => post_error(&scope, "Failed to initialize identity"),
    }
}

async fn handle_list(scope: web_sys::DedicatedWorkerGlobalScope, state: IdentityRef) {
    let id = state.borrow().clone();
    match id {
        Some(id) => match id.list_conversations().await {
            Ok(convos) => post_conversations(&scope, &convos),
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "Identity not initialized"),
    }
}

async fn handle_create_group(scope: web_sys::DedicatedWorkerGlobalScope, state: IdentityRef) {
    let id = state.borrow().clone();
    match id {
        Some(id) => match id.create_group().await {
            Ok(_) => handle_list(scope, state).await,
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "Identity not initialized"),
    }
}

async fn handle_leave(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: IdentityRef,
    conversation_id: String,
) {
    let id = state.borrow().clone();
    match id {
        Some(id) => match id.leave_conversation(conversation_id).await {
            Ok(_) => handle_list(scope, state).await,
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "Identity not initialized"),
    }
}

async fn new_identity() -> Option<Identity> {
    Identity::new(Env::Dev(Some(XMTP_HOST.to_string()))).await.ok()
}

// ── host-side ────────────────────────────────────────────────────────────────

/// A handle to the XMTP Dedicated Worker.
/// Methods send typed commands to the worker; results arrive via the
/// callbacks passed to `spawn_xmtp_worker`.
#[derive(Clone)]
pub struct XmtpHandle {
    worker: web_sys::Worker,
}

impl XmtpHandle {
    pub fn request_list(&self) {
        self.send("list");
    }

    pub fn request_create_group(&self) {
        self.send("create_group");
    }

    pub fn request_leave(&self, id: String) {
        let msg = typed_obj("leave");
        set_str(&msg, "id", &id);
        self.worker.post_message(&msg).unwrap_throw();
    }

    fn send(&self, msg_type: &str) {
        self.worker.post_message(&typed_obj(msg_type)).unwrap_throw();
    }
}

/// Spawn the XMTP Dedicated Worker and return a handle to it.
///
/// * `key_hex` – stored private key hex from localStorage, or `None`
///   to generate a fresh identity.
/// * `on_ready` – called with the (possibly new) key hex once the
///   identity is ready. Persist this value to localStorage.
/// * `on_conversations` – called whenever the worker sends a
///   conversation list.
pub fn spawn_xmtp_worker(
    key_hex: Option<String>,
    on_ready: impl Fn(String) + 'static,
    on_conversations: impl Fn(Vec<ConversationSummary>) + 'static,
) -> XmtpHandle {
    let arr = js_sys::Array::of1(&JsValue::from_str(WORKER_BOOTSTRAP));
    let mut props = web_sys::BlobPropertyBag::new();
    props.type_("application/javascript");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&arr, &props).unwrap_throw();
    let blob_url = web_sys::Url::create_object_url_with_blob(&blob).unwrap_throw();
    let worker = web_sys::Worker::new(&blob_url).unwrap_throw();
    web_sys::Url::revoke_object_url(&blob_url).unwrap_throw();

    let worker_cb = worker.clone();

    let onmessage = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
        let data = e.data();
        match str_field(&data, "type").as_str() {
            "worker_ready" => {
                let msg = typed_obj("init");
                match &key_hex {
                    Some(hex) => set_str(&msg, "key_hex", hex),
                    None => {
                        Reflect::set(&msg, &"key_hex".into(), &JsValue::null()).unwrap_throw();
                    }
                }
                worker_cb.post_message(&msg).unwrap_throw();
            }
            "ready" => on_ready(str_field(&data, "key_hex")),
            "conversations" => {
                let arr = Reflect::get(&data, &"data".into())
                    .ok()
                    .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
                    .unwrap_or_default();
                on_conversations(parse_conversations(&arr));
            }
            _ => {}
        }
    }) as Box<dyn Fn(web_sys::MessageEvent)>);

    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let script_url = find_script_url().unwrap_or_default();
    let page_origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    let msg = typed_obj("wasm_init");
    set_str(&msg, "scriptUrl", &script_url);
    set_str(&msg, "pageOrigin", &page_origin);
    worker.post_message(&msg).unwrap_throw();

    XmtpHandle { worker }
}

fn find_script_url() -> Option<String> {
    let document = web_sys::window()?.document()?;
    let nodes = document.query_selector_all("script[src]").ok()?;
    for i in 0..nodes.length() {
        let el = nodes.item(i)?.dyn_into::<web_sys::HtmlScriptElement>().ok()?;
        let src = el.src();
        if !src.is_empty() {
            return Some(src);
        }
    }
    None
}

fn parse_conversations(arr: &js_sys::Array) -> Vec<ConversationSummary> {
    (0..arr.length())
        .filter_map(|i| {
            let item = arr.get(i);
            let id = str_field(&item, "id");
            if id.is_empty() { return None; }
            let name = str_field(&item, "name");
            let last_sender = Reflect::get(&item, &"last_sender".into())
                .ok()
                .and_then(|v| v.as_string());
            Some(ConversationSummary { id, name, last_sender })
        })
        .collect()
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn typed_obj(msg_type: &str) -> js_sys::Object {
    let o = js_sys::Object::new();
    set_str(&o, "type", msg_type);
    o
}

fn set_str(obj: &js_sys::Object, key: &str, val: &str) {
    Reflect::set(obj, &key.into(), &val.into()).unwrap_throw();
}

fn str_field(val: &JsValue, key: &str) -> String {
    Reflect::get(val, &key.into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default()
}

fn opt_str_field(val: &JsValue, key: &str) -> Option<String> {
    let v = Reflect::get(val, &key.into()).ok()?;
    if v.is_null() || v.is_undefined() { None } else { v.as_string() }
}

fn post_conversations(scope: &web_sys::DedicatedWorkerGlobalScope, convos: &[ConversationSummary]) {
    let arr = js_sys::Array::new();
    for c in convos {
        let item = js_sys::Object::new();
        set_str(&item, "id", &c.id);
        set_str(&item, "name", &c.name);
        Reflect::set(
            &item,
            &"last_sender".into(),
            &c.last_sender.as_deref().map(JsValue::from_str).unwrap_or(JsValue::null()),
        ).unwrap_throw();
        arr.push(&item);
    }
    let msg = typed_obj("conversations");
    Reflect::set(&msg, &"data".into(), &arr).unwrap_throw();
    scope.post_message(&msg).unwrap_throw();
}

fn post_error(scope: &web_sys::DedicatedWorkerGlobalScope, msg: &str) {
    let o = typed_obj("error");
    set_str(&o, "msg", msg);
    scope.post_message(&o).unwrap_throw();
}
