use dioxus::prelude::*;
use xmes_xmtp_wasm::ConversationSummary;
use crate::View;

/// Placeholder message for design preview.
/// Will be replaced by real message data once send/receive is implemented.
#[derive(Clone)]
struct DemoMsg {
    text: &'static str,
    is_own: bool,
    time: &'static str,
}

const DEMO: &[DemoMsg] = &[
    DemoMsg { text: "Hey! 👋 Glad this works.",           is_own: false, time: "10:41" },
    DemoMsg { text: "Same here — fully encrypted too.",   is_own: true,  time: "10:42" },
    DemoMsg { text: "Powered by XMTP. Pretty cool.",      is_own: false, time: "10:42" },
    DemoMsg { text: "And open-source. Built with Rust 🦀", is_own: true, time: "10:43" },
];

fn av_class(name: &str) -> &'static str {
    let idx = name.bytes().fold(0usize, |a, b| a.wrapping_add(b as usize)) % 8;
    match idx {
        0 => "av-0", 1 => "av-1", 2 => "av-2", 3 => "av-3",
        4 => "av-4", 5 => "av-5", 6 => "av-6", _ => "av-7",
    }
}

fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().filter(|w| !w.is_empty()).collect();
    match words.as_slice() {
        [] => "?".into(),
        [w] => w.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or("?".into()),
        [first, .., last] => format!(
            "{}{}",
            first.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
            last.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
        ),
    }
}

#[component]
pub fn Chat(conversation: ConversationSummary) -> Element {
    let mut message = use_signal(|| String::new());
    let view = use_context::<Signal<View>>();
    let av = av_class(&conversation.name);
    let av_text = initials(&conversation.name);

    rsx! {
        div { class: "app-shell chat-shell",

            // ── Header ──────────────────────────────────────────
            header { class: "chat-header",
                button {
                    class: "chat-back-btn",
                    onclick: move |_| { let mut v = view; v.set(View::Conversations); },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "20", height: "20",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2.2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M19 12H5" }
                        path { d: "M12 19l-7-7 7-7" }
                    }
                }
                div { class: "chat-header-avatar {av}", "{av_text}" }
                div { class: "chat-header-info",
                    span { class: "chat-header-name", "{conversation.name}" }
                    span { class: "chat-header-sub", "Group · XMTP" }
                }
            }

            // ── Messages ─────────────────────────────────────────
            div { class: "chat-messages",
                // Demo messages (placeholder until real messaging is implemented)
                for msg in DEMO {
                    div { class: if msg.is_own { "bubble-row own" } else { "bubble-row other" },
                        if !msg.is_own {
                            div { class: "bubble-avatar {av}", "{av_text}" }
                        }
                        div { class: "bubble-col",
                            div { class: if msg.is_own { "bubble own" } else { "bubble other" },
                                "{msg.text}"
                            }
                            span { class: "bubble-time", "{msg.time}" }
                        }
                    }
                }
            }

            // ── Input bar ────────────────────────────────────────
            div { class: "chat-input-bar",
                input {
                    class: "chat-input",
                    r#type: "text",
                    placeholder: "Message…",
                    value: "{message}",
                    oninput: move |e| message.set(e.value()),
                }
                button {
                    class: "chat-send-btn",
                    disabled: message.read().trim().is_empty(),
                    title: "Send",
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "18", height: "18",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2.2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M22 2 11 13" }
                        path { d: "M22 2 15 22 11 13 2 9l20-7z" }
                    }
                }
            }
        }
    }
}
