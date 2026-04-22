use dioxus::prelude::*;
use js_sys::Date;
use xmes_xmtp_wasm::{ConversationSummary, IdentityInfo, MessageInfo, XmtpHandle};
use crate::View;
use crate::components::add_members::AddMembersSheet;

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

fn short_addr(s: &str) -> String {
    if s.len() <= 13 { s.to_string() }
    else { format!("{}…{}", &s[..6], &s[s.len()-4..]) }
}

fn copy_to_clipboard(text: String, mut copied: Signal<bool>) {
    let _ = js_sys::eval(&format!(
        "navigator.clipboard.writeText('{}')",
        text.replace('\'', "\\'")
    ));
    copied.set(true);
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(1500).await;
        copied.set(false);
    });
}

#[component]
fn CopyBtn(text: String) -> Element {
    let mut copied = use_signal(|| false);
    rsx! {
        button {
            class: "copy-btn",
            title: if copied() { "Copied!" } else { "Copy" },
            onclick: move |e| { e.stop_propagation(); copy_to_clipboard(text.clone(), copied); },
            onpointerdown: move |e| { e.stop_propagation(); },
            if copied() {
                svg {
                    xmlns: "http://www.w3.org/2000/svg", width: "13", height: "13",
                    view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                    stroke_width: "2.8", stroke_linecap: "round", stroke_linejoin: "round",
                    polyline { points: "20 6 9 17 4 12" }
                }
            } else {
                svg {
                    xmlns: "http://www.w3.org/2000/svg", width: "13", height: "13",
                    view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                    stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                    rect { x: "9", y: "9", width: "13", height: "13", rx: "2", ry: "2" }
                    path { d: "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" }
                }
            }
        }
    }
}

/// Bottom sheet showing all group members and an "Add Member" option.
#[component]
fn ChatMembersSheet(
    conversation_id: String,
    members: Vec<String>,
    xmtp: Signal<Option<XmtpHandle>>,
    on_close: EventHandler<()>,
    #[props(default = false)]
    start_adding: bool,
) -> Element {
    let mut show_add    = use_signal(move || start_adding);
    let mut add_input   = use_signal(|| String::new());
    let member_label    = if members.len() == 1 { "1 Member".to_string() }
                          else { format!("{} Members", members.len()) };

    // Focus the add-member input whenever it becomes visible
    use_effect(move || {
        if show_add() {
            if let Some(win) = web_sys::window() {
                if let Some(doc) = win.document() {
                    if let Ok(Some(el)) = doc.query_selector(".add-member-input") {
                        let html_el: Option<web_sys::HtmlElement> = wasm_bindgen::JsCast::dyn_into(el).ok();
                        if let Some(e) = html_el { let _ = e.focus(); }
                    }
                }
            }
        }
    });

    rsx! {
        div {
            class: "sheet-backdrop",
            onclick: move |_| on_close.call(()),
        }
        div { class: "identity-sheet",
            // title row
            div { class: "sheet-header",
                span { class: "sheet-title", "{member_label}" }
                button {
                    class: "sheet-close-btn",
                    onclick: move |_| on_close.call(()),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "20", height: "20",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        line { x1: "18", y1: "6", x2: "6", y2: "18" }
                        line { x1: "6", y1: "6", x2: "18", y2: "18" }
                    }
                }
            }

            // member list
            div { class: "sheet-addr-list",
                for addr in members.iter() {
                    {
                        let addr = addr.clone();
                        rsx! {
                            div { class: "addr-row",
                                div { class: "addr-primary-pill",
                                    span { class: "addr-primary-text", "{short_addr(&addr)}" }
                                    CopyBtn { text: addr.clone() }
                                }
                            }
                        }
                    }
                }
            }

            // Add member section
            if show_add() {
                div { class: "add-member-body",
                    input {
                        class: "add-member-input",
                        r#type: "text",
                        placeholder: "Inbox ID…",
                        autofocus: true,
                        value: "{add_input}",
                        oninput: move |e| add_input.set(e.value()),
                        onkeydown: {
                            let conv_id = conversation_id.clone();
                            move |e: Event<KeyboardData>| {
                                if e.data().code().to_string() == "Enter" {
                                    let id = add_input.read().trim().to_string();
                                    if id.is_empty() { return; }
                                    add_input.set(String::new());
                                    show_add.set(false);
                                    if let Some(h) = xmtp.peek().as_ref() {
                                        h.request_add_members(&conv_id, &[id]);
                                    }
                                }
                            }
                        },
                    }
                    button {
                        class: "add-member-btn",
                        disabled: add_input.read().trim().is_empty(),
                        onclick: {
                            let conv_id = conversation_id.clone();
                            move |_| {
                                let id = add_input.read().trim().to_string();
                                if id.is_empty() { return; }
                                add_input.set(String::new());
                                show_add.set(false);
                                if let Some(h) = xmtp.peek().as_ref() {
                                    h.request_add_members(&conv_id, &[id]);
                                }
                            }
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
                            circle { cx: "9", cy: "7", r: "4" }
                            line { x1: "19", y1: "8", x2: "19", y2: "14" }
                            line { x1: "22", y1: "11", x2: "16", y2: "11" }
                        }
                        "Add"
                    }
                }
            }

            // FAB to show add-member input
            if !show_add() {
                div { class: "sheet-fab-row",
                    button {
                        class: "sheet-fab",
                        title: "Add member",
                        onclick: move |_| show_add.set(true),
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "22", height: "22",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
                            circle { cx: "9", cy: "7", r: "4" }
                            line { x1: "19", y1: "8", x2: "19", y2: "14" }
                            line { x1: "22", y1: "11", x2: "16", y2: "11" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn Chat(conversation: ConversationSummary) -> Element {
    let mut text_input    = use_signal(|| String::new());
    let view              = use_context::<Signal<View>>();
    let anim              = use_context::<Signal<&'static str>>();
    let xmtp              = use_context::<Signal<Option<XmtpHandle>>>();
    let messages          = use_context::<Signal<Vec<MessageInfo>>>();
    let group_members     = use_context::<Signal<Vec<String>>>();
    let identity_info     = use_context::<Signal<Option<IdentityInfo>>>();

    let mut show_members       = use_signal(|| false);
    let mut sheet_start_adding = use_signal(|| false);
    let conv_id           = conversation.id.clone();
    let own_inbox         = identity_info.read().as_ref().map(|i| i.inbox_id.clone()).unwrap_or_default();
    let av                = av_class(&conversation.name);
    let av_text           = initials(&conversation.name);

    let member_count      = group_members.read().len();
    let member_label      = if member_count == 1 { "1 Member".to_string() }
                            else { format!("{} Members", member_count) };

    // Fetch messages + members on mount
    use_effect(move || {
        if let Some(h) = xmtp.read().as_ref() {
            h.request_list_messages(&conv_id);
            h.request_list_members(&conv_id);
        }
    });

    // Periodic sync every 8 seconds while chat is open
    let conv_id_sync = conversation.id.clone();
    use_effect(move || {
        let id = conv_id_sync.clone();
        let interval = gloo_timers::callback::Interval::new(8_000, move || {
            if let Some(h) = xmtp.peek().as_ref() {
                h.request_list_messages(&id);
            }
        });
        interval.forget();
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
                div { class: "chat-header-center",
                    div { class: "chat-header-avatar {av}", "{av_text}" }
                    div { class: "chat-header-info",
                        span { class: "chat-header-name", "{conversation.name}" }
                        span { class: "chat-header-sub", "{member_label}" }
                    }
                }
                // Three-dots → members sheet
                button {
                    class: "chat-menu-btn",
                    title: "Group members",
                    onclick: move |_| {
                        sheet_start_adding.set(false);
                        show_members.set(true);
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "20", height: "20",
                        view_box: "0 0 24 24", fill: "none",
                        stroke: "currentColor", stroke_width: "2",
                        stroke_linecap: "round", stroke_linejoin: "round",
                        circle { cx: "12", cy: "5",  r: "1" }
                        circle { cx: "12", cy: "12", r: "1" }
                        circle { cx: "12", cy: "19", r: "1" }
                    }
                }
            }

            // ── Members sheet ─────────────────────────────────────
            if show_members() {
                ChatMembersSheet {
                    conversation_id: conversation.id.clone(),
                    members: group_members.read().clone(),
                    xmtp,
                    start_adding: sheet_start_adding(),
                    on_close: move |_| {
                        sheet_start_adding.set(false);
                        show_members.set(false);
                    },
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
