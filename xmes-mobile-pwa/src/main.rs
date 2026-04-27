#![recursion_limit = "256"]

mod components;
mod crypto_store;

use std::sync::Arc;
use dioxus::prelude::*;
use dioxus_sdk::storage::{LocalStorage, use_storage};
use xmes_xmtp_wasm::{
    ConversationSummary,
    Env,
    IdentityInfo,
    IdentityListUpdate,
    MemberInfo,
    MessageInfo,
    XmtpHandle,
    is_worker_context,
    init_worker_mode,
    spawn_xmtp_worker,
};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/styling/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const APPLE_ICON_180: Asset = asset!("/assets/icons/icon-180x180.png");
const APPLE_ICON_167: Asset = asset!("/assets/icons/icon-167x167.png");
const APPLE_ICON_152: Asset = asset!("/assets/icons/icon-152x152.png");
const APPLE_ICON_120: Asset = asset!("/assets/icons/icon-120x120.png");

/// A pending confirmation action. Store it in the `confirm_action` context
/// signal to show the modal; set it back to `None` to dismiss.
#[derive(Clone)]
pub struct ConfirmAction {
    pub title:         String,
    pub message:       String,
    pub confirm_label: String,
    pub on_confirm:    Arc<dyn Fn() + 'static>,
}

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
        trimmed
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|p| p.trim().trim_matches('"').to_string())
            .filter(|k| k.len() == 64)
            .collect()
    } else {
        let k = trimmed.trim_matches('"').to_string();
        if k.len() == 64 { vec![k] } else { vec![] }
    }
}

/// Parse a JSON array of arbitrary strings (used for mnemonics).
fn json_to_strings(s: &str) -> Vec<String> {
    let trimmed = s.trim();
    if trimmed.starts_with('[') {
        trimmed
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|p| p.trim().trim_matches('"').to_string())
            .collect()
    } else {
        vec![]
    }
}

/// Serialize a list of optional strings as a JSON array (empty string for None).
fn mnemonics_to_json(mnemonics: &[Option<String>]) -> String {
    let items: Vec<String> = mnemonics.iter()
        .map(|m| format!("\"{}\"", m.as_deref().unwrap_or("")))
        .collect();
    format!("[{}]", items.join(","))
}

#[component]
fn App() -> Element {
    // `signing_keys` stores a JSON array of private-key hex strings.
    // Legacy single-key values are migrated automatically by `json_to_keys`.
    let signing_keys: Signal<Option<String>> = use_storage::<LocalStorage, _>("signing_keys".to_string(), || None);
    // `mnemonics_v1` stores a JSON array of BIP39 phrases parallel to signing_keys.
    // Empty string means no mnemonic for that identity.
    let mnemonics_storage: Signal<Option<String>> = use_storage::<LocalStorage, _>("mnemonics_v1".to_string(), || None);

    let mut xmtp_handle:   Signal<Option<XmtpHandle>>              = use_signal(|| None);
    let conversations:     Signal<Option<Vec<ConversationSummary>>> = use_signal(|| None);
    let identity_ready:    Signal<bool>                             = use_signal(|| false);
    let identity_info:     Signal<Option<IdentityInfo>>             = use_signal(|| None);
    let all_identities:    Signal<Vec<IdentityInfo>>                = use_signal(|| vec![]);
    let view:              Signal<View>                             = use_signal(|| View::Conversations);
    let anim:              Signal<&'static str>                     = use_signal(|| "");
    let pending_open:      Signal<Option<()>>                       = use_signal(|| None);
    let confirm_action:    Signal<Option<ConfirmAction>>            = use_signal(|| None);
    let messages:          Signal<Vec<MessageInfo>>                 = use_signal(|| vec![]);
    let group_members:     Signal<Vec<MemberInfo>>                  = use_signal(|| vec![]);
    let unread_ids:   Signal<std::collections::HashSet<String>>       = use_storage::<LocalStorage, _>("unread_ids".to_string(),   || std::collections::HashSet::new());
    let last_seen_ns: Signal<std::collections::HashMap<String, i64>> = use_storage::<LocalStorage, _>("last_seen_ns".to_string(), || std::collections::HashMap::new());

    use_context_provider(|| xmtp_handle);
    use_context_provider(|| conversations);
    use_context_provider(|| identity_ready);
    use_context_provider(|| identity_info);
    use_context_provider(|| all_identities);
    use_context_provider(|| view);
    use_context_provider(|| anim);
    use_context_provider(|| pending_open);
    use_context_provider(|| confirm_action);
    use_context_provider(|| messages);
    use_context_provider(|| group_members);
    use_context_provider(|| unread_ids);

    use_resource(move || async move {
        if xmtp_handle.read().is_some() {
            return;
        }

        // Decrypt stored keys; falls back to plaintext for migration from older versions.
        let key_hexes = if let Some(raw) = signing_keys.peek().clone() {
            let plaintext = crypto_store::decrypt(&raw).unwrap_or(raw);
            json_to_keys(&plaintext)
        } else {
            vec![]
        };
        let mnemonics_loaded: Vec<Option<String>> = if let Some(raw) = mnemonics_storage.peek().clone() {
            let plaintext = crypto_store::decrypt(&raw).unwrap_or(raw);
            json_to_strings(&plaintext).into_iter().map(|s| if s.is_empty() { None } else { Some(s) }).collect()
        } else {
            vec![]
        };

        let env = if option_env!("PRODUCTION").is_some() {
            Env::Production(None)
        } else {
            Env::Dev(None)
        };

        let handle = spawn_xmtp_worker(
            env,
            key_hexes,
            mnemonics_loaded,
            move |update: IdentityListUpdate| {
                // Encrypt and persist all keys as JSON array.
                let keys: Vec<String> = update.identities.iter().map(|i| i.key_hex.clone()).collect();
                let json = keys_to_json(&keys);
                let mut sk = signing_keys;
                if let Some(encrypted) = crypto_store::encrypt(&json) {
                    sk.set(Some(encrypted));
                }
                // Persist mnemonics in parallel.
                let mnemos: Vec<Option<String>> = update.identities.iter().map(|i| i.mnemonic.clone()).collect();
                let mnemonic_json = mnemonics_to_json(&mnemos);
                let mut ms = mnemonics_storage;
                if let Some(encrypted) = crypto_store::encrypt(&mnemonic_json) {
                    ms.set(Some(encrypted));
                }

                // Active identity.
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

                // Detect new messages: compare last_message_ns against what we last saw.
                // Skip conversations currently open (user is already reading them).
                // Skip conversations where the last message was sent by ourselves.
                let open_id = match view.peek().clone() {
                    View::Chat(c) => Some(c.id),
                    _ => None,
                };
                let my_inbox_id = identity_info.peek()
                    .as_ref()
                    .map(|i| i.inbox_id.clone());
                let mut seen = last_seen_ns;
                let mut unread = unread_ids;
                for conv in &convos {
                    if let Some(ns) = conv.last_message_ns {
                        let prev = seen.peek().get(&conv.id).copied().unwrap_or(0);
                        if ns > prev {
                            // Only mark unread when the conversation is not currently open
                            if Some(&conv.id) != open_id.as_ref() {
                                let last_sender_is_me = my_inbox_id.as_deref()
                                    .zip(conv.last_sender_inbox_id.as_deref())
                                    .map(|(me, sender)| me == sender)
                                    .unwrap_or(false);
                                if !last_sender_is_me {
                                    unread.write().insert(conv.id.clone());
                                }
                            }
                        }
                        // Always advance the watermark so closing the chat doesn't re-trigger unread
                        seen.write().insert(conv.id.clone(), ns);
                    }
                }

                let mut c = conversations;
                c.set(Some(convos));
            },
            move |_conv_id, msgs| {
                let mut m = messages;
                m.set(msgs);
            },
            move |members| {
                let mut gm = group_members;
                gm.set(members);
            },
        );

        xmtp_handle.set(Some(handle));
    });

    // Periodic sync every 12 seconds
    use_effect(move || {
        let interval = gloo_timers::callback::Interval::new(12_000, move || {
            if let Some(h) = xmtp_handle.peek().as_ref() {
                h.request_list();
            }
        });
        interval.forget(); // keep running for the lifetime of the app
    });

    let in_chat = matches!(*view.read(), View::Chat(_));

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "icon", r#type: "image/png", sizes: "32x32", href: "/icons/icon-32x32.png" }
        document::Link { rel: "icon", r#type: "image/png", sizes: "16x16", href: "/icons/icon-16x16.png" }
        document::Link { rel: "apple-touch-icon", sizes: "180x180", href: APPLE_ICON_180 }
        document::Link { rel: "apple-touch-icon", sizes: "167x167", href: APPLE_ICON_167 }
        document::Link { rel: "apple-touch-icon", sizes: "152x152", href: APPLE_ICON_152 }
        document::Link { rel: "apple-touch-icon", sizes: "120x120", href: APPLE_ICON_120 }
        document::Link { rel: "manifest", href: "/manifest.webmanifest" }
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no, viewport-fit=cover" }
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

        // ── Confirmation modal ───────────────────────────────────
        if let Some(action) = confirm_action.read().clone() {
            div {
                class: "modal-backdrop",
                onclick: move |_| { let mut ca = confirm_action; ca.set(None); },
            }
            div { class: "modal-card",
                h3 { class: "modal-title", "{action.title}" }
                p  { class: "modal-message", "{action.message}" }
                div { class: "modal-buttons",
                    button {
                        class: "modal-btn modal-cancel",
                        onclick: move |_| { let mut ca = confirm_action; ca.set(None); },
                        "Cancel"
                    }
                    button {
                        class: "modal-btn modal-confirm",
                        onclick: move |_| {
                            (action.on_confirm)();
                            let mut ca = confirm_action;
                            ca.set(None);
                        },
                        "{action.confirm_label}"
                    }
                }
            }
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
