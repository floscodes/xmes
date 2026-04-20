use std::sync::Arc;
use dioxus::prelude::*;
use xmes_xmtp_wasm::{IdentityInfo, XmtpHandle};
use crate::{ConfirmAction, View};

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
                    div { class: "app-logo-mark",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "100%", height: "100%",
                            view_box: "0 0 176 170", fill: "none",
                            path {
                                d: "M175,23L0,24L1,170L175,168L175,23L86,107",
                                stroke: "rgba(255,255,255,0.55)",
                                stroke_width: "18",
                                stroke_linejoin: "round", stroke_linecap: "round",
                            }
                            path {
                                d: "M2,170L1,26L74,96L86,106L175,20L176,168L4,170L176,0",
                                stroke: "white",
                                stroke_width: "18",
                                stroke_linejoin: "round", stroke_linecap: "round",
                            }
                        }
                    }
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
    view: Signal<View>,
    anim: Signal<&'static str>,
    confirm: Signal<Option<ConfirmAction>>,
) -> Element {
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
        div { class: "convo-item",  // reuse swipe container styles

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
                        // Treat as tap → switch identity + navigate
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
                    span { class: "identity-address", "{inbox_short}" }

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

                // Right side: checkmark + add-address button
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
                        title: "Link another address",
                        onclick: move |e| {
                            e.stop_propagation();
                            if let Some(h) = xmtp.read().as_ref() {
                                h.request_add_address(idx);
                            }
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "14", height: "14",
                            view_box: "0 0 24 24", fill: "none",
                            stroke: "currentColor", stroke_width: "2.5",
                            stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M12 5v14" }
                            path { d: "M5 12h14" }
                        }
                    }
                }
            }
        }
    }
}
