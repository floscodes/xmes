#![recursion_limit = "256"]

mod components;

use dioxus::prelude::*;
use dioxus_sdk::storage::use_persistent;
use xmes_xmtp_wasm::{
    ConversationSummary,
    IdentityInfo,
    IdentityListUpdate,
    XmtpHandle,
    is_worker_context,
    init_worker_mode,
    spawn_xmtp_worker,
};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/styling/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

#[derive(Clone, PartialEq)]
pub enum View {
    Conversations,
    Identities,
    Chat(ConversationSummary),
}

fn main() {
    if is_worker_context() {
        init_worker_mode();
        return;
    }
    dioxus::launch(App);
}

/// Serialize a list of key-hex strings to a compact JSON array.
fn keys_to_json(keys: &[String]) -> String {
    let items: Vec<String> = keys.iter().map(|k| format!("\"{}\"", k)).collect();
    format!("[{}]", items.join(","))
}

/// Parse a JSON array of 64-char hex strings. Handles both `["a","b"]`
/// and a plain single string (legacy `signing_key` format).
fn json_to_keys(s: &str) -> Vec<String> {
    let trimmed = s.trim();
    if trimmed.starts_with('[') {
        // JSON array
        trimmed
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|p| p.trim().trim_matches('"').to_string())
            .filter(|k| k.len() == 64)
            .collect()
    } else {
        // Legacy single key
        let k = trimmed.trim_matches('"').to_string();
        if k.len() == 64 { vec![k] } else { vec![] }
    }
}

#[component]
fn App() -> Element {
    // `signing_keys` stores a JSON array of private-key hex strings.
    // Legacy single-key values are migrated automatically by `json_to_keys`.
    let signing_keys: Signal<Option<String>> = use_persistent("signing_keys", || None);

    let mut xmtp_handle:   Signal<Option<XmtpHandle>>              = use_signal(|| None);
    let conversations:     Signal<Option<Vec<ConversationSummary>>> = use_signal(|| None);
    let identity_ready:    Signal<bool>                             = use_signal(|| false);
    let identity_info:     Signal<Option<IdentityInfo>>             = use_signal(|| None);
    let all_identities:    Signal<Vec<IdentityInfo>>                = use_signal(|| vec![]);
    let view:              Signal<View>                             = use_signal(|| View::Conversations);
    let anim:              Signal<&'static str>                     = use_signal(|| "");
    let pending_open:      Signal<Option<()>>                       = use_signal(|| None);

    use_context_provider(|| xmtp_handle);
    use_context_provider(|| conversations);
    use_context_provider(|| identity_ready);
    use_context_provider(|| identity_info);
    use_context_provider(|| all_identities);
    use_context_provider(|| view);
    use_context_provider(|| anim);
    use_context_provider(|| pending_open);

    use_resource(move || async move {
        if xmtp_handle.read().is_some() {
            return;
        }

        // Parse stored keys; supports legacy single-key format.
        let key_hexes = signing_keys.peek().as_deref()
            .map(json_to_keys)
            .unwrap_or_default();

        let handle = spawn_xmtp_worker(
            key_hexes,
            move |update: IdentityListUpdate| {
                // Persist all keys as JSON array.
                let keys: Vec<String> = update.identities.iter().map(|i| i.key_hex.clone()).collect();
                let mut sk = signing_keys;
                sk.set(Some(keys_to_json(&keys)));

                // Active identity info.
                let active = update.identities.get(update.active_idx).cloned();
                let mut ii = identity_info;
                ii.set(active);

                // Full identity list.
                let mut ai = all_identities;
                ai.set(update.identities);

                // Mark identity as ready.
                let mut ir = identity_ready;
                ir.set(true);

                // Trigger conversation list for the (possibly new) active identity.
                if let Some(h) = xmtp_handle.peek().as_ref() {
                    h.request_list();
                }
            },
            move |convos| {
                if pending_open.peek().is_some() {
                    let mut po = pending_open;
                    po.set(None);
                    if let Some(first) = convos.first().cloned() {
                        let mut a = anim; a.set("slide-in-right");
                        let mut v = view; v.set(View::Chat(first));
                    }
                }
                let mut c = conversations;
                c.set(Some(convos));
            },
        );

        xmtp_handle.set(Some(handle));
    });

    let in_chat = matches!(*view.read(), View::Chat(_));

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "icon", r#type: "image/png", sizes: "32x32", href: "/icons/icon-32x32.png" }
        document::Link { rel: "icon", r#type: "image/png", sizes: "16x16", href: "/icons/icon-16x16.png" }
        document::Link { rel: "apple-touch-icon", sizes: "180x180", href: "/icons/icon-180x180.png" }
        document::Link { rel: "apple-touch-icon", sizes: "167x167", href: "/icons/icon-167x167.png" }
        document::Link { rel: "apple-touch-icon", sizes: "152x152", href: "/icons/icon-152x152.png" }
        document::Link { rel: "manifest", href: "/manifest.webmanifest" }
        document::Meta { name: "theme-color",                    content: "#4F46E5" }
        document::Meta { name: "mobile-web-app-capable",         content: "yes" }
        document::Meta { name: "apple-mobile-web-app-capable",   content: "yes" }
        document::Meta { name: "apple-mobile-web-app-status-bar-style", content: "default" }
        document::Meta { name: "apple-mobile-web-app-title",     content: "xmes" }
        document::Script { src: "/register-sw.js" }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }

        match view.read().clone() {
            View::Conversations => rsx! {
                div { class: "view-slide {anim}",
                    components::conversations::Conversations {}
                }
            },
            View::Identities => rsx! {
                div { class: "view-slide {anim}",
                    components::identities::Identities {}
                }
            },
            View::Chat(conversation) => rsx! {
                div { class: "view-slide {anim}",
                    components::chat::Chat { conversation }
                }
            },
        }

        if !in_chat {
            nav { class: "bottom-nav",
                button {
                    class: if *view.read() == View::Identities { "bottom-nav-tab active" } else { "bottom-nav-tab" },
                    onclick: move |_| {
                        let mut a = anim; a.set("slide-in-tab");
                        let mut v = view; v.set(View::Identities);
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "22", height: "22",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: if *view.read() == View::Identities { "2.5" } else { "1.8" },
                        stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" }
                        circle { cx: "12", cy: "7", r: "4" }
                    }
                    span { "Identity" }
                }
                button {
                    class: if *view.read() == View::Conversations { "bottom-nav-tab active" } else { "bottom-nav-tab" },
                    onclick: move |_| {
                        let mut a = anim; a.set("slide-in-tab");
                        let mut v = view; v.set(View::Conversations);
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "22", height: "22",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: if *view.read() == View::Conversations { "2.5" } else { "1.8" },
                        stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                    }
                    span { "Conversations" }
                }
            }
        }
    }
}
