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

/// Per-identity metadata sent to the host thread.
#[derive(Clone, PartialEq)]
pub struct IdentityInfo {
    pub key_hex:         String,
    pub inbox_id:        String,
    /// The address derived from this identity's own signing key (cannot be removed).
    pub primary_address: String,
    /// All Ethereum addresses linked to this inbox (fetched from the network).
    pub addresses:       Vec<String>,
}

/// Sent whenever the identity list or active selection changes.
#[derive(Clone)]
pub struct IdentityListUpdate {
    pub identities:  Vec<IdentityInfo>,
    pub active_idx:  usize,
}

// ── Worker-side state ─────────────────────────────────────────────────────────

struct WorkerState {
    identities: Vec<Identity>,
    active: usize,
}

impl WorkerState {
    fn active(&self) -> Option<&Identity> {
        self.identities.get(self.active)
    }
    fn active_clone(&self) -> Option<Identity> {
        self.identities.get(self.active).cloned()
    }
}

type StateRef = Rc<RefCell<WorkerState>>;

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

// ── worker-side ───────────────────────────────────────────────────────────────

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

    let state: StateRef = Rc::new(RefCell::new(WorkerState {
        identities: vec![],
        active: 0,
    }));

    let scope_cb  = scope.clone();
    let state_cb  = state.clone();

    let handler = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
        let data     = e.data();
        let msg_type = str_field(&data, "type");
        let scope    = scope_cb.clone();
        let state    = state_cb.clone();

        match msg_type.as_str() {
            "init" => {
                let arr = Reflect::get(&data, &"key_hexes".into())
                    .ok()
                    .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
                    .unwrap_or_default();
                let key_hexes: Vec<String> = (0..arr.length())
                    .filter_map(|i| arr.get(i).as_string())
                    .collect();
                spawn_local(handle_init(scope, state, key_hexes));
            }
            "create_identity" => spawn_local(handle_create_identity(scope, state)),
            "remove_identity" => {
                let idx = Reflect::get(&data, &"index".into())
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as usize;
                spawn_local(handle_remove_identity(scope, state, idx));
            }
            "add_address" => {
                let idx = Reflect::get(&data, &"index".into())
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as usize;
                spawn_local(handle_add_address(scope, state, idx));
            }
            "remove_address" => {
                let idx = Reflect::get(&data, &"index".into())
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as usize;
                let address = str_field(&data, "address");
                spawn_local(handle_remove_address(scope, state, idx, address));
            }
            "switch_identity" => {
                let idx = Reflect::get(&data, &"index".into())
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as usize;
                spawn_local(handle_switch_identity(scope, state, idx));
            }
            "list"         => spawn_local(handle_list(scope, state)),
            "create_group" => spawn_local(handle_create_group(scope, state)),
            "leave" => {
                let id = str_field(&data, "id");
                spawn_local(handle_leave(scope, state, id));
            }
            _ => {}
        }
    }) as Box<dyn Fn(web_sys::MessageEvent)>);

    scope.set_onmessage(Some(handler.as_ref().unchecked_ref()));
    handler.forget();

    let msg = typed_obj("worker_ready");
    scope.post_message(&msg).unwrap_throw();
}

// ── message handlers ──────────────────────────────────────────────────────────

async fn handle_init(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    key_hexes: Vec<String>,
) {
    let env = Env::Dev(Some(XMTP_HOST.to_string()));

    let mut identities: Vec<Identity> = Vec::new();
    for hex in key_hexes {
        match Identity::from_key_hex(hex, env.clone()).await {
            Ok(id)  => identities.push(id),
            Err(_)  => {} // skip corrupt keys
        }
    }

    // Always have at least one identity
    if identities.is_empty() {
        if let Some(id) = new_identity().await {
            identities.push(id);
        }
    }

    state.borrow_mut().identities = identities;
    state.borrow_mut().active     = 0;

    post_identity_list_async(&scope, &state).await;
}

async fn handle_create_identity(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
) {
    if let Some(id) = new_identity().await {
        let new_idx = {
            let mut s = state.borrow_mut();
            s.identities.push(id);
            s.identities.len() - 1
        };
        state.borrow_mut().active = new_idx;
        post_identity_list_async(&scope, &state).await;
        handle_list(scope, state).await;
    } else {
        post_error(&scope, "Failed to create new identity");
    }
}

async fn handle_remove_identity(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    idx: usize,
) {
    {
        let mut s = state.borrow_mut();
        if idx >= s.identities.len() { return; }
        s.identities.remove(idx);
        if s.identities.is_empty() {
            s.active = 0;
        } else if s.active >= s.identities.len() {
            s.active = s.identities.len() - 1;
        } else if idx < s.active {
            s.active -= 1;
        }
    }
    // Always keep at least one identity.
    if state.borrow().identities.is_empty() {
        if let Some(id) = new_identity().await {
            state.borrow_mut().identities.push(id);
        }
    }
    post_identity_list_async(&scope, &state).await;
    handle_list(scope, state).await;
}

async fn handle_remove_address(
    scope: web_sys::DedicatedWorkerGlobalScope,
    _state: StateRef,
    _idx: usize,
    _address: String,
) {
    post_error(&scope, "Address removal not yet supported — requires upstream libxmtp API change");
}

async fn handle_add_address(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    idx: usize,
) {
    let id = state.borrow().identities.get(idx).cloned();
    match id {
        Some(id) => match id.link_new_address().await {
            Ok(_new_key_hex) => {
                // Refresh the identity list so the new address shows up.
                post_identity_list_async(&scope, &state).await;
            }
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "Identity not found"),
    }
}

async fn handle_switch_identity(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    idx: usize,
) {
    {
        let mut s = state.borrow_mut();
        if idx < s.identities.len() {
            s.active = idx;
        }
    }
    post_identity_list_async(&scope, &state).await;
    handle_list(scope, state).await;
}

async fn handle_list(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.list_conversations().await {
            Ok(convos) => post_conversations(&scope, &convos),
            Err(e)     => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn handle_create_group(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.create_group().await {
            Ok(_)  => handle_list(scope, state).await,
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn handle_leave(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.leave_conversation(conversation_id).await {
            Ok(_)  => handle_list(scope, state).await,
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn new_identity() -> Option<Identity> {
    Identity::new(Env::Dev(Some(XMTP_HOST.to_string()))).await.ok()
}

// ── host-side ─────────────────────────────────────────────────────────────────

/// A handle to the XMTP Dedicated Worker.
#[derive(Clone)]
pub struct XmtpHandle {
    worker: web_sys::Worker,
}

impl XmtpHandle {
    pub fn request_list(&self)                        { self.send("list"); }
    pub fn request_create_group(&self)                { self.send("create_group"); }
    pub fn request_create_identity(&self) { self.send("create_identity"); }

    pub fn request_remove_identity(&self, idx: usize) {
        let msg = typed_obj("remove_identity");
        Reflect::set(&msg, &"index".into(), &JsValue::from_f64(idx as f64)).unwrap_throw();
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_add_address(&self, identity_idx: usize) {
        let msg = typed_obj("add_address");
        Reflect::set(&msg, &"index".into(), &JsValue::from_f64(identity_idx as f64)).unwrap_throw();
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_remove_address(&self, identity_idx: usize, address: &str) {
        let msg = typed_obj("remove_address");
        Reflect::set(&msg, &"index".into(), &JsValue::from_f64(identity_idx as f64)).unwrap_throw();
        set_str(&msg, "address", address);
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_switch_identity(&self, idx: usize) {
        let msg = typed_obj("switch_identity");
        Reflect::set(&msg, &"index".into(), &JsValue::from_f64(idx as f64)).unwrap_throw();
        self.worker.post_message(&msg).unwrap_throw();
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
/// * `key_hexes` – stored private keys (from localStorage), empty for first run.
/// * `on_identity_update` – called whenever the identity list or active changes.
/// * `on_conversations` – called whenever a conversation list arrives.
pub fn spawn_xmtp_worker(
    key_hexes: Vec<String>,
    on_identity_update: impl Fn(IdentityListUpdate) + 'static,
    on_conversations:   impl Fn(Vec<ConversationSummary>) + 'static,
) -> XmtpHandle {
    let arr = js_sys::Array::of1(&JsValue::from_str(WORKER_BOOTSTRAP));
    let mut props = web_sys::BlobPropertyBag::new();
    props.type_("application/javascript");
    let blob      = web_sys::Blob::new_with_str_sequence_and_options(&arr, &props).unwrap_throw();
    let blob_url  = web_sys::Url::create_object_url_with_blob(&blob).unwrap_throw();
    let worker    = web_sys::Worker::new(&blob_url).unwrap_throw();
    web_sys::Url::revoke_object_url(&blob_url).unwrap_throw();

    let worker_cb = worker.clone();

    let onmessage = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
        let data = e.data();
        match str_field(&data, "type").as_str() {
            "worker_ready" => {
                // Send all stored keys to the worker for initialisation.
                let msg = typed_obj("init");
                let arr = js_sys::Array::new();
                for hex in &key_hexes {
                    arr.push(&JsValue::from_str(hex));
                }
                Reflect::set(&msg, &"key_hexes".into(), &arr).unwrap_throw();
                worker_cb.post_message(&msg).unwrap_throw();
            }
            "identity_list" => {
                let raw = Reflect::get(&data, &"identities".into())
                    .ok()
                    .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
                    .unwrap_or_default();
                let active_idx = Reflect::get(&data, &"active_idx".into())
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as usize;

                let identities: Vec<IdentityInfo> = (0..raw.length())
                    .map(|i| {
                        let item = raw.get(i);
                        let addr_arr = Reflect::get(&item, &"addresses".into())
                            .ok()
                            .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
                            .unwrap_or_default();
                        let addresses: Vec<String> = (0..addr_arr.length())
                            .filter_map(|j| addr_arr.get(j).as_string())
                            .collect();
                        IdentityInfo {
                            key_hex:         str_field(&item, "key_hex"),
                            inbox_id:        str_field(&item, "inbox_id"),
                            primary_address: str_field(&item, "primary_address"),
                            addresses,
                        }
                    })
                    .collect();

                on_identity_update(IdentityListUpdate { identities, active_idx });
            }
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

    let script_url  = find_script_url().unwrap_or_default();
    let page_origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    let msg = typed_obj("wasm_init");
    set_str(&msg, "scriptUrl",  &script_url);
    set_str(&msg, "pageOrigin", &page_origin);
    worker.post_message(&msg).unwrap_throw();

    XmtpHandle { worker }
}

fn find_script_url() -> Option<String> {
    let document = web_sys::window()?.document()?;
    let nodes    = document.query_selector_all("script[src]").ok()?;
    for i in 0..nodes.length() {
        let el = nodes.item(i)?.dyn_into::<web_sys::HtmlScriptElement>().ok()?;
        let src = el.src();
        if !src.is_empty() { return Some(src); }
    }
    None
}

fn parse_conversations(arr: &js_sys::Array) -> Vec<ConversationSummary> {
    (0..arr.length())
        .filter_map(|i| {
            let item = arr.get(i);
            let id   = str_field(&item, "id");
            if id.is_empty() { return None; }
            Some(ConversationSummary {
                id,
                name:        str_field(&item, "name"),
                last_sender: Reflect::get(&item, &"last_sender".into())
                    .ok()
                    .and_then(|v| v.as_string()),
            })
        })
        .collect()
}

// ── serialisation helpers ─────────────────────────────────────────────────────

/// Async version — fetches linked addresses from the network for each identity.
async fn post_identity_list_async(
    scope: &web_sys::DedicatedWorkerGlobalScope,
    state: &StateRef,
) {
    let arr        = js_sys::Array::new();
    let active_idx = state.borrow().active;
    let count      = state.borrow().identities.len();

    for i in 0..count {
        let id = state.borrow().identities[i].clone();
        let addresses = id.linked_addresses().await;

        let item = js_sys::Object::new();
        set_str(&item, "key_hex",         &id.to_key_hex());
        set_str(&item, "inbox_id",        &id.inbox_id());
        set_str(&item, "primary_address", &id.address());

        let addr_arr = js_sys::Array::new();
        for a in &addresses {
            addr_arr.push(&JsValue::from_str(a));
        }
        Reflect::set(&item, &"addresses".into(), &addr_arr).unwrap_throw();
        arr.push(&item);
    }

    let msg = typed_obj("identity_list");
    Reflect::set(&msg, &"identities".into(), &arr).unwrap_throw();
    Reflect::set(&msg, &"active_idx".into(), &JsValue::from_f64(active_idx as f64)).unwrap_throw();
    scope.post_message(&msg).unwrap_throw();
}

fn post_conversations(scope: &web_sys::DedicatedWorkerGlobalScope, convos: &[ConversationSummary]) {
    let arr = js_sys::Array::new();
    for c in convos {
        let item = js_sys::Object::new();
        set_str(&item, "id",   &c.id);
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
