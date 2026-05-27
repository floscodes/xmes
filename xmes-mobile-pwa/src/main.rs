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

#[derive(Clone, PartialEq)]
pub struct DarkMode(pub bool);

fn main() {
    if is_worker_context() {
        init_worker_mode();
        return;
    }
    // Expose push worker URL to JS (read by register-sw.js for push subscription).
    // Set at startup so it's available before the load event fires.
    let push_url = option_env!("PUSH_WORKER_URL").unwrap_or("");
    if !push_url.is_empty() {
        let _ = js_sys::eval(&format!("window.XMES_PUSH_WORKER_URL='{push_url}'"));
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
    let unread_ids:       Signal<std::collections::HashSet<String>>       = use_storage::<LocalStorage, _>("unread_ids".to_string(),       || std::collections::HashSet::new());
    let last_seen_ns:     Signal<std::collections::HashMap<String, i64>> = use_storage::<LocalStorage, _>("last_seen_ns".to_string(),     || std::collections::HashMap::new());
    // Per-inbox unread indicator: inbox_ids that currently have unread messages.
    // Uses BTreeSet (not HashSet) to have a distinct type for Dioxus context lookup.
    let inbox_has_unread: Signal<std::collections::BTreeSet<String>>      = use_storage::<LocalStorage, _>("inbox_has_unread".to_string(), || std::collections::BTreeSet::new());
    // Persists the inbox_id of the last active identity so it can be restored on next launch.
    let last_active_inbox: Signal<Option<String>> = use_storage::<LocalStorage, _>("last_active_inbox".to_string(), || None);
    // Persistent address → alias display name map.
    let mut aliases: Signal<std::collections::BTreeMap<String, String>> =
        use_storage::<LocalStorage, _>("addr_aliases".to_string(), || std::collections::BTreeMap::new());
    // group_id → inbox_id: block push after leave is confirmed by XMTP
    let mut pending_blocks: Signal<std::collections::HashMap<String, String>> = use_signal(|| std::collections::HashMap::new());

    let dark_mode: Signal<DarkMode> = use_signal(|| {
        let is_dark = js_sys::eval(
            "var s=localStorage.getItem('xmes-theme');\
             s==='dark'||(s===null&&window.matchMedia&&window.matchMedia('(prefers-color-scheme:dark)').matches)"
        ).ok().and_then(|v| v.as_bool()).unwrap_or(false);
        DarkMode(is_dark)
    });

    use_effect(move || {
        let theme = if dark_mode.read().0 { "dark" } else { "light" };
        let _ = js_sys::eval(&format!(
            "document.documentElement.setAttribute('data-theme','{}');\
             document.documentElement.setAttribute('lang',navigator.language||'en');\
             localStorage.setItem('xmes-theme','{}')",
            theme, theme
        ));
    });

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
    use_context_provider(|| inbox_has_unread);
    use_context_provider(|| dark_mode);
    use_context_provider(|| pending_blocks);
    use_context_provider(|| aliases);

    // Keep app-icon badge in sync with unread conversation count.
    use_effect(move || {
        let count = unread_ids.read().len();
        let js = if count > 0 {
            format!(
                "var n={count};\
                 if('setAppBadge' in navigator) navigator.setAppBadge(n);\
                 navigator.serviceWorker&&navigator.serviceWorker.ready.then(function(sw){{sw.active&&sw.active.postMessage({{type:'sync-badge',count:n}})}});"
            )
        } else {
            "if('clearAppBadge' in navigator) navigator.clearAppBadge();\
             navigator.serviceWorker&&navigator.serviceWorker.ready.then(function(sw){sw.active&&sw.active.postMessage({type:'sync-badge',count:0})});"
                .to_string()
        };
        let _ = js_sys::eval(&js);
    });

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

        let is_first_identity_cb = std::rc::Rc::new(std::cell::Cell::new(true));
        let is_first_identity_cb2 = is_first_identity_cb.clone();

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

                // Active identity — on the very first callback, restore the inbox saved
                // from the last session instead of always defaulting to index 0.
                let active = {
                    let mut resolved = update.identities.get(update.active_idx).cloned();
                    if is_first_identity_cb2.get() {
                        is_first_identity_cb2.set(false);
                        if let Some(saved) = last_active_inbox.peek().clone() {
                            if let Some(target_idx) = update.identities.iter().position(|i| i.inbox_id == saved) {
                                if target_idx != update.active_idx {
                                    resolved = update.identities.get(target_idx).cloned();
                                    if let Some(h) = xmtp_handle.peek().as_ref() {
                                        h.request_switch_identity(target_idx);
                                    }
                                }
                            }
                        }
                    }
                    resolved
                };
                // Persist whichever identity is now active for next launch.
                if let Some(ref id) = active {
                    let mut lai = last_active_inbox;
                    lai.set(Some(id.inbox_id.clone()));
                }
                // Expose inbox ID to JS, then auto-subscribe push if already permitted.
                if let Some(ref id) = active {
                    let escaped_inbox = id.inbox_id.replace('\'', "\\'");
                    let escaped_addr  = id.primary_address.replace('\'', "\\'");
                    let _ = js_sys::eval(&format!(
                        "window.XMES_INBOX_ID='{escaped_inbox}';window.XMES_ETH_ADDRESS='{escaped_addr}';window.xmesSubscribePush&&window.xmesSubscribePush();window.xmesEnablePushOnNextTap&&window.xmesEnablePushOnNextTap()"
                    ));
                }
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

                // Update per-inbox unread indicator for the active identity.
                {
                    let active_inbox = identity_info.peek()
                        .as_ref()
                        .map(|i| i.inbox_id.clone())
                        .unwrap_or_default();
                    if !active_inbox.is_empty() {
                        let has_unread = convos.iter()
                            .any(|c| unread_ids.peek().contains(&c.id));
                        let mut ihu = inbox_has_unread;
                        if has_unread {
                            ihu.write().insert(active_inbox);
                        } else {
                            ihu.write().remove(&active_inbox);
                        }
                    }
                }

                // Fire xmesBlockGroup once the group has left the list (leave confirmed).
                let convo_ids: std::collections::HashSet<&str> = convos.iter().map(|c| c.id.as_str()).collect();
                let blocks_snapshot: Vec<(String, String)> = pending_blocks.peek()
                    .iter()
                    .filter(|(gid, _)| !convo_ids.contains(gid.as_str()))
                    .map(|(g, i)| (g.clone(), i.clone()))
                    .collect();
                if !blocks_snapshot.is_empty() {
                    let mut pb = pending_blocks;
                    for (gid, iid) in &blocks_snapshot {
                        pb.write().remove(gid);
                        let g = gid.replace('\'', "\\'");
                        let i = iid.replace('\'', "\\'");
                        let _ = js_sys::eval(&format!(
                            "window.xmesBlockGroup&&window.xmesBlockGroup('{i}','{g}')"
                        ));
                    }
                }

                let mut c = conversations;
                c.set(Some(convos));
            },
            move |conv_id, msgs| {
                // Fire push only when a pending own message is confirmed delivered.
                // Strategy:
                //   - Send handler increments window.__xmes_push_pending (a counter).
                //   - Here we maintain a per-conversation seen-set of delivered own msg IDs.
                //   - First call: initialise the set (no push, avoids spurious pushes for
                //     historical messages already present on open).
                //   - Subsequent calls: for each newly-delivered own message, if
                //     __xmes_push_pending > 0, fire push once and decrement the counter.
                let own = identity_info.peek()
                    .as_ref()
                    .map(|i| i.inbox_id.clone())
                    .unwrap_or_default();
                if !own.is_empty() {
                    let delivered_own_ids = msgs.iter()
                        .filter(|msg| msg.delivered && msg.sender_inbox_id == own)
                        .map(|msg| msg.id.clone())
                        .collect::<Vec<_>>();
                    if !delivered_own_ids.is_empty() {
                        let ids_js = delivered_own_ids.iter()
                            .map(|id| format!("\"{}\"", id.replace('"', "\\\"")))
                            .collect::<Vec<_>>()
                            .join(",");
                        let conv_key = conv_id.replace(|c: char| !c.is_alphanumeric(), "_");
                        let cname = conversations.peek()
                            .as_ref()
                            .and_then(|cs| cs.iter().find(|c| c.id == conv_id))
                            .map(|c| c.name.clone())
                            .unwrap_or_default();
                        let members_js = group_members.peek().iter()
                            .filter(|m| m.inbox_id != own)
                            .map(|m| format!("\"{}\"", m.inbox_id.replace('"', "\\\"")))
                            .collect::<Vec<_>>()
                            .join(",");
                        let sender = own.replace('"', "");
                        let name   = cname.replace('"', "").replace('\\', "");
                        let _ = js_sys::eval(&format!(
                            r#"(function(){{
                                var k='__xmes_seen_{conv_key}';
                                var ids=[{ids_js}];
                                if(!window[k]){{ window[k]=new Set(ids); return; }}
                                ids.forEach(function(id){{
                                    if(window[k].has(id))return;
                                    window[k].add(id);
                                    if(!(window.__xmes_push_pending>0))return;
                                    window.__xmes_push_pending--;
                                    var u=window.XMES_PUSH_WORKER_URL;
                                    if(!u)return;
                                    fetch(u+"/notify",{{method:"POST",headers:{{"content-type":"application/json"}},body:JSON.stringify({{member_inbox_ids:[{members_js}],sender_inbox_id:"{sender}",group_name:"{name}"}})}}).catch(()=>{{}});
                                }});
                            }})()"#
                        ));
                    }
                }
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

    // Burst sync: fire every 3 s for the first 5 calls, then fall back to 12 s steady polling.
    use_effect(move || {
        let burst_count = std::rc::Rc::new(std::cell::Cell::new(0u8));
        let burst_count2 = burst_count.clone();
        let burst = gloo_timers::callback::Interval::new(3_000, move || {
            let n = burst_count2.get();
            if n >= 5 { return; }
            burst_count2.set(n + 1);
            if let Some(h) = xmtp_handle.peek().as_ref() {
                h.request_list();
            }
        });
        burst.forget();

        let interval = gloo_timers::callback::Interval::new(12_000, move || {
            if let Some(h) = xmtp_handle.peek().as_ref() {
                h.request_list();
            }
        });
        interval.forget();
    });

    // Refresh on foreground: register a JS visibilitychange listener that sets a
    // flag, then poll every second and trigger the right request when it fires.
    // Also absorbs any push inbox IDs accumulated in window.XMES_PUSH_INBOX_LIST.
    use_effect(move || {
        let _ = js_sys::eval(
            "document.addEventListener('visibilitychange',function(){\
                if(!document.hidden)window.__xmes_became_visible=1;\
            })"
        );
        let interval = gloo_timers::callback::Interval::new(1_000, move || {
            // Absorb any inbox IDs that received a push (set by register-sw.js).
            let push_list = js_sys::eval("window.XMES_PUSH_INBOX_LIST||''")
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            if !push_list.is_empty() {
                let mut ihu = inbox_has_unread;
                for id in push_list.split(',') {
                    if !id.is_empty() {
                        ihu.write().insert(id.to_string());
                    }
                }
                let _ = js_sys::eval("window.XMES_PUSH_INBOX_LIST=''");
            }

            let flagged = js_sys::eval("window.__xmes_became_visible||0")
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as u8;
            if flagged == 0 { return; }
            let _ = js_sys::eval("window.__xmes_became_visible=0");

            if let Some(h) = xmtp_handle.peek().as_ref() {
                h.request_list();
                if let View::Chat(conv) = view.peek().clone() {
                    h.request_list_messages(&conv.id);
                    h.request_list_members(&conv.id);
                }
            }
        });
        interval.forget();
    });

    let in_chat = matches!(*view.read(), View::Chat(_));

    // use_memo ensures Dioxus tracks signal dependencies and re-evaluates reactively.
    // Case (a): inbox_has_unread — populated by conversations callback & push SW interval.
    // Case (b): any unread_id not in the active identity's conversations (stale from previous sync).
    let other_identity_unread = use_memo(move || {
        let ihu = inbox_has_unread.read();
        let active_inbox = identity_info.read().as_ref()
            .map(|i| i.inbox_id.clone())
            .unwrap_or_default();
        let has_via_ihu = ihu.iter().any(|id| id != &active_inbox);
        let has_via_unread_ids = {
            let unreads = unread_ids.read();
            if let Some(convos) = conversations.read().as_ref() {
                let active_ids: std::collections::HashSet<&str> =
                    convos.iter().map(|c| c.id.as_str()).collect();
                unreads.iter().any(|id| !active_ids.contains(id.as_str()))
            } else {
                false
            }
        };
        has_via_ihu || has_via_unread_ids
    });

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
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }

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
                    div { class: "bottom-nav-icon-wrap",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "22", height: "22",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: if *view.read() == View::Identities { "2.5" } else { "1.8" },
                            stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" }
                            circle { cx: "12", cy: "7", r: "4" }
                        }
                        if other_identity_unread() {
                            div { class: "nav-unread-badge" }
                        }
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
