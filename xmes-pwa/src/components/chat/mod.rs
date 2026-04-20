use dioxus::prelude::*;
use js_sys::Date;
use xmes_xmtp_wasm::{ConversationSummary, IdentityInfo, MessageInfo, XmtpHandle};
use crate::View;

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

fn format_time_ns(ns: i64) -> String {
    let ms = (ns / 1_000_000) as f64;
    let d = Date::new(&wasm_bindgen::JsValue::from_f64(ms));
    format!("{:02}:{:02}", d.get_hours(), d.get_minutes())
}

#[component]
pub fn Chat(conversation: ConversationSummary) -> Element {
    let mut text_input  = use_signal(|| String::new());
    let view            = use_context::<Signal<View>>();
    let anim            = use_context::<Signal<&'static str>>();
    let xmtp            = use_context::<Signal<Option<XmtpHandle>>>();
    let messages        = use_context::<Signal<Vec<MessageInfo>>>();
    let identity_info   = use_context::<Signal<Option<IdentityInfo>>>();

    let conv_id     = conversation.id.clone();
    let own_inbox   = identity_info.read().as_ref().map(|i| i.inbox_id.clone()).unwrap_or_default();
    let av          = av_class(&conversation.name);
    let av_text     = initials(&conversation.name);

    // Fetch messages on mount (re-runs if xmtp signal changes, i.e. when worker is ready)
    use_effect(move || {
        if let Some(h) = xmtp.read().as_ref() {
            h.request_list_messages(&conv_id);
        }
    });

    // Auto-scroll to bottom whenever messages change
    use_effect(move || {
        let _ = messages.read();
        if let Some(window) = web_sys::window() {
            if let Some(doc) = window.document() {
                if let Some(el) = doc.query_selector(".chat-messages").ok().flatten() {
                    el.set_scroll_top(el.scroll_height());
                }
            }
        }
    });

    rsx! {
        div { class: "app-shell chat-shell",

            // ── Header ───────────────────────────────────────────
            header { class: "chat-header",
                button {
                    class: "chat-back-btn",
                    onclick: move |_| {
                        let mut a = anim; a.set("slide-in-left");
                        let mut v = view; v.set(View::Conversations);
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "20", height: "20",
                        view_box: "0 0 24 24", fill: "none",
                        stroke: "currentColor", stroke_width: "2.2",
                        stroke_linecap: "round", stroke_linejoin: "round",
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
                if messages.read().is_empty() {
                    div { class: "chat-empty",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "32", height: "32",
                            view_box: "0 0 24 24", fill: "none",
                            stroke: "currentColor", stroke_width: "1.5",
                            stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                        }
                        span { "No messages yet" }
                    }
                }
                for msg in messages.read().iter() {
                    {
                        let is_own  = msg.sender_inbox_id == own_inbox;
                        let time    = format_time_ns(msg.sent_at_ns);
                        let text    = msg.text.clone();
                        let deliv   = msg.delivered;
                        rsx! {
                            div { class: if is_own { "bubble-row own" } else { "bubble-row other" },
                                if !is_own {
                                    div { class: "bubble-avatar {av}", "{av_text}" }
                                }
                                div { class: "bubble-col",
                                    div { class: if is_own { "bubble own" } else { "bubble other" },
                                        "{text}"
                                    }
                                    div { class: "bubble-meta",
                                        span { class: "bubble-time", "{time}" }
                                        if is_own {
                                            if deliv {
                                                span { class: "bubble-sent",
                                                    svg {
                                                        xmlns: "http://www.w3.org/2000/svg",
                                                        width: "12", height: "12",
                                                        view_box: "0 0 24 24", fill: "none",
                                                        stroke: "currentColor", stroke_width: "2.8",
                                                        stroke_linecap: "round", stroke_linejoin: "round",
                                                        polyline { points: "20 6 9 17 4 12" }
                                                    }
                                                }
                                            } else {
                                                span { class: "bubble-sending", "•" }
                                            }
                                        }
                                    }
                                }
                            }
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
                    value: "{text_input}",
                    oninput: move |e| text_input.set(e.value()),
                    onkeydown: {
                        let conv_id = conversation.id.clone();
                        let own_inbox2 = own_inbox.clone();
                        move |e: Event<KeyboardData>| {
                            if e.data().code().to_string() == "Enter" {
                                let text = text_input.read().trim().to_string();
                                if text.is_empty() { return; }
                                text_input.set(String::new());
                                // Optimistic message (delivered: false)
                                let mut m = messages;
                                let mut list = m.read().clone();
                                list.push(MessageInfo {
                                    id:              format!("pending-{}", Date::now() as i64),
                                    text:            text.clone(),
                                    sender_inbox_id: own_inbox2.clone(),
                                    sent_at_ns:      (Date::now() * 1_000_000.0) as i64,
                                    delivered:       false,
                                });
                                m.set(list);
                                if let Some(h) = xmtp.read().as_ref() {
                                    h.request_send_message(&conv_id, &text);
                                }
                            }
                        }
                    },
                }
                button {
                    class: "chat-send-btn",
                    disabled: text_input.read().trim().is_empty(),
                    title: "Send",
                    onclick: {
                        let conv_id = conversation.id.clone();
                        let own_inbox3 = own_inbox.clone();
                        move |_| {
                            let text = text_input.read().trim().to_string();
                            if text.is_empty() { return; }
                            text_input.set(String::new());
                            // Optimistic message (delivered: false)
                            let mut m = messages;
                            let mut list = m.read().clone();
                            list.push(MessageInfo {
                                id:              format!("pending-{}", Date::now() as i64),
                                text:            text.clone(),
                                sender_inbox_id: own_inbox3.clone(),
                                sent_at_ns:      (Date::now() * 1_000_000.0) as i64,
                                delivered:       false,
                            });
                            m.set(list);
                            if let Some(h) = xmtp.read().as_ref() {
                                h.request_send_message(&conv_id, &text);
                            }
                        }
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "18", height: "18",
                        view_box: "0 0 24 24", fill: "none",
                        stroke: "currentColor", stroke_width: "2.2",
                        stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M22 2 11 13" }
                        path { d: "M22 2 15 22 11 13 2 9l20-7z" }
                    }
                }
            }
        }
    }
}
