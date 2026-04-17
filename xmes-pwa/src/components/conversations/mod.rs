use dioxus::prelude::*;
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

    rsx! {
        div { class: "app-shell",

            // ── Header ──────────────────────────────────────────
            header { class: "app-header",
                div { class: "app-logo",
                    div { class: "app-logo-mark",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "100%", height: "100%",
                            view_box: "0 0 176 170",
                            fill: "none",
                            // Envelope body — slightly transparent white
                            path {
                                d: "M175,23L0,24L1,170L175,168L175,23L86,107",
                                stroke: "rgba(255,255,255,0.55)",
                                stroke_width: "18",
                                stroke_linejoin: "round",
                                stroke_linecap: "round",
                            }
                            // X-fold / outer envelope shape — full white
                            path {
                                d: "M2,170L1,26L74,96L86,106L175,20L176,168L4,170L176,0",
                                stroke: "white",
                                stroke_width: "18",
                                stroke_linejoin: "round",
                                stroke_linecap: "round",
                            }
                        }
                    }
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
                            conversation::Convo {
                                summary,
                                on_open: move |s: ConversationSummary| {
                                    let mut a = anim; a.set("slide-in-right");
                                    let mut v = view; v.set(View::Chat(s));
                                },
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
