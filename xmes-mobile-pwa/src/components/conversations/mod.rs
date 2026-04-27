use dioxus::prelude::*;

const LOGO: Asset = asset!("/assets/icons/xmes-icon.svg");
use xmes_xmtp_wasm::{ConversationSummary, XmtpHandle};
use crate::View;

mod conversation;

#[component]
pub fn Conversations() -> Element {
    let xmtp = use_context::<Signal<Option<XmtpHandle>>>();
    let conversations = use_context::<Signal<Option<Vec<ConversationSummary>>>>();
    let identity_ready = use_context::<Signal<bool>>();
    let view = use_context::<Signal<View>>();
    let anim = use_context::<Signal<&'static str>>();
    let pending_open = use_context::<Signal<Option<()>>>();
    let mut unread_ids = use_context::<Signal<std::collections::HashSet<String>>>();

    // Auto-request push permission 2 s after first open, if not yet granted.
    // Read push support and permission state at runtime (not compile-time),
    // because XMES_PUSH_WORKER_URL is set by WASM and checked inside the JS functions.
    let push_supported = js_sys::eval("'Notification' in window && 'PushManager' in window")
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let initial_perm = js_sys::eval("typeof Notification!=='undefined'?Notification.permission:'granted'")
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "granted".into());
    let needs_push = push_supported && initial_perm == "default";

    use_effect(move || {
        if needs_push {
            let _ = js_sys::eval(
                "window.xmesEnablePushOnNextTap&&window.xmesEnablePushOnNextTap()"
            );
        }
    });

    rsx! {
        div { class: "app-shell",

            // ── Header ──────────────────────────────────────────
            header { class: "app-header",
                div { class: "app-logo",
                    img { class: "app-logo-mark", src: LOGO, alt: "xmes logo" }
                    span { class: "app-logo-name", "xmes" }
                }
            }

            // ── Search ──────────────────────────────────────────
            div { class: "search-wrap",
                div { class: "search-field",
                    span { class: "search-icon",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "16", height: "16",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            circle { cx: "11", cy: "11", r: "8" }
                            path { d: "m21 21-4.35-4.35" }
                        }
                    }
                    input {
                        class: "search-input",
                        r#type: "text",
                        placeholder: "Search conversations…",
                    }
                }
            }

            // ── Conversation list ────────────────────────────────
            div { class: "section-label", "Conversations" }

            div { class: "convo-list",
                match conversations.read().as_ref() {

                    None => rsx! {
                        div { class: "spinner-wrap",
                            div { class: "spinner" }
                            span { class: "spinner-label", "Connecting…" }
                        }
                    },

                    Some(convos) if convos.is_empty() => rsx! {
                        div { class: "empty-state",
                            div { class: "empty-icon-wrap",
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg",
                                    width: "26", height: "26",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "1.8",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                                }
                            }
                            p { class: "empty-title", "No conversations yet" }
                            p { class: "empty-sub",
                                "Tap the + button below to start your first encrypted conversation."
                            }
                        }
                    },

                    Some(convos) => rsx! {
                        for summary in convos.clone() {
                            {
                                let has_unread = unread_ids.read().contains(&summary.id);
                                rsx! {
                                    conversation::Convo {
                                        summary: summary.clone(),
                                        has_unread,
                                        on_open: move |s: ConversationSummary| {
                                            // Clear unread when opening
                                            unread_ids.write().remove(&s.id);
                                            let mut a = anim; a.set("slide-in-right");
                                            let mut v = view; v.set(View::Chat(s));
                                        },
                                    }
                                }
                            }
                        }
                    },
                }
            }
        }

        // ── FAB ─────────────────────────────────────────────────
        button {
            class: "fab",
            title: "New conversation",
            disabled: !identity_ready(),
            onclick: move |_| {
                if let Some(h) = xmtp.read().as_ref() {
                    let mut po = pending_open; po.set(Some(()));
                    h.request_create_group();
                }
            },
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                width: "22", height: "22",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2.2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                path { d: "M12 5v14" }
                path { d: "M5 12h14" }
            }
        }
    }
}
