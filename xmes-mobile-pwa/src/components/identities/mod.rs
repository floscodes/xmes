use std::sync::Arc;
use dioxus::prelude::*;
use xmes_xmtp_wasm::{IdentityInfo, XmtpHandle};
use crate::{ConfirmAction, View};

const LOGO: Asset = asset!("/assets/icons/xmes-icon.svg");

const DELETE_WIDTH: f64 = 80.0;
const SWIPE_THRESHOLD: f64 = 40.0;

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

/// Write `text` to the clipboard and briefly flip `copied` to true.
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

/// Small inline copy button. Shows a checkmark for 1.5 s after clicking.
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
            onpointerdown: move |e| { e.stop_propagation(); },
            onpointerup:   move |e| { e.stop_propagation(); },
            if copied() {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "13", height: "13",
                    view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2.8",
                    stroke_linecap: "round", stroke_linejoin: "round",
                    polyline { points: "20 6 9 17 4 12" }
                }
            } else {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "13", height: "13",
                    view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2",
                    stroke_linecap: "round", stroke_linejoin: "round",
                    rect { x: "9", y: "9", width: "13", height: "13", rx: "2", ry: "2" }
                    path { d: "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" }
                }
            }
        }
    }
}

#[component]
pub fn Identities() -> Element {
    let xmtp           = use_context::<Signal<Option<XmtpHandle>>>();
    let all_identities = use_context::<Signal<Vec<IdentityInfo>>>();
    let identity_info  = use_context::<Signal<Option<IdentityInfo>>>();
    let identity_ready = use_context::<Signal<bool>>();
    let view           = use_context::<Signal<View>>();
    let anim           = use_context::<Signal<&'static str>>();
    let confirm        = use_context::<Signal<Option<ConfirmAction>>>();

    let mut fab_menu_open    = use_signal(|| false);
    let mut show_restore     = use_signal(|| false);
    let mut show_phrase_for: Signal<Option<Vec<String>>> = use_signal(|| None);

    rsx! {
        div { class: "app-shell",

            // ── Header ──────────────────────────────────────────
            header { class: "app-header",
                div { class: "app-logo",
                    img { class: "app-logo-mark", src: LOGO, alt: "xmes logo" }
                    span { class: "app-logo-name", "xmes" }
                }
            }

            div { class: "section-label", "Identities" }

            // ── Identity list ────────────────────────────────────
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
                            view,
                            anim,
                            confirm,
                            show_phrase_for,
                        }
                    }
                }
            }
        }

        // ── FAB menu overlay ────────────────────────────────────
        if fab_menu_open() {
            div {
                class: "fab-menu-overlay",
                onclick: move |_| fab_menu_open.set(false),
            }
            div { class: "fab-menu",
                button {
                    class: "fab-menu-item",
                    onclick: move |_| {
                        fab_menu_open.set(false);
                        if let Some(h) = xmtp.read().as_ref() {
                            h.request_create_identity();
                        }
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "18", height: "18",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M12 5v14" }
                        path { d: "M5 12h14" }
                    }
                    span { "Create" }
                }
                button {
                    class: "fab-menu-item",
                    onclick: move |_| {
                        fab_menu_open.set(false);
                        show_restore.set(true);
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "18", height: "18",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
                        path { d: "M3 3v5h5" }
                    }
                    span { "Restore" }
                }
            }
        }

        // ── FAB ─────────────────────────────────────────────────
        button {
            class: if fab_menu_open() { "fab fab-open" } else { "fab" },
            title: "Add identity",
            disabled: !identity_ready(),
            onclick: move |_| fab_menu_open.set(!fab_menu_open()),
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                width: "22", height: "22",
                view_box: "0 0 24 24", fill: "none",
                stroke: "currentColor", stroke_width: "2.2",
                stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M12 5v14" }
                path { d: "M5 12h14" }
            }
        }

        // ── Restore sheet ────────────────────────────────────────
        if show_restore() {
            RestoreMnemonicSheet {
                xmtp,
                on_close: move |_| show_restore.set(false),
            }
        }

        // ── Show mnemonic sheet ───────────────────────────────────
        if let Some(words) = show_phrase_for.read().clone() {
            ShowMnemonicSheet {
                words,
                on_close: move |_| show_phrase_for.set(None),
            }
        }
    }
}

#[component]
fn RestoreMnemonicSheet(
    xmtp: Signal<Option<XmtpHandle>>,
    on_close: EventHandler<()>,
) -> Element {
    let mut words: Signal<[String; 12]> = use_signal(|| std::array::from_fn(|_| String::new()));
    let all_filled = words.read().iter().all(|w| !w.trim().is_empty());

    rsx! {
        div { class: "sheet-backdrop", onclick: move |_| on_close.call(()), }
        div { class: "identity-sheet restore-sheet",
            div { class: "sheet-handle" }
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
            div { class: "restore-hint",
                "Enter your 12-word recovery phrase."
            }
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

#[component]
fn ShowMnemonicSheet(
    words: Vec<String>,
    on_close: EventHandler<()>,
) -> Element {
    let mut revealed = use_signal(|| false);
    rsx! {
        div { class: "sheet-backdrop", onclick: move |_| on_close.call(()), }
        div { class: "identity-sheet restore-sheet",
            div { class: "sheet-handle" }
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

            // Security warning
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
                    "Never share this phrase with anyone. Anyone with this phrase can take control of your identity. Store it somewhere safe and private."
                }
            }

            if words.is_empty() {
                div { class: "restore-hint",
                    "No recovery phrase available for this identity. It was created before phrase support was added. Create a new identity to get a recovery phrase."
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

#[component]
fn IdentityCard(
    idx: usize,
    info: IdentityInfo,
    is_active: bool,
    xmtp: Signal<Option<XmtpHandle>>,
    all_identities: Signal<Vec<IdentityInfo>>,
    view: Signal<View>,
    anim: Signal<&'static str>,
    confirm: Signal<Option<ConfirmAction>>,
    show_phrase_for: Signal<Option<Vec<String>>>,
) -> Element {
    let mut offset   = use_signal(|| 0.0f64);
    let mut start_x  = use_signal(|| 0.0f64);
    let mut dragging = use_signal(|| false);

    let av          = inbox_avatar(&info.inbox_id);
    let addr_short  = short(&info.primary_address, 8);
    let inbox_short = short(&info.inbox_id, 8);
    let words: Vec<String> = info.mnemonic.as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(str::to_string)
        .collect();

    let row_style = format!(
        "transform: translateX({}px); transition: {}; touch-action: pan-y; user-select: none;",
        -offset(),
        if *dragging.read() { "none" } else { "transform 0.22s cubic-bezier(0.4,0,0.2,1)" }
    );

    rsx! {
        div { class: "convo-item",

            // Delete action revealed on swipe
            div { class: "delete-reveal",
                button {
                    class: "delete-btn",
                    onclick: move |_| {
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
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "18", height: "18",
                        view_box: "0 0 24 24", fill: "none",
                        stroke: "currentColor", stroke_width: "2.5",
                        stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M18 6L6 18" }
                        path { d: "M6 6l12 12" }
                    }
                    span { "Remove" }
                }
            }

            // Card row (slides on swipe, tap to switch)
            div {
                class: if is_active { "identity-card active" } else { "identity-card" },
                style: "{row_style}",
                onpointerdown: move |e| {
                    start_x.set(e.client_coordinates().x);
                    dragging.set(true);
                },
                onpointermove: move |e| {
                    if !*dragging.read() { return; }
                    let dx = (start_x() - e.client_coordinates().x)
                        .max(0.0).min(DELETE_WIDTH);
                    offset.set(dx);
                },
                onpointerup: move |_| {
                    dragging.set(false);
                    let cur = *offset.read();
                    if cur < SWIPE_THRESHOLD {
                        offset.set(0.0);
                        if let Some(h) = xmtp.read().as_ref() {
                            h.request_switch_identity(idx);
                        }
                        let mut a = anim; a.set("slide-in-right");
                        let mut v = view; v.set(View::Conversations);
                    } else {
                        offset.set(DELETE_WIDTH);
                    }
                },
                onpointercancel: move |_| {
                    dragging.set(false);
                    offset.set(0.0);
                },

                // Avatar
                div { class: "identity-avatar {av}",
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "20", height: "20",
                        view_box: "0 0 24 24", fill: "none",
                        stroke: "white", stroke_width: "2",
                        stroke_linecap: "round", stroke_linejoin: "round",
                        rect { x: "3", y: "11", width: "18", height: "11", rx: "2", ry: "2" }
                        path { d: "M7 11V7a5 5 0 0 1 10 0v4" }
                    }
                }

                // Info block
                div { class: "identity-info",
                    div { class: "identity-row",
                        span { class: "identity-label", "Address" }
                        if is_active {
                            span { class: "identity-active-badge", "Active" }
                        }
                    }
                    // Primary address with copy button
                    div { class: "identity-copy-row",
                        span { class: "identity-address", "{addr_short}" }
                        CopyBtn { text: info.primary_address.clone() }
                    }
                    // Inbox ID small
                    div { class: "identity-addr-section",
                        span { class: "identity-addr-label", "Inbox ID" }
                        div { class: "identity-copy-row identity-copy-row--small",
                            span { class: "identity-inbox", "{inbox_short}" }
                            CopyBtn { text: info.inbox_id.clone() }
                        }
                    }

                    // Show recovery phrase button (always visible)
                    button {
                        class: "show-phrase-btn",
                        onpointerdown: move |e| e.stop_propagation(),
                        onpointerup:   move |e| e.stop_propagation(),
                        onclick: move |e| {
                            e.stop_propagation();
                            show_phrase_for.set(Some(words.clone()));
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "12", height: "12",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                            rect { x: "3", y: "11", width: "18", height: "11", rx: "2", ry: "2" }
                            path { d: "M7 11V7a5 5 0 0 1 10 0v4" }
                        }
                        span { "Share with other device" }
                    }
                }

                // Right side: checkmark if active
                if is_active {
                    div { class: "identity-actions",
                        div { class: "identity-check",
                            svg {
                                xmlns: "http://www.w3.org/2000/svg",
                                width: "16", height: "16",
                                view_box: "0 0 24 24", fill: "none",
                                stroke: "currentColor", stroke_width: "2.5",
                                stroke_linecap: "round", stroke_linejoin: "round",
                                polyline { points: "20 6 9 17 4 12" }
                            }
                        }
                    }
                }
            }
        }

    }
}
