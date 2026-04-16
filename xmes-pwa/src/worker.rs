use js_sys::Reflect;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use xmes_xmtp_wasm::{ConversationSummary, Env, Identity};

type IdentityRef = Rc<RefCell<Option<Rc<Identity>>>>;

const XMTP_HOST: &str = "https://xmtp-dev.floscodes.net";

/// Entry point when the WASM module is running inside a Dedicated Worker.
/// Sets up the message handler and signals readiness to the main thread.
pub async fn run() {
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
                let toml = opt_str_field(&data, "toml");
                spawn_local(handle_init(scope, identity, toml));
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

    // Notify the main thread that the worker message handler is ready.
    let msg = typed_obj("worker_ready");
    scope.post_message(&msg).unwrap_throw();
}

// --- message handlers ---

async fn handle_init(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: IdentityRef,
    toml: Option<String>,
) {
    let identity = match toml {
        Some(hex) => match Identity::from_key_hex(hex, Env::Dev(Some(XMTP_HOST.to_string()))).await {
            Ok(id) => Some(Rc::new(id)),
            Err(_) => new_identity().await,
        },
        None => new_identity().await,
    };

    match identity {
        Some(id) => {
            let new_toml = id.to_key_hex();
            *state.borrow_mut() = Some(id);
            let msg = typed_obj("ready");
            set_str(&msg, "toml", &new_toml);
            scope.post_message(&msg).unwrap_throw();
        }
        None => post_error(&scope, "Failed to initialize identity"),
    }
}

async fn handle_list(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: IdentityRef,
) {
    let id = state.borrow().clone();
    match id {
        Some(id) => match id.list_conversations().await {
            Ok(convos) => post_conversations(&scope, &convos),
            Err(e) => post_error(&scope, &e.to_string()),
        },
        None => post_error(&scope, "Identity not initialized"),
    }
}

async fn handle_create_group(
    scope: web_sys::DedicatedWorkerGlobalScope,
    state: IdentityRef,
) {
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

async fn new_identity() -> Option<Rc<Identity>> {
    Identity::new(Env::Dev(Some(XMTP_HOST.to_string())))
        .await
        .ok()
        .map(Rc::new)
}

// --- serialization helpers ---

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
    if v.is_null() || v.is_undefined() {
        None
    } else {
        v.as_string()
    }
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
            &c.last_sender
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::null()),
        )
        .unwrap_throw();
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
