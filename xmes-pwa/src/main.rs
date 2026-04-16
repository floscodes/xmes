#![recursion_limit = "256"]

mod components;

use dioxus::prelude::*;
use dioxus_sdk::storage::use_persistent;
use xmes_xmtp_wasm::{
    ConversationSummary,
    XmtpHandle,
    is_worker_context,
    init_worker_mode,
    spawn_xmtp_worker,
};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/styling/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
    if is_worker_context() {
        init_worker_mode();
        return;
    }
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut signing_key: Signal<Option<String>> = use_persistent("signing_key", || None);
    let mut xmtp_handle: Signal<Option<XmtpHandle>> = use_signal(|| None);
    let mut conversations: Signal<Option<Vec<ConversationSummary>>> = use_signal(|| None);
    let mut identity_ready: Signal<bool> = use_signal(|| false);

    use_context_provider(|| xmtp_handle);
    use_context_provider(|| conversations);
    use_context_provider(|| identity_ready);

    use_resource(move || async move {
        if xmtp_handle.read().is_some() {
            return;
        }

        let key_hex = signing_key.peek().clone();

        let handle = spawn_xmtp_worker(
            key_hex,
            move |new_key_hex| {
                let mut sk = signing_key;
                sk.set(Some(new_key_hex));
                let mut ir = identity_ready;
                ir.set(true);
                if let Some(h) = xmtp_handle.peek().as_ref() {
                    h.request_list();
                }
            },
            move |convos| {
                let mut c = conversations;
                c.set(Some(convos));
            },
        );

        xmtp_handle.set(Some(handle));
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        components::conversations::Conversations {}
    }
}
