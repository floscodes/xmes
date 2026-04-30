use dioxus::prelude::*;
use js_sys::Date;
use xmes_xmtp_wasm::{ConversationSummary, IdentityInfo, MemberInfo, MessageInfo, XmtpHandle};
use crate::{components::qr::QrScannerSheet, View};

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


/// Load pending-push-exclusion list for a conversation from localStorage.
fn pending_load(conv_id: &str) -> Vec<String> {
    let key = conv_id.replace('\'', "");
    let now_days = js_sys::Date::now() as u64 / 86_400_000;
    let raw = js_sys::eval(&format!("localStorage.getItem('pending_push_{key}')||''"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    // Format: "inbox_id:day_added,inbox_id:day_added,..."
    raw.split(',')
        .filter_map(|entry| {
            let mut parts = entry.splitn(2, ':');
            let id  = parts.next()?.trim().to_string();
            let day: u64 = parts.next().and_then(|d| d.trim().parse().ok()).unwrap_or(0);
            if id.is_empty() || now_days.saturating_sub(day) > 7 { None } else { Some(id) }
        })
        .collect()
}

/// Persist the pending list to localStorage.
fn pending_save(conv_id: &str, members: &[String]) {
    let key  = conv_id.replace('\'', "");
    let day  = (js_sys::Date::now() as u64 / 86_400_000).to_string();
    let data = members.iter()
        .map(|m| format!("{}:{day}", m.replace(['\'', ',', ':'], "")))
        .collect::<Vec<_>>()
        .join(",");
    let _ = js_sys::eval(&format!("localStorage.setItem('pending_push_{key}','{data}')"));
}

/// Send an invitation push to a newly added member.
fn notify_push_invite(new_member_inbox_id: &str, group_name: &str) {
    let id   = new_member_inbox_id.replace('"', "");
    let name = group_name.replace('"', "").replace('\\', "");
    let _ = js_sys::eval(&format!(
        r#"(function(){{var u=window.XMES_PUSH_WORKER_URL;if(!u)return;fetch(u+"/notify",{{method:"POST",headers:{{"content-type":"application/json"}},body:JSON.stringify({{member_inbox_ids:["{id}"],sender_inbox_id:"",group_name:"{name}",title:"Group welcome",body:"You have been added to group {name}"}})}}).catch(()=>{{}})}})()"#,
        id=id, name=name
    ));
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
    let copied = use_signal(|| false);
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

fn role_label(role: u8) -> &'static str {
    match role {
        2 => "Super Admin",
        1 => "Admin",
        _ => "Member",
    }
}

fn role_class(role: u8) -> &'static str {
    match role {
        2 => "role-badge role-superadmin",
        1 => "role-badge role-admin",
        _ => "role-badge role-member",
    }
}

/// Bottom sheet showing all group members, with sticky header (name + rename) and sticky add-member footer.
#[component]
fn ChatGroupSettingsSheet(
    conversation_id: String,
    conv_name: Signal<String>,
    members: Vec<MemberInfo>,
    own_inbox_id: String,
    xmtp: Signal<Option<XmtpHandle>>,
    pending_members: Signal<Vec<String>>,
    on_close: EventHandler<()>,
) -> Element {
    let mut add_input:    Signal<String>        = use_signal(|| String::new());
    let mut menu_open:    Signal<Option<String>> = use_signal(|| None);
    let mut show_rename:  Signal<bool>           = use_signal(|| false);
    let mut rename_input: Signal<String>         = use_signal(move || conv_name.peek().clone());
    let mut show_scanner: Signal<bool>           = use_signal(|| false);

    let own_role = members.iter()
        .find(|m| m.inbox_id == own_inbox_id)
        .map(|m| m.role)
        .unwrap_or(0);

    let can_rename = own_role >= 1;

    rsx! {
        div {
            class: "sheet-backdrop",
            onclick: move |_| on_close.call(()),
        }
        div { class: "identity-sheet members-sheet",

            // ── Sticky header: conversation name + optional rename ──
            div { class: "members-sheet-header",
                if show_rename() {
                    // Inline rename input
                    input {
                        class: "members-rename-input",
                        r#type: "text",
                        value: "{rename_input}",
                        oninput: move |e| rename_input.set(e.value()),
                        onkeydown: {
                            let conv_id = conversation_id.clone();
                            move |e: Event<KeyboardData>| {
                                if e.data().code().to_string() == "Enter" {
                                    let name = rename_input.read().trim().to_string();
                                    if !name.is_empty() {
                                        conv_name.set(name.clone());
                                        if let Some(h) = xmtp.peek().as_ref() {
                                            h.request_update_group_name(&conv_id, &name);
                                        }
                                    }
                                    show_rename.set(false);
                                }
                                if e.data().code().to_string() == "Escape" {
                                    rename_input.set(conv_name.peek().clone());
                                    show_rename.set(false);
                                }
                            }
                        },
                    }
                    button {
                        class: "members-rename-confirm",
                        disabled: rename_input.read().trim().is_empty(),
                        onclick: {
                            let conv_id = conversation_id.clone();
                            move |_| {
                                let name = rename_input.read().trim().to_string();
                                if !name.is_empty() {
                                    conv_name.set(name.clone());
                                    if let Some(h) = xmtp.peek().as_ref() {
                                        h.request_update_group_name(&conv_id, &name);
                                    }
                                }
                                show_rename.set(false);
                            }
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2.8", stroke_linecap: "round", stroke_linejoin: "round",
                            polyline { points: "20 6 9 17 4 12" }
                        }
                    }
                } else {
                    // Name display row
                    div { class: "members-sheet-name-row",
                        if can_rename {
                            button {
                                class: "member-menu-btn",
                                title: "Rename group",
                                onclick: move |_| show_rename.set(true),
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                                    view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                                    stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                                    circle { cx: "12", cy: "5",  r: "1" }
                                    circle { cx: "12", cy: "12", r: "1" }
                                    circle { cx: "12", cy: "19", r: "1" }
                                }
                            }
                        }
                        span { class: "members-sheet-title", "{conv_name()}" }
                    }
                }
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

            // ── Scrollable member list ─────────────────────────────
            div { class: "sheet-addr-list members-sheet-list",
                for m in members.iter() {
                    {
                        let m = m.clone();
                        let is_menu_open = menu_open.read().as_deref() == Some(&m.inbox_id);
                        let show_menu_btn = own_role >= 1 && m.role < 2;
                        let conv_id = conversation_id.clone();
                        let iid = m.inbox_id.clone();
                        rsx! {
                            div { class: "addr-row member-row",
                                div { class: "addr-primary-pill",
                                    span { class: "addr-primary-text", "{short_addr(&m.address)}" }
                                    CopyBtn { text: m.address.clone() }
                                }
                                span { class: "{role_class(m.role)}", "{role_label(m.role)}" }
                                if show_menu_btn {
                                    div { class: "member-menu-wrap",
                                        button {
                                            class: "member-menu-btn",
                                            title: "Manage member",
                                            onclick: move |e| {
                                                e.stop_propagation();
                                                if is_menu_open {
                                                    menu_open.set(None);
                                                } else {
                                                    menu_open.set(Some(iid.clone()));
                                                }
                                            },
                                            svg {
                                                xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                                                view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                                                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                                                circle { cx: "12", cy: "5",  r: "1" }
                                                circle { cx: "12", cy: "12", r: "1" }
                                                circle { cx: "12", cy: "19", r: "1" }
                                            }
                                        }
                                        if is_menu_open {
                                            div {
                                                class: "member-dropdown-overlay",
                                                onclick: move |_| menu_open.set(None),
                                            }
                                            div { class: "member-dropdown",
                                                onclick: move |e| e.stop_propagation(),
                                                if own_role == 2 {
                                                    if m.role == 0 {
                                                        button {
                                                            class: "member-dropdown-item",
                                                            onclick: {
                                                                let cid = conv_id.clone();
                                                                let mid = m.inbox_id.clone();
                                                                move |_| {
                                                                    menu_open.set(None);
                                                                    if let Some(h) = xmtp.peek().as_ref() {
                                                                        h.request_set_admin(&cid, &mid, true);
                                                                    }
                                                                }
                                                            },
                                                            "Make Admin"
                                                        }
                                                        button {
                                                            class: "member-dropdown-item",
                                                            onclick: {
                                                                let cid = conv_id.clone();
                                                                let mid = m.inbox_id.clone();
                                                                move |_| {
                                                                    menu_open.set(None);
                                                                    if let Some(h) = xmtp.peek().as_ref() {
                                                                        h.request_set_super_admin(&cid, &mid, true);
                                                                    }
                                                                }
                                                            },
                                                            "Make Super Admin"
                                                        }
                                                    }
                                                    if m.role == 1 {
                                                        button {
                                                            class: "member-dropdown-item",
                                                            onclick: {
                                                                let cid = conv_id.clone();
                                                                let mid = m.inbox_id.clone();
                                                                move |_| {
                                                                    menu_open.set(None);
                                                                    if let Some(h) = xmtp.peek().as_ref() {
                                                                        h.request_set_admin(&cid, &mid, false);
                                                                    }
                                                                }
                                                            },
                                                            "Remove Admin"
                                                        }
                                                        button {
                                                            class: "member-dropdown-item",
                                                            onclick: {
                                                                let cid = conv_id.clone();
                                                                let mid = m.inbox_id.clone();
                                                                move |_| {
                                                                    menu_open.set(None);
                                                                    if let Some(h) = xmtp.peek().as_ref() {
                                                                        h.request_set_super_admin(&cid, &mid, true);
                                                                    }
                                                                }
                                                            },
                                                            "Make Super Admin"
                                                        }
                                                    }
                                                }
                                                button {
                                                    class: "member-dropdown-item member-dropdown-danger",
                                                    onclick: {
                                                        let cid = conv_id.clone();
                                                        let mid = m.inbox_id.clone();
                                                        move |_| {
                                                            menu_open.set(None);
                                                            if let Some(h) = xmtp.peek().as_ref() {
                                                                h.request_remove_member(&cid, &mid);
                                                            }
                                                        }
                                                    },
                                                    "Remove from group"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Sticky footer: add member ──────────────────────────
            div { class: "members-sheet-footer",
                div { class: "add-member-input-row",
                    input {
                        class: "add-member-input",
                        r#type: "text",
                        placeholder: "Address / Inbox ID…",
                        value: "{add_input}",
                        oninput: move |e| add_input.set(e.value()),
                        onkeydown: {
                            let conv_id   = conversation_id.clone();
                            let conv_name = conv_name.clone();
                            move |e: Event<KeyboardData>| {
                                if e.data().code().to_string() == "Enter" {
                                    let id = add_input.read().trim().to_string();
                                    if id.is_empty() { return; }
                                    add_input.set(String::new());
                                    notify_push_invite(&id, &conv_name.peek());
                                    pending_members.write().push(id.clone());
                                    pending_save(&conv_id, &pending_members.read());
                                    if let Some(h) = xmtp.peek().as_ref() {
                                        h.request_add_members(&conv_id, &[id]);
                                    }
                                    on_close.call(());
                                }
                            }
                        },
                    }
                    button {
                        class: "qr-scan-btn",
                        title: "Scan QR code",
                        onclick: move |_| show_scanner.set(true),
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "18", height: "18",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M11 3H5a2 2 0 0 0-2 2v6" }
                            path { d: "M13 21h6a2 2 0 0 0 2-2v-6" }
                            path { d: "M3 13v6a2 2 0 0 0 2 2h6" }
                            path { d: "M21 11V5a2 2 0 0 0-2-2h-6" }
                            rect { x: "7", y: "7", width: "4", height: "4" }
                            rect { x: "13", y: "7", width: "4", height: "4" }
                            rect { x: "7", y: "13", width: "4", height: "4" }
                        }
                    }
                }
                button {
                    class: "add-member-btn",
                    disabled: add_input.read().trim().is_empty(),
                    onclick: {
                        let conv_id   = conversation_id.clone();
                        let conv_name = conv_name.clone();
                        move |_| {
                            let id = add_input.read().trim().to_string();
                            if id.is_empty() { return; }
                            add_input.set(String::new());
                            notify_push_invite(&id, &conv_name.peek());
                            pending_members.write().push(id.clone());
                            pending_save(&conv_id, &pending_members.read());
                            if let Some(h) = xmtp.peek().as_ref() {
                                h.request_add_members(&conv_id, &[id]);
                            }
                            on_close.call(());
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

        if show_scanner() {
            QrScannerSheet {
                conversation_id: conversation_id.clone(),
                xmtp,
                on_close: move |_| {
                    show_scanner.set(false);
                    on_close.call(());
                },
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
    let mut messages      = use_context::<Signal<Vec<MessageInfo>>>();
    let group_members     = use_context::<Signal<Vec<MemberInfo>>>();
    let identity_info     = use_context::<Signal<Option<IdentityInfo>>>();

    let mut unread_ids         = use_context::<Signal<std::collections::HashSet<String>>>();
    let mut initial_scroll_done = use_signal(|| false);
    let mut user_scrolled_up   = use_signal(|| false);
    let mut loading            = use_signal(|| true);
    // Members excluded from message push until they send their first message
    // (proof they have synced the group welcome). Persisted in localStorage.
    let conv_id_for_pending = conversation.id.clone();
    let mut pending_members: Signal<Vec<String>> = use_signal(move || {
        pending_load(&conv_id_for_pending)
    });
    let mut show_members = use_signal(|| false);
    let mut conv_name     = use_signal(|| conversation.name.clone());
    let conv_id           = conversation.id.clone();
    let own_inbox         = identity_info.read().as_ref().map(|i| i.inbox_id.clone()).unwrap_or_default();

    let member_count      = group_members.read().len();
    let member_label      = if member_count == 1 { "1 Member".to_string() }
                            else { format!("{} Members", member_count) };

    // When a pending member sends a message they have synced the group — remove from pending.
    let conv_id_pending = conversation.id.clone();
    use_effect(move || {
        let msgs = messages.read();
        let mut pending = pending_members.write();
        let before = pending.len();
        pending.retain(|id| !msgs.iter().any(|m| &m.sender_inbox_id == id));
        if pending.len() != before {
            pending_save(&conv_id_pending, &pending);
        }
    });

    // Clear stale messages immediately, then fetch fresh ones
    let conv_id_unread = conversation.id.clone();
    use_effect(move || {
        messages.set(vec![]);
        unread_ids.write().remove(&conv_id_unread);
        if let Some(h) = xmtp.read().as_ref() {
            h.request_list_messages(&conv_id);
            h.request_list_members(&conv_id);
        }
    });

    // Periodic sync every 8 seconds while chat is open.
    // The callback checks the current view so it becomes a no-op after navigation.
    let conv_id_sync = conversation.id.clone();
    use_effect(move || {
        let id = conv_id_sync.clone();
        let interval = gloo_timers::callback::Interval::new(8_000, move || {
            let still_open = matches!(view.peek().clone(), View::Chat(c) if c.id == id);
            if still_open {
                if let Some(h) = xmtp.peek().as_ref() {
                    h.request_list_messages(&id);
                }
            }
        });
        interval.forget();
    });

    // Auto-scroll to bottom on first load; afterwards only when user hasn't scrolled up.
    // Uses peek() for user_scrolled_up so the effect doesn't re-run on scroll events.
    use_effect(move || {
        let _ = messages.read();
        loading.set(false);
        let is_initial = !*initial_scroll_done.peek();
        let scrolled_up = *user_scrolled_up.peek();
        if is_initial || !scrolled_up {
            if let Some(window) = web_sys::window() {
                if let Some(doc) = window.document() {
                    if let Some(el) = doc.query_selector(".chat-messages").ok().flatten() {
                        el.set_scroll_top(el.scroll_height());
                        initial_scroll_done.set(true);
                    }
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
                    div { class: "chat-header-avatar {av_class(&conv_name())}", "{initials(&conv_name())}" }
                    div { class: "chat-header-info",
                        span { class: "chat-header-name", "{conv_name()}" }
                        span { class: "chat-header-sub", "{member_label}" }
                    }
                }
                // Three-dots → members sheet
                button {
                    class: "chat-menu-btn",
                    title: "Group members",
                    onclick: move |_| { show_members.set(true); },
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
                ChatGroupSettingsSheet {
                    conversation_id: conversation.id.clone(),
                    conv_name,
                    members: group_members.read().clone(),
                    own_inbox_id: own_inbox.clone(),
                    xmtp,
                    pending_members,
                    on_close: move |_| show_members.set(false),
                }
            }

            // ── Messages ─────────────────────────────────────────
            div {
                class: "chat-messages",
                onscroll: move |_| {
                    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                        if let Some(el) = doc.query_selector(".chat-messages").ok().flatten() {
                            let distance = el.scroll_height() - el.scroll_top() - el.client_height();
                            user_scrolled_up.set(distance > 80);
                        }
                    }
                },
                if loading() {
                    div { class: "chat-loading",
                        div { class: "chat-spinner" }
                    }
                } else if messages.read().is_empty() {
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
                        let system_text = msg.system_text.clone();
                        let is_own  = msg.sender_inbox_id == own_inbox;
                        let time    = format_time_ns(msg.sent_at_ns);
                        let text    = msg.text.clone();
                        let deliv   = msg.delivered;
                        let sender_addr = if !is_own {
                            group_members.read().iter()
                                .find(|m| m.inbox_id == msg.sender_inbox_id)
                                .map(|m| short_addr(&m.address))
                        } else { None };
                        rsx! {
                            if let Some(ref st) = system_text {
                                div { class: "system-event", "{st}" }
                            } else {
                            div { class: if is_own { "bubble-row own" } else { "bubble-row other" },
                                if !is_own {
                                    div { class: "bubble-avatar {av_class(&conv_name())}", "{initials(&conv_name())}" }
                                }
                                div { class: "bubble-col",
                                    if let Some(ref addr) = sender_addr {
                                        span { class: "bubble-sender", "{addr}" }
                                    }
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
                        let conv_id   = conversation.id.clone();
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
                                    system_text:     None,
                                    sender_inbox_id: own_inbox2.clone(),
                                    sent_at_ns:      (Date::now() * 1_000_000.0) as i64,
                                    delivered:       false,
                                });
                                m.set(list);
                                if let Some(h) = xmtp.read().as_ref() {
                                    h.request_send_message(&conv_id, &text);
                                    let _ = js_sys::eval("window.__xmes_push_pending = (window.__xmes_push_pending || 0) + 1");
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
                        let conv_id    = conversation.id.clone();
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
                                system_text:     None,
                            });
                            m.set(list);
                            if let Some(h) = xmtp.read().as_ref() {
                                h.request_send_message(&conv_id, &text);
                                let _ = js_sys::eval("window.__xmes_push_pending = (window.__xmes_push_pending || 0) + 1");
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
