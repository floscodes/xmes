#![recursion_limit = "256"]

mod components;
mod worker;

use dioxus::prelude::*;
use dioxus_sdk::storage::use_persistent;
use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use xmes_xmtp_wasm::ConversationSummary;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/styling/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

/// Minimal JS loaded in the Dedicated Worker.
/// It awaits a `wasm_init` message, then dynamically imports the app's ES module.
/// The module auto-initialises the WASM binary; `main()` detects the worker
/// context and delegates to `worker::run()` instead of launching Dioxus.
const WORKER_BOOTSTRAP: &str = r#"
self.addEventListener('message', async function(e) {
    if (e.data.type !== 'wasm_init') return;
    await import(e.data.scriptUrl);
}, { once: true });
"#;

fn main() {
    // When the same WASM binary is loaded inside a Dedicated Worker (via the
    // WORKER_BOOTSTRAP script), run the XMTP message handler instead of Dioxus.
    if js_sys::global()
        .dyn_ref::<web_sys::DedicatedWorkerGlobalScope>()
        .is_some()
    {
        wasm_bindgen_futures::spawn_local(worker::run());
        return;
    }

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Persisted across page loads via localStorage.
    let mut identities_toml: Signal<Option<String>> = use_persistent("identities", || None);

    // In-memory state, reset on every page load.
    let mut worker_handle: Signal<Option<web_sys::Worker>> = use_signal(|| None);
    let mut conversations: Signal<Option<Vec<ConversationSummary>>> = use_signal(|| None);
    let mut identity_ready: Signal<bool> = use_signal(|| false);

    // Make these available to child components via context.
    use_context_provider(|| worker_handle);
    use_context_provider(|| conversations);
    use_context_provider(|| identity_ready);

    // Spawn the XMTP worker once on mount.
    use_resource(move || async move {
        if worker_handle.read().is_some() {
            return;
        }

        let Some(script_url) = find_app_script_url() else {
            return;
        };

        let init_toml = identities_toml.peek().clone();

        // Build a classic Worker from an inline JS blob.
        let arr = js_sys::Array::of1(&JsValue::from_str(WORKER_BOOTSTRAP));
        let mut props = web_sys::BlobPropertyBag::new();
        props.type_("application/javascript");
        let blob =
            web_sys::Blob::new_with_str_sequence_and_options(&arr, &props).unwrap_throw();
        let blob_url = web_sys::Url::create_object_url_with_blob(&blob).unwrap_throw();
        let worker = web_sys::Worker::new(&blob_url).unwrap_throw();
        web_sys::Url::revoke_object_url(&blob_url).unwrap_throw();

        // Clone for use inside the onmessage closure.
        let worker_cb = worker.clone();

        // Handle messages coming back from the worker.
        let onmessage = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            let data = e.data();
            let msg_type = js_str(&data, "type");

            match msg_type.as_str() {
                // Worker WASM has initialised; send the identity init request.
                "worker_ready" => {
                    let msg = js_obj("init");
                    match &init_toml {
                        Some(t) => js_set(&msg, "toml", t),
                        None => {
                            Reflect::set(&msg, &"toml".into(), &JsValue::null())
                                .unwrap_throw();
                        }
                    }
                    worker_cb.post_message(&msg).unwrap_throw();
                }
                // Identity is ready; persist the (possibly refreshed) TOML and
                // request the initial conversation list.
                "ready" => {
                    let toml = js_str(&data, "toml");
                    // Signal is Copy — create local mutable copies so the
                    // closure stays Fn (not FnMut) as Closure::wrap requires.
                    let mut t = identities_toml;
                    t.set(Some(toml));
                    let mut ir = identity_ready;
                    ir.set(true);
                    let msg = js_obj("list");
                    worker_cb.post_message(&msg).unwrap_throw();
                }
                // Conversation list received from the worker.
                "conversations" => {
                    let arr = Reflect::get(&data, &"data".into())
                        .ok()
                        .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
                        .unwrap_or_default();
                    let convos = parse_conversations(&arr);
                    let mut c = conversations;
                    c.set(Some(convos));
                }
                _ => {}
            }
        }) as Box<dyn Fn(web_sys::MessageEvent)>);

        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();

        // Bootstrap the WASM inside the worker.
        let msg = js_obj("wasm_init");
        js_set(&msg, "scriptUrl", &script_url);
        worker.post_message(&msg).unwrap_throw();

        worker_handle.set(Some(worker));
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        components::conversations::Conversations {}
    }
}

// --- helpers ---

/// Find the absolute URL of the app's main script element.
fn find_app_script_url() -> Option<String> {
    let document = web_sys::window()?.document()?;
    let nodes = document.query_selector_all("script[src]").ok()?;
    for i in 0..nodes.length() {
        let el = nodes
            .item(i)?
            .dyn_into::<web_sys::HtmlScriptElement>()
            .ok()?;
        let src = el.src();
        if !src.is_empty() {
            return Some(src);
        }
    }
    None
}

fn js_obj(msg_type: &str) -> js_sys::Object {
    let o = js_sys::Object::new();
    Reflect::set(&o, &"type".into(), &msg_type.into()).unwrap_throw();
    o
}

fn js_set(obj: &js_sys::Object, key: &str, val: &str) {
    Reflect::set(obj, &key.into(), &val.into()).unwrap_throw();
}

fn js_str(val: &JsValue, key: &str) -> String {
    Reflect::get(val, &key.into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default()
}

fn parse_conversations(arr: &js_sys::Array) -> Vec<ConversationSummary> {
    (0..arr.length())
        .filter_map(|i| {
            let item = arr.get(i);
            let id = js_str(&item, "id");
            if id.is_empty() {
                return None;
            }
            let name = js_str(&item, "name");
            let last_sender = Reflect::get(&item, &"last_sender".into())
                .ok()
                .and_then(|v| v.as_string());
            Some(ConversationSummary { id, name, last_sender })
        })
        .collect()
}
