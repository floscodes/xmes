use std::sync::Arc;
use dioxus::prelude::*;
use xmes_xmtp_wasm::{ConversationSummary, IdentityInfo, XmtpHandle};
use crate::{ConfirmAction, DarkMode, SidebarTab, View};
use crate::components::{add_members::AddMembersSheet, qr::ShowAddressQrSheet};

const LOGO: Asset = asset!("/assets/icons/xmes-icon.svg");

// ── helpers ──────────────────────────────────────────────────────────────────

fn avatar_class(name: &str) -> &'static str {
    let idx = name.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize)) % 8;
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

fn short(s: &str, keep: usize) -> String {
    if s.len() <= keep * 2 + 3 { s.to_string() }
    else { format!("{}…{}", &s[..keep], &s[s.len()-4..]) }
}

fn inbox_avatar(inbox_id: &str) -> &'static str {
    let idx = inbox_id.bytes().fold(0usize, |a, b| a.wrapping_add(b as usize)) % 8;
    match idx {
        0 => "av-0", 1 => "av-1", 2 => "av-2", 3 => "av-3",
        4 => "av-4", 5 => "av-5", 6 => "av-6", _ => "av-7",
    }
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

// ── CopyBtn ───────────────────────────────────────────────────────────────────

#[component]
fn CopyBtn(text: String) -> Element {
    let copied = use_signal(|| false);
    rsx! {
        button {
            class: "copy-btn",
            title: if copied() { "Copied!" } else { "Copy" },
            onclick: move |e| {
                e.stop_propagation();
                copy_to_clipboard(text.clone(), copied);
            },
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

// ── Welcome screen (right panel) ──────────────────────────────────────────────

#[component]
pub fn WelcomePanel() -> Element {
    rsx! {
        div { class: "welcome-screen",
            div { class: "welcome-icon",
                svg {
                    xmlns: "http://www.w3.org/2000/svg", width: "36", height: "36",
                    view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                    stroke_width: "1.5", stroke_linecap: "round", stroke_linejoin: "round",
                    path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                }
            }
            h2 { class: "welcome-title", "Welcome to xmes" }
            p { class: "welcome-sub",
                "Select a conversation on the left to start messaging, or create a new encrypted conversation."
            }
        }
    }
}

// ── Conversation row ──────────────────────────────────────────────────────────

#[component]
fn ConvoRow(
    summary: ConversationSummary,
    is_active: bool,
    has_unread: bool,
    on_open: EventHandler<ConversationSummary>,
) -> Element {
    let mut show_add  = use_signal(|| false);
    let confirm       = use_context::<Signal<Option<ConfirmAction>>>();
    let xmtp          = use_context::<Signal<Option<XmtpHandle>>>();
    let conversations = use_context::<Signal<Option<Vec<ConversationSummary>>>>();

    let av_class  = avatar_class(&summary.name);
    let av_text   = initials(&summary.name);
    let open_summ = summary.clone();
    let delete_id = summary.id.clone();

    let auto_accept_id = summary.id.clone();
    use_effect(move || {
        if summary.is_pending {
            if let Some(h) = xmtp.peek().as_ref() {
                h.request_accept_invitation(&auto_accept_id);
            }
        }
    });

    rsx! {
        div { class: "convo-item",
            div {
                class: if is_active { "convo-row active" } else { "convo-row" },
                onclick: move |_| on_open.call(open_summ.clone()),

                div { class: "convo-avatar-wrap",
                    div { class: "convo-avatar {av_class}", "{av_text}" }
                    if has_unread {
                        div { class: "unread-badge" }
                    }
                }
                div { class: "convo-info",
                    span {
                        class: if has_unread { "convo-name convo-name-unread" } else { "convo-name" },
                        "{summary.name}"
                    }
                    if let Some(ref sender) = summary.last_sender {
                        span { class: "convo-sub", "{short(sender, 6)}" }
                    }
                }

                // Hover action buttons
                div { class: "convo-actions",
                    button {
                        class: "convo-action-btn",
                        title: "Add member",
                        onclick: move |e| {
                            e.stop_propagation();
                            show_add.set(true);
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "15", height: "15",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
                            circle { cx: "9", cy: "7", r: "4" }
                            line { x1: "19", y1: "8", x2: "19", y2: "14" }
                            line { x1: "22", y1: "11", x2: "16", y2: "11" }
                        }
                    }
                    button {
                        class: "convo-action-btn danger",
                        title: "Leave conversation",
                        onclick: move |e| {
                            e.stop_propagation();
                            let id = delete_id.clone();
                            let mut c = confirm;
                            c.set(Some(ConfirmAction {
                                title:         "Leave conversation?".into(),
                                message:       "You will leave this group permanently.".into(),
                                confirm_label: "Leave".into(),
                                on_confirm: Arc::new(move || {
                                    let mut convos = conversations;
                                    let filtered = convos.peek().as_ref().map(|list| {
                                        list.iter().filter(|c| c.id != id).cloned().collect::<Vec<_>>()
                                    });
                                    if let Some(f) = filtered {
                                        convos.set(Some(f));
                                    }
                                    if let Some(h) = xmtp.peek().as_ref() {
                                        h.request_leave(id.clone());
                                    }
                                }),
                            }));
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "15", height: "15",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4" }
                            polyline { points: "10 17 15 12 10 7" }
                            line { x1: "15", y1: "12", x2: "3", y2: "12" }
                        }
                    }
                }
            }
        }

        if show_add() {
            AddMembersSheet {
                conversation_id: summary.id.clone(),
                conversation_name: summary.name.clone(),
                xmtp,
                on_close: move |_| show_add.set(false),
            }
        }
    }
}

// ── Conversations panel (sidebar content) ─────────────────────────────────────

#[component]
fn ConversationsPanel() -> Element {
    let xmtp          = use_context::<Signal<Option<XmtpHandle>>>();
    let conversations = use_context::<Signal<Option<Vec<ConversationSummary>>>>();
    let identity_ready = use_context::<Signal<bool>>();
    let view          = use_context::<Signal<View>>();
    let pending_open  = use_context::<Signal<Option<()>>>();
    let mut unread_ids = use_context::<Signal<std::collections::HashSet<String>>>();

    let active_id = match view.read().clone() {
        View::Chat(c) => Some(c.id),
        _ => None,
    };

    rsx! {
        div { class: "sidebar-content",
            // Search
            div { class: "sidebar-search",
                div { class: "search-field",
                    span { class: "search-icon",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "15", height: "15",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
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

            div { class: "section-label", "Conversations" }

            // List
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
                                    xmlns: "http://www.w3.org/2000/svg", width: "22", height: "22",
                                    view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                                    stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round",
                                    path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                                }
                            }
                            p { class: "empty-title", "No conversations yet" }
                            p { class: "empty-sub",
                                "Click the button below to start your first encrypted conversation."
                            }
                        }
                    },
                    Some(convos) => rsx! {
                        for summary in convos.clone() {
                            {
                                let has_unread = unread_ids.read().contains(&summary.id);
                                let is_active  = active_id.as_deref() == Some(&summary.id);
                                rsx! {
                                    ConvoRow {
                                        summary: summary.clone(),
                                        is_active,
                                        has_unread,
                                        on_open: move |s: ConversationSummary| {
                                            unread_ids.write().remove(&s.id);
                                            let mut v = view; v.set(View::Chat(s));
                                        },
                                    }
                                }
                            }
                        }
                    },
                }
            }

            // New conversation button
            div { class: "sidebar-footer",
                button {
                    class: "new-conv-btn",
                    disabled: !identity_ready(),
                    onclick: move |_| {
                        if let Some(h) = xmtp.read().as_ref() {
                            let mut po = pending_open; po.set(Some(()));
                            h.request_create_group();
                        }
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "18", height: "18",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M12 5v14" }
                        path { d: "M5 12h14" }
                    }
                    "New Conversation"
                }
            }
        }
    }
}

// ── Identity card (in sidebar panel) ─────────────────────────────────────────

#[component]
fn IdentityCard(
    idx: usize,
    info: IdentityInfo,
    is_active: bool,
    xmtp: Signal<Option<XmtpHandle>>,
    all_identities: Signal<Vec<IdentityInfo>>,
    confirm: Signal<Option<ConfirmAction>>,
    sidebar_tab: Signal<SidebarTab>,
    view: Signal<View>,
    show_phrase_for: Signal<Option<Vec<String>>>,
    show_qr_for: Signal<Option<String>>,
) -> Element {
    let av          = inbox_avatar(&info.inbox_id);
    let addr_short  = short(&info.primary_address, 8);
    let inbox_short = short(&info.inbox_id, 8);
    let words: Vec<String> = info.mnemonic.as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(str::to_string)
        .collect();

    rsx! {
        div {
            class: if is_active { "identity-card active" } else { "identity-card" },

            // Avatar
            div { class: "identity-avatar {av}",
                svg {
                    xmlns: "http://www.w3.org/2000/svg", width: "20", height: "20",
                    view_box: "0 0 24 24", fill: "none", stroke: "white",
                    stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                    rect { x: "3", y: "11", width: "18", height: "11", rx: "2", ry: "2" }
                    path { d: "M7 11V7a5 5 0 0 1 10 0v4" }
                }
            }

            // Info
            div {
                class: "identity-info",
                onclick: move |_| {
                    if let Some(h) = xmtp.read().as_ref() {
                        h.request_switch_identity(idx);
                    }
                    sidebar_tab.set(SidebarTab::Conversations);
                    view.set(View::Welcome);
                },
                div { class: "identity-row",
                    span { class: "identity-label", "Address" }
                    if is_active {
                        span { class: "identity-active-badge", "Active" }
                    }
                }
                div { class: "identity-copy-row",
                    span { class: "identity-address", "{addr_short}" }
                    CopyBtn { text: info.primary_address.clone() }
                    {
                        let addr = info.primary_address.clone();
                        rsx! {
                            button {
                                class: "copy-btn",
                                title: "Show QR code",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    show_qr_for.set(Some(addr.clone()));
                                },
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg", width: "13", height: "13",
                                    view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                                    stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                                    rect { x: "3", y: "3", width: "7", height: "7" }
                                    rect { x: "14", y: "3", width: "7", height: "7" }
                                    rect { x: "3", y: "14", width: "7", height: "7" }
                                    rect { x: "14", y: "14", width: "3", height: "3" }
                                    rect { x: "18", y: "14", width: "3", height: "3" }
                                    rect { x: "14", y: "18", width: "3", height: "3" }
                                    rect { x: "18", y: "18", width: "3", height: "3" }
                                }
                            }
                        }
                    }
                }
                div { class: "identity-addr-section",
                    span { class: "identity-addr-label", "Inbox ID" }
                    div { class: "identity-copy-row",
                        span { class: "identity-inbox", "{inbox_short}" }
                        CopyBtn { text: info.inbox_id.clone() }
                    }
                }
                button {
                    class: "show-phrase-btn",
                    onclick: move |e| {
                        e.stop_propagation();
                        show_phrase_for.set(Some(words.clone()));
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "11", height: "11",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        rect { x: "3", y: "11", width: "18", height: "11", rx: "2", ry: "2" }
                        path { d: "M7 11V7a5 5 0 0 1 10 0v4" }
                    }
                    span { "Recovery phrase / export" }
                }
            }

            // Remove button (visible on hover)
            div { class: "identity-actions",
                if is_active {
                    div { class: "identity-check",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2.5", stroke_linecap: "round", stroke_linejoin: "round",
                            polyline { points: "20 6 9 17 4 12" }
                        }
                    }
                }
                button {
                    class: "identity-remove-btn",
                    title: "Remove identity from this device",
                    onclick: move |e| {
                        e.stop_propagation();
                        let inbox = info.inbox_id.clone();
                        let mut c = confirm;
                        c.set(Some(ConfirmAction {
                            title:         "Remove identity?".into(),
                            message:       "This removes the identity from this device. The XMTP inbox remains on the network.".into(),
                            confirm_label: "Remove".into(),
                            on_confirm: Arc::new(move || {
                                let mut ids = all_identities;
                                let remaining: Vec<IdentityInfo> = ids.peek()
                                    .iter()
                                    .filter(|i| i.inbox_id != inbox)
                                    .cloned()
                                    .collect();
                                ids.set(remaining);
                                if let Some(h) = xmtp.peek().as_ref() {
                                    h.request_remove_identity(idx);
                                }
                            }),
                        }));
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "14", height: "14",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.5", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M18 6L6 18" }
                        path { d: "M6 6l12 12" }
                    }
                }
            }
        }
    }
}

// ── Restore mnemonic dialog ────────────────────────────────────────────────────

#[component]
fn RestoreMnemonicSheet(xmtp: Signal<Option<XmtpHandle>>, on_close: EventHandler<()>) -> Element {
    let mut words: Signal<[String; 12]> = use_signal(|| std::array::from_fn(|_| String::new()));
    let all_filled = words.read().iter().all(|w| !w.trim().is_empty());

    rsx! {
        div { class: "sheet-backdrop", onclick: move |_| on_close.call(()), }
        div { class: "identity-sheet restore-sheet",
            div { class: "sheet-header",
                span { class: "sheet-title", "Restore Identity" }
                button {
                    class: "sheet-close-btn",
                    onclick: move |_| on_close.call(()),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "14", height: "14",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.5", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M18 6L6 18" }
                        path { d: "M6 6l12 12" }
                    }
                }
            }
            div { class: "sheet-body",
                p { class: "restore-hint", "Enter your 12-word recovery phrase." }
                div { class: "mnemonic-grid",
                    for i in 0..12usize {
                        div { class: "mnemonic-word-wrap",
                            span { class: "mnemonic-num", "{i + 1}" }
                            input {
                                class: "mnemonic-input",
                                r#type: "text",
                                autocomplete: "off",
                                autocorrect: "off",
                                autocapitalize: "none",
                                spellcheck: false,
                                value: "{words.read()[i]}",
                                oninput: move |e| {
                                    words.write()[i] = e.value().trim().to_lowercase();
                                },
                            }
                        }
                    }
                }
            }
            div { class: "sheet-footer",
                button {
                    class: "add-member-btn",
                    disabled: !all_filled,
                    onclick: move |_| {
                        let phrase = words.read().join(" ");
                        if let Some(h) = xmtp.read().as_ref() {
                            h.request_restore_identity(&phrase);
                        }
                        on_close.call(());
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
                        path { d: "M3 3v5h5" }
                    }
                    span { "Restore Identity" }
                }
            }
        }
    }
}

// ── Show mnemonic dialog ───────────────────────────────────────────────────────

#[component]
fn ShowMnemonicSheet(words: Vec<String>, on_close: EventHandler<()>) -> Element {
    let mut revealed = use_signal(|| false);
    rsx! {
        div { class: "sheet-backdrop", onclick: move |_| on_close.call(()), }
        div { class: "identity-sheet restore-sheet",
            div { class: "sheet-header",
                span { class: "sheet-title", "Recovery Phrase" }
                button {
                    class: "sheet-close-btn",
                    onclick: move |_| on_close.call(()),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "14", height: "14",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.5", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M18 6L6 18" }
                        path { d: "M6 6l12 12" }
                    }
                }
            }
            div { class: "sheet-body",
                div { class: "mnemonic-security-warning",
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "18", height: "18",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" }
                        line { x1: "12", y1: "9", x2: "12", y2: "13" }
                        line { x1: "12", y1: "17", x2: "12.01", y2: "17" }
                    }
                    span {
                        "Never share this phrase with anyone. Store it somewhere safe and private."
                    }
                }

                if words.is_empty() {
                    p { class: "restore-hint",
                        "No recovery phrase available for this identity. Create a new identity to get one."
                    }
                } else {
                    if !revealed() {
                        div { class: "mnemonic-reveal-wrap",
                            button {
                                class: "mnemonic-reveal-btn",
                                onclick: move |_| revealed.set(true),
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg", width: "17", height: "17",
                                    view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                                    stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                                    path { d: "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" }
                                    circle { cx: "12", cy: "12", r: "3" }
                                }
                                span { "I understand — show the phrase" }
                            }
                        }
                    }
                    div { class: "mnemonic-grid",
                        for (i, word) in words.iter().enumerate() {
                            div {
                                class: if revealed() { "mnemonic-word-wrap" } else { "mnemonic-word-wrap mnemonic-blurred" },
                                span { class: "mnemonic-num", "{i + 1}" }
                                span { class: "mnemonic-word-text", "{word}" }
                                if revealed() {
                                    CopyBtn { text: word.clone() }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Identities panel (sidebar content) ────────────────────────────────────────

#[component]
fn IdentitiesPanel() -> Element {
    let xmtp           = use_context::<Signal<Option<XmtpHandle>>>();
    let all_identities = use_context::<Signal<Vec<IdentityInfo>>>();
    let identity_info  = use_context::<Signal<Option<IdentityInfo>>>();
    let identity_ready = use_context::<Signal<bool>>();
    let sidebar_tab    = use_context::<Signal<SidebarTab>>();
    let view           = use_context::<Signal<View>>();
    let confirm        = use_context::<Signal<Option<ConfirmAction>>>();

    let mut show_restore    = use_signal(|| false);
    let mut show_phrase_for: Signal<Option<Vec<String>>> = use_signal(|| None);
    let mut show_qr_for:     Signal<Option<String>>      = use_signal(|| None);

    rsx! {
        div { class: "identity-panel",
            div { class: "section-label", "Identities" }

            div { class: "identity-list",
                if !identity_ready() {
                    div { class: "spinner-wrap",
                        div { class: "spinner" }
                        span { class: "spinner-label", "Loading…" }
                    }
                } else {
                    for (idx, info) in all_identities.read().iter().enumerate() {
                        IdentityCard {
                            idx,
                            info: info.clone(),
                            is_active: identity_info.read()
                                .as_ref()
                                .map(|a| a.inbox_id == info.inbox_id)
                                .unwrap_or(false),
                            xmtp,
                            all_identities,
                            confirm,
                            sidebar_tab,
                            view,
                            show_phrase_for,
                            show_qr_for,
                        }
                    }
                }
            }

            // Footer: create / restore buttons
            div { class: "identity-footer",
                button {
                    class: "identity-footer-btn",
                    disabled: !identity_ready(),
                    onclick: move |_| {
                        if let Some(h) = xmtp.read().as_ref() {
                            h.request_create_identity();
                        }
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M12 5v14" }
                        path { d: "M5 12h14" }
                    }
                    "Create"
                }
                button {
                    class: "identity-footer-btn",
                    disabled: !identity_ready(),
                    onclick: move |_| show_restore.set(true),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
                        path { d: "M3 3v5h5" }
                    }
                    "Restore"
                }
            }
        }

        if show_restore() {
            RestoreMnemonicSheet {
                xmtp,
                on_close: move |_| show_restore.set(false),
            }
        }

        if let Some(words) = show_phrase_for.read().clone() {
            ShowMnemonicSheet {
                words,
                on_close: move |_| show_phrase_for.set(None),
            }
        }

        if let Some(addr) = show_qr_for.read().clone() {
            ShowAddressQrSheet {
                address: addr,
                on_close: move |_| show_qr_for.set(None),
            }
        }
    }
}

// ── Sidebar (top-level component) ──────────────────────────────────────────────

#[component]
pub fn Sidebar() -> Element {
    let sidebar_tab = use_context::<Signal<SidebarTab>>();
    let dark_mode   = use_context::<Signal<DarkMode>>();

    let is_convos = *sidebar_tab.read() == SidebarTab::Conversations;
    let is_dark   = dark_mode.read().0;

    rsx! {
        div { class: "sidebar",
            // ── Header ─────────────────────────────────────────────
            div { class: "sidebar-header",
                div { class: "sidebar-logo",
                    img { class: "sidebar-logo-mark", src: LOGO, alt: "xmes" }
                    span { class: "sidebar-logo-name", "xmes" }
                }
                div { class: "sidebar-nav",
                    button {
                        class: if is_convos { "sidebar-nav-btn active" } else { "sidebar-nav-btn" },
                        title: "Conversations",
                        onclick: move |_| { let mut st = sidebar_tab; st.set(SidebarTab::Conversations); },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "18", height: "18",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: if is_convos { "2.4" } else { "1.8" },
                            stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                        }
                    }
                    button {
                        class: if !is_convos { "sidebar-nav-btn active" } else { "sidebar-nav-btn" },
                        title: "Identities",
                        onclick: move |_| { let mut st = sidebar_tab; st.set(SidebarTab::Identities); },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "18", height: "18",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: if !is_convos { "2.4" } else { "1.8" },
                            stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" }
                            circle { cx: "12", cy: "7", r: "4" }
                        }
                    }
                    // ── Dark mode toggle ──────────────────────────
                    button {
                        class: "dark-toggle-btn",
                        title: if is_dark { "Switch to light mode" } else { "Switch to dark mode" },
                        onclick: move |_| {
                            let curr = dark_mode.peek().0;
                            let mut dm = dark_mode;
                            dm.set(DarkMode(!curr));
                        },
                        if is_dark {
                            // Sun icon — currently dark, click to go light
                            svg {
                                xmlns: "http://www.w3.org/2000/svg", width: "17", height: "17",
                                view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                                circle { cx: "12", cy: "12", r: "4" }
                                path { d: "M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" }
                            }
                        } else {
                            // Moon icon — currently light, click to go dark
                            svg {
                                xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                                view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                                path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
                            }
                        }
                    }
                }
            }

            // ── Content ────────────────────────────────────────────
            if is_convos {
                ConversationsPanel {}
            } else {
                IdentitiesPanel {}
            }
        }
    }
}
