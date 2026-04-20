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
    let mut copied = use_signal(|| false);
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
                        }
                    }
                }
            }
        }

        // ── FAB — add new independent identity ──────────────────
        button {
            class: "fab",
            title: "Add new identity",
            disabled: !identity_ready(),
            onclick: move |_| {
                if let Some(h) = xmtp.read().as_ref() {
                    h.request_create_identity();
                }
            },
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
) -> Element {
    let mut show_options = use_signal(|| false);
    let mut offset   = use_signal(|| 0.0f64);
    let mut start_x  = use_signal(|| 0.0f64);
    let mut dragging = use_signal(|| false);

    let av           = inbox_avatar(&info.inbox_id);
    let inbox_short  = short(&info.inbox_id, 8);
    let addresses    = info.addresses.clone();

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
                        span { class: "identity-label", "Inbox" }
                        if is_active {
                            span { class: "identity-active-badge", "Active" }
                        }
                    }
                    // Inbox ID with copy button
                    div { class: "identity-copy-row",
                        span { class: "identity-address", "{inbox_short}" }
                        CopyBtn { text: info.inbox_id.clone() }
                    }

                    if !addresses.is_empty() {
                        div { class: "identity-addr-section",
                            span { class: "identity-addr-label",
                                if addresses.len() == 1 { "Linked address" } else { "Linked addresses" }
                            }
                            for addr in &addresses {
                                span { class: "identity-inbox", "{short(addr, 6)}" }
                            }
                        }
                    }
                }

                // Right side: checkmark + options button
                div { class: "identity-actions",
                    if is_active {
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
                    button {
                        class: "identity-add-addr-btn",
                        title: "Identity options",
                        onpointerdown: move |e| { e.stop_propagation(); },
                        onpointerup:   move |e| { e.stop_propagation(); },
                        onclick: move |e| {
                            e.stop_propagation();
                            show_options.set(true);
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "16", height: "16",
                            view_box: "0 0 24 24", fill: "currentColor",
                            stroke: "none",
                            circle { cx: "5",  cy: "12", r: "2" }
                            circle { cx: "12", cy: "12", r: "2" }
                            circle { cx: "19", cy: "12", r: "2" }
                        }
                    }
                }
            }
        }

        if show_options() {
            IdentityOptionsSheet {
                identity_idx: idx,
                inbox_id: info.inbox_id.clone(),
                xmtp,
                all_identities,
                on_close: move |_| show_options.set(false),
            }
        }
    }
}

#[component]
fn IdentityOptionsSheet(
    identity_idx: usize,
    inbox_id: String,
    xmtp: Signal<Option<XmtpHandle>>,
    all_identities: Signal<Vec<IdentityInfo>>,
    on_close: EventHandler<()>,
) -> Element {
    let current = all_identities
        .read()
        .iter()
        .find(|i| i.inbox_id == inbox_id)
        .cloned();

    let (primary_address, other_addresses) = match &current {
        Some(info) => {
            let others = info.addresses.iter()
                .filter(|a| **a != info.primary_address)
                .cloned()
                .collect::<Vec<_>>();
            (info.primary_address.clone(), others)
        }
        None => (String::new(), vec![]),
    };

    rsx! {
        div {
            class: "sheet-backdrop",
            onclick: move |_| on_close.call(()),
        }
        div { class: "identity-sheet",
            div { class: "sheet-handle" }
            div { class: "sheet-header",
                // Inbox ID + copy button in the title area
                div { class: "sheet-title-row",
                    span { class: "sheet-title", "{short(&inbox_id, 8)}" }
                    CopyBtn { text: inbox_id.clone() }
                }
                button {
                    class: "sheet-close-btn",
                    onclick: move |_| on_close.call(()),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "14", height: "14",
                        view_box: "0 0 24 24", fill: "none",
                        stroke: "currentColor", stroke_width: "2.5",
                        stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M18 6L6 18" }
                        path { d: "M6 6l12 12" }
                    }
                }
            }
            div { class: "sheet-section-label", "Linked addresses" }
            div { class: "sheet-addr-list",
                // Primary address — not removable, shown as inactive pill
                if !primary_address.is_empty() {
                    div { class: "addr-primary-pill",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "14", height: "14",
                            view_box: "0 0 24 24", fill: "none",
                            stroke: "currentColor", stroke_width: "2",
                            stroke_linecap: "round", stroke_linejoin: "round",
                            rect { x: "3", y: "11", width: "18", height: "11", rx: "2", ry: "2" }
                            path { d: "M7 11V7a5 5 0 0 1 10 0v4" }
                        }
                        span { class: "addr-text", "{short(&primary_address, 10)}" }
                        CopyBtn { text: primary_address.clone() }
                        span { class: "addr-primary-badge", "Primary" }
                    }
                }
                // Additional linked addresses — swipeable
                if other_addresses.is_empty() && primary_address.is_empty() {
                    div { class: "sheet-empty", "No linked addresses" }
                }
                for addr in other_addresses.iter() {
                    AddressRow {
                        identity_idx,
                        address: addr.clone(),
                        inbox_id: inbox_id.clone(),
                        xmtp,
                        all_identities,
                    }
                }
            }
            div { class: "sheet-footer",
                button {
                    class: "sheet-fab",
                    title: "Link another address",
                    onclick: move |_| {
                        if let Some(h) = xmtp.read().as_ref() {
                            h.request_add_address(identity_idx);
                        }
                    },
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
            }
        }
    }
}

#[component]
fn AddressRow(
    identity_idx: usize,
    address: String,
    inbox_id: String,
    xmtp: Signal<Option<XmtpHandle>>,
    all_identities: Signal<Vec<IdentityInfo>>,
) -> Element {
    let mut offset   = use_signal(|| 0.0f64);
    let mut start_x  = use_signal(|| 0.0f64);
    let mut dragging = use_signal(|| false);

    let addr_display = short(&address, 10);

    let row_style = format!(
        "transform: translateX({}px); transition: {}; touch-action: pan-y; user-select: none;",
        -offset(),
        if *dragging.read() { "none" } else { "transform 0.22s cubic-bezier(0.4,0,0.2,1)" }
    );

    rsx! {
        div { class: "convo-item",
            div { class: "delete-reveal",
                button {
                    class: "delete-btn",
                    onclick: move |_| {
                        let addr = address.clone();
                        let iid  = inbox_id.clone();
                        {
                            let mut v = all_identities.write();
                            if let Some(identity) = v.iter_mut().find(|i| i.inbox_id == iid) {
                                identity.addresses.retain(|a| a != &addr);
                            }
                        }
                        if let Some(h) = xmtp.peek().as_ref() {
                            h.request_remove_address(identity_idx, &addr);
                        }
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
            div {
                class: "addr-row",
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
                    if *offset.read() < SWIPE_THRESHOLD {
                        offset.set(0.0);
                    } else {
                        offset.set(DELETE_WIDTH);
                    }
                },
                onpointercancel: move |_| {
                    dragging.set(false);
                    offset.set(0.0);
                },
                div { class: "addr-dot" }
                span { class: "addr-text", "{addr_display}" }
                CopyBtn { text: address.clone() }
            }
        }
    }
}
