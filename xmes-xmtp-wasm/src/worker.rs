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

use crate::{ConversationSummary, Env, Identity, MemberInfo, MessageInfo};

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
    env: Env,
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
        env: Env::Dev(None),
    }));

    let scope_cb  = scope.clone();
    let state_cb  = state.clone();

    let handler = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
        let data     = e.data();
        let msg_type = str_field(&data, "type");
        let scope    = scope_cb.clone();
        let state    = state_cb.clone();

        match msg_type.as_str() {
            "init_dev_env" | "init_production_env" | "init_local_env" => {
                let arr = Reflect::get(&data, &"key_hexes".into())
                    .ok()
                    .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
                    .unwrap_or_default();
                let key_hexes: Vec<String> = (0..arr.length())
                    .filter_map(|i| arr.get(i).as_string())
                    .collect();
                let env = match msg_type.as_str() {
                    "init_production_env" => Env::Production(None),
                    "init_local_env"      => {
                        let host = str_field(&data, "host");
                        Env::Local(host)
                    }
                    _                     => Env::Dev(None),
                };
                spawn_local(handle_init(env, scope, state, key_hexes));
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
            "list_members" => {
                let conversation_id = str_field(&data, "conversation_id");
                spawn_local(handle_list_members(scope, state, conversation_id));
            }
            "list_messages" => {
                let conversation_id = str_field(&data, "conversation_id");
                spawn_local(handle_list_messages(scope, state, conversation_id));
            }
            "send_message" => {
                let conversation_id = str_field(&data, "conversation_id");
                let text            = str_field(&data, "text");
                spawn_local(handle_send_message(scope, state, conversation_id, text));
            }
            "add_members" => {
                let conversation_id = str_field(&data, "conversation_id");
                let raw = Reflect::get(&data, &"inbox_ids".into())
                    .ok()
                    .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
                    .unwrap_or_default();
                let inbox_ids: Vec<String> = (0..raw.length())
                    .filter_map(|i| raw.get(i).as_string())
                    .collect();
                spawn_local(handle_add_members(scope, state, conversation_id, inbox_ids));
            }
            "remove_member" => {
                let conversation_id = str_field(&data, "conversation_id");
                let inbox_id        = str_field(&data, "inbox_id");
                spawn_local(handle_remove_member(scope, state, conversation_id, inbox_id));
            }
            "set_admin" => {
                let conversation_id = str_field(&data, "conversation_id");
                let inbox_id        = str_field(&data, "inbox_id");
                let add = Reflect::get(&data, &"add".into()).ok().and_then(|v| v.as_bool()).unwrap_or(false);
                spawn_local(handle_set_admin(scope, state, conversation_id, inbox_id, add));
            }
            "set_super_admin" => {
                let conversation_id = str_field(&data, "conversation_id");
                let inbox_id        = str_field(&data, "inbox_id");
                let add = Reflect::get(&data, &"add".into()).ok().and_then(|v| v.as_bool()).unwrap_or(false);
                spawn_local(handle_set_super_admin(scope, state, conversation_id, inbox_id, add));
            }
            "update_group_name" => {
                let conversation_id = str_field(&data, "conversation_id");
                let name            = str_field(&data, "name");
                spawn_local(handle_update_group_name(scope, state, conversation_id, name));
            }
            "accept_invitation" => {
                let id = str_field(&data, "id");
                spawn_local(handle_accept_invitation(scope, state, id));
            }
            "decline_invitation" => {
                let id = str_field(&data, "id");
                spawn_local(handle_decline_invitation(scope, state, id));
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
    env: Env,
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    key_hexes: Vec<String>,
) {

    let mut identities: Vec<Identity> = Vec::new();
    for hex in key_hexes {
        match Identity::from_key_hex(hex, env.clone()).await {
            Ok(id)  => identities.push(id),
            Err(_)  => {} // skip corrupt keys
        }
    }

    // Always have at least one identity
    if identities.is_empty() {
        if let Some(id) = new_identity(env.clone()).await {
            identities.push(id);
        }
    }

    state.borrow_mut().env        = env;
    state.borrow_mut().identities = identities;
    state.borrow_mut().active     = 0;

    post_identity_list_async(&scope, &state).await;
}

async fn handle_create_identity(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
) {
    let env = state.borrow().env.clone();
    if let Some(id) = new_identity(env).await {
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
        let env = state.borrow().env.clone();
        if let Some(id) = new_identity(env).await {
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

async fn handle_accept_invitation(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.accept_invitation(conversation_id) {
            Ok(_)  => handle_list(scope, state).await,
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn handle_decline_invitation(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.decline_invitation(conversation_id) {
            Ok(_)  => handle_list(scope, state).await,
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
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

async fn handle_add_members(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
    inbox_ids: Vec<String>,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.add_members_to_conversation(conversation_id, inbox_ids).await {
            Ok(_)  => handle_list(scope, state).await,
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn handle_remove_member(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
    inbox_id: String,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.remove_member(conversation_id.clone(), inbox_id).await {
            Ok(_)  => {
                let id2 = state.borrow().active_clone();
                if let Some(id2) = id2 {
                    if let Ok(m) = id2.get_conversation_members(conversation_id.clone()).await {
                        post_group_members(&scope, &conversation_id, &m);
                    }
                }
            }
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn handle_set_admin(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
    inbox_id: String,
    add: bool,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.set_admin(conversation_id.clone(), inbox_id, add).await {
            Ok(_)  => {
                let id2 = state.borrow().active_clone();
                if let Some(id2) = id2 {
                    if let Ok(m) = id2.get_conversation_members(conversation_id.clone()).await {
                        post_group_members(&scope, &conversation_id, &m);
                    }
                }
            }
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn handle_set_super_admin(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
    inbox_id: String,
    add: bool,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.set_super_admin(conversation_id.clone(), inbox_id, add).await {
            Ok(_)  => {
                let id2 = state.borrow().active_clone();
                if let Some(id2) = id2 {
                    if let Ok(m) = id2.get_conversation_members(conversation_id.clone()).await {
                        post_group_members(&scope, &conversation_id, &m);
                    }
                }
            }
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn handle_update_group_name(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
    name: String,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.update_group_name(conversation_id, name).await {
            Ok(_)  => handle_list(scope, state).await,
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn handle_list_members(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.get_conversation_members(conversation_id.clone()).await {
            Ok(addrs) => post_group_members(&scope, &conversation_id, &addrs),
            Err(e)    => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn handle_list_messages(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => match id.fetch_messages(conversation_id.clone()).await {
            Ok(msgs)  => post_messages(&scope, &conversation_id, &msgs),
            Err(e)    => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "No identity available"),
    }
}

async fn handle_send_message(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: StateRef,
    conversation_id: String,
    text: String,
) {
    let id = state.borrow().active_clone();
    match id {
        Some(id) => {
            if let Err(e) = id.send_text_message(conversation_id.clone(), text).await {
                post_error(&scope, &e.to_string());
                return;
            }
            match id.fetch_messages(conversation_id.clone()).await {
                Ok(msgs)  => post_messages(&scope, &conversation_id, &msgs),
                Err(e)    => post_error(&scope, &e.to_string()),
            }
        }
        None => post_error(&scope, "No identity available"),
    }
}

async fn new_identity(env: Env) -> Option<Identity> {
    Identity::new(env).await.ok()
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

    pub fn request_add_members(&self, conversation_id: &str, inbox_ids: &[String]) {
        let msg = typed_obj("add_members");
        set_str(&msg, "conversation_id", conversation_id);
        let arr = js_sys::Array::new();
        for id in inbox_ids {
            arr.push(&JsValue::from_str(id));
        }
        Reflect::set(&msg, &"inbox_ids".into(), &arr).unwrap_throw();
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_remove_member(&self, conversation_id: &str, inbox_id: &str) {
        let msg = typed_obj("remove_member");
        set_str(&msg, "conversation_id", conversation_id);
        set_str(&msg, "inbox_id", inbox_id);
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_set_admin(&self, conversation_id: &str, inbox_id: &str, add: bool) {
        let msg = typed_obj("set_admin");
        set_str(&msg, "conversation_id", conversation_id);
        set_str(&msg, "inbox_id", inbox_id);
        Reflect::set(&msg, &"add".into(), &JsValue::from_bool(add)).unwrap_throw();
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_set_super_admin(&self, conversation_id: &str, inbox_id: &str, add: bool) {
        let msg = typed_obj("set_super_admin");
        set_str(&msg, "conversation_id", conversation_id);
        set_str(&msg, "inbox_id", inbox_id);
        Reflect::set(&msg, &"add".into(), &JsValue::from_bool(add)).unwrap_throw();
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_list_members(&self, conversation_id: &str) {
        let msg = typed_obj("list_members");
        set_str(&msg, "conversation_id", conversation_id);
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_list_messages(&self, conversation_id: &str) {
        let msg = typed_obj("list_messages");
        set_str(&msg, "conversation_id", conversation_id);
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_send_message(&self, conversation_id: &str, text: &str) {
        let msg = typed_obj("send_message");
        set_str(&msg, "conversation_id", conversation_id);
        set_str(&msg, "text", text);
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_update_group_name(&self, conversation_id: &str, name: &str) {
        let msg = typed_obj("update_group_name");
        set_str(&msg, "conversation_id", conversation_id);
        set_str(&msg, "name", name);
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_leave(&self, id: String) {
        let msg = typed_obj("leave");
        set_str(&msg, "id", &id);
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_accept_invitation(&self, id: &str) {
        let msg = typed_obj("accept_invitation");
        set_str(&msg, "id", id);
        self.worker.post_message(&msg).unwrap_throw();
    }

    pub fn request_decline_invitation(&self, id: &str) {
        let msg = typed_obj("decline_invitation");
        set_str(&msg, "id", id);
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
    env: Env,
    key_hexes: Vec<String>,
    on_identity_update: impl Fn(IdentityListUpdate) + 'static,
    on_conversations:   impl Fn(Vec<ConversationSummary>) + 'static,
    on_messages:        impl Fn(String, Vec<MessageInfo>) + 'static,
    on_group_members:   impl Fn(Vec<MemberInfo>) + 'static,
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
                let (msg_type, host) = match &env {
                    Env::Production(_) => ("init_production_env", None),
                    Env::Local(h)      => ("init_local_env", Some(h.clone())),
                    Env::Dev(_)        => ("init_dev_env", None),
                };
                let msg = typed_obj(msg_type);
                if let Some(h) = host {
                    set_str(&msg, "host", &h);
                }
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
            "messages" => {
                let conv_id = str_field(&data, "conversation_id");
                let raw = Reflect::get(&data, &"data".into())
                    .ok()
                    .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
                    .unwrap_or_default();
                let messages: Vec<MessageInfo> = (0..raw.length()).map(|i| {
                    let item       = raw.get(i);
                    let sent_at_ns = Reflect::get(&item, &"sent_at_ns".into())
                        .ok().and_then(|v| v.as_f64()).unwrap_or(0.0) as i64;
                    let delivered  = Reflect::get(&item, &"delivered".into())
                        .ok().and_then(|v| v.as_bool()).unwrap_or(false);
                    let system_text = Reflect::get(&item, &"system_text".into())
                        .ok().and_then(|v| v.as_string());
                    MessageInfo {
                        id:              str_field(&item, "id"),
                        text:            str_field(&item, "text"),
                        system_text,
                        sender_inbox_id: str_field(&item, "sender_inbox_id"),
                        sent_at_ns,
                        delivered,
                    }
                }).collect();
                on_messages(conv_id, messages);
            }
            "group_members" => {
                let raw = Reflect::get(&data, &"data".into())
                    .ok()
                    .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
                    .unwrap_or_default();
                let inbox_id_key = JsValue::from_str("inbox_id");
                let address_key  = JsValue::from_str("address");
                let role_key     = JsValue::from_str("role");
                let members: Vec<MemberInfo> = (0..raw.length()).filter_map(|i| {
                    let item     = raw.get(i);
                    let inbox_id = Reflect::get(&item, &inbox_id_key).ok()?.as_string()?;
                    let address  = Reflect::get(&item, &address_key).ok()?.as_string()?;
                    let role     = Reflect::get(&item, &role_key).ok()?.as_f64()? as u8;
                    Some(MemberInfo { inbox_id, address, role })
                }).collect();
                on_group_members(members);
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
            let is_pending = Reflect::get(&item, &"is_pending".into())
                .ok().and_then(|v| v.as_bool()).unwrap_or(false);
            let last_message_ns = Reflect::get(&item, &"last_message_ns".into())
                .ok().and_then(|v| v.as_f64()).map(|f| f as i64);
            Some(ConversationSummary {
                id,
                name:        str_field(&item, "name"),
                last_sender: Reflect::get(&item, &"last_sender".into())
                    .ok()
                    .and_then(|v| v.as_string()),
                last_message_ns,
                is_pending,
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

fn post_group_members(scope: &web_sys::DedicatedWorkerGlobalScope, conversation_id: &str, members: &[MemberInfo]) {
    let arr = js_sys::Array::new();
    for m in members {
        let item = js_sys::Object::new();
        set_str(&item, "inbox_id", &m.inbox_id);
        set_str(&item, "address", &m.address);
        Reflect::set(&item, &"role".into(), &JsValue::from_f64(m.role as f64)).unwrap_throw();
        arr.push(&item);
    }
    let msg = typed_obj("group_members");
    set_str(&msg, "conversation_id", conversation_id);
    Reflect::set(&msg, &"data".into(), &arr).unwrap_throw();
    scope.post_message(&msg).unwrap_throw();
}

fn post_messages(scope: &web_sys::DedicatedWorkerGlobalScope, conversation_id: &str, msgs: &[MessageInfo]) {
    let arr = js_sys::Array::new();
    for m in msgs {
        let item = js_sys::Object::new();
        set_str(&item, "id",              &m.id);
        set_str(&item, "text",            &m.text);
        set_str(&item, "sender_inbox_id", &m.sender_inbox_id);
        Reflect::set(&item, &"sent_at_ns".into(), &JsValue::from_f64(m.sent_at_ns as f64)).unwrap_throw();
        Reflect::set(&item, &"delivered".into(),  &JsValue::from_bool(m.delivered)).unwrap_throw();
        if let Some(ref st) = m.system_text {
            set_str(&item, "system_text", st);
        }
        arr.push(&item);
    }
    let msg = typed_obj("messages");
    set_str(&msg, "conversation_id", conversation_id);
    Reflect::set(&msg, &"data".into(), &arr).unwrap_throw();
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
        Reflect::set(
            &item,
            &"last_message_ns".into(),
            &c.last_message_ns.map(|v| JsValue::from_f64(v as f64)).unwrap_or(JsValue::null()),
        ).unwrap_throw();
        Reflect::set(&item, &"is_pending".into(), &JsValue::from_bool(c.is_pending)).unwrap_throw();
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
