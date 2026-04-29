use dioxus::prelude::*;

const LOGO: Asset = asset!("/assets/icons/xmes-icon.svg");
use xmes_xmtp_wasm::{ConversationSummary, XmtpHandle};
use crate::View;

mod conversation;

const PTR_THRESHOLD: f64 = 64.0;
const PTR_MAX:       f64 = 80.0;

#[component]
pub fn Conversations() -> Element {
    let xmtp = use_context::<Signal<Option<XmtpHandle>>>();
    let conversations = use_context::<Signal<Option<Vec<ConversationSummary>>>>();
    let identity_ready = use_context::<Signal<bool>>();
    let view = use_context::<Signal<View>>();
    let anim = use_context::<Signal<&'static str>>();
    let pending_open = use_context::<Signal<Option<()>>>();
    let mut unread_ids = use_context::<Signal<std::collections::HashSet<String>>>();

    // Pull-to-refresh
    let mut ptr_offset   = use_signal(|| 0.0f64);
    let mut ptr_start_y  = use_signal(|| 0.0f64);
    let mut ptr_dragging = use_signal(|| false);
    let mut refreshing   = use_signal(|| false);

    // When refreshing flips to true, clear it after 1.5 s
    use_effect(move || {
        if *refreshing.read() {
            dioxus::prelude::spawn(async move {
                gloo_timers::future::TimeoutFuture::new(1_500).await;
                refreshing.set(false);
                ptr_offset.set(0.0);
            });
        }
    });

    // Auto-request push permission 2 s after first open, if not yet granted.
    // Read push support and permission state at runtime (not compile-time),
    // because XMES_PUSH_WORKER_URL is set by WASM and checked inside the JS functions.

    let ptr_h = if *refreshing.read() { PTR_MAX } else { *ptr_offset.read() };
    let spinner_style = if *refreshing.read() {
        "animation: spin 0.7s linear infinite;"
    } else {
        ""
    };
    let ptr_indicator_style = format!(
        "height: {}px; opacity: {}; overflow: hidden; display: flex; align-items: center; justify-content: center; transition: {};",
        ptr_h,
        if ptr_h > 8.0 { 1.0 } else { 0.0 },
        if *ptr_dragging.read() { "none" } else { "height 0.25s ease, opacity 0.25s ease" },
    );

    rsx! {
        div {
            class: "app-shell",
            // Pointer handlers for pull-to-refresh on the whole shell
            onpointerdown: move |e| {
                let at_top = js_sys::eval(
                    "(document.querySelector('.convo-list')?.scrollTop??1)===0"
                ).ok().and_then(|v| v.as_bool()).unwrap_or(true);
                if at_top {
                    ptr_start_y.set(e.client_coordinates().y);
                    ptr_dragging.set(true);
                }
            },
            onpointermove: move |e| {
                if !*ptr_dragging.read() { return; }
                let dy = (e.client_coordinates().y - ptr_start_y()).max(0.0).min(PTR_MAX);
                ptr_offset.set(dy);
            },
            onpointerup: move |_| {
                ptr_dragging.set(false);
                let pulled = *ptr_offset.read();
                if pulled >= PTR_THRESHOLD {
                    refreshing.set(true);
                    if let Some(h) = xmtp.read().as_ref() {
                        h.request_list();
                    }
                } else {
                    ptr_offset.set(0.0);
                }
            },
            onpointercancel: move |_| {
                ptr_dragging.set(false);
                ptr_offset.set(0.0);
            },

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

            // ── Pull-to-refresh indicator ────────────────────────
            div { style: "{ptr_indicator_style}",
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "22", height: "22",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "var(--color-primary)",
                    stroke_width: "2.2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    style: "{spinner_style}",
                    path { d: "M21 12a9 9 0 1 1-6.219-8.56" }
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
