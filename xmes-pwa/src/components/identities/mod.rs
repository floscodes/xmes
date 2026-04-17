use dioxus::prelude::*;
use xmes_xmtp_wasm::{IdentityInfo, XmtpHandle};
use crate::View;

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
                        {
                            let av = inbox_avatar(&info.inbox_id);
                            let inbox_short = short(&info.inbox_id, 8);
                            let is_active   = identity_info.read()
                                .as_ref()
                                .map(|a| a.inbox_id == info.inbox_id)
                                .unwrap_or(false);
                            let addresses   = info.addresses.clone();
                            let info_clone  = info.clone();

                            rsx! {
                                div {
                                    class: if is_active { "identity-card active" } else { "identity-card" },

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

                                    // Info block — tap to switch + navigate
                                    div {
                                        class: "identity-info",
                                        style: "flex:1; cursor:pointer;",
                                        onclick: move |_| {
                                            if let Some(h) = xmtp.read().as_ref() {
                                                h.request_switch_identity(idx);
                                            }
                                            let mut a = anim; a.set("slide-in-right");
                                            let mut v = view; v.set(View::Conversations);
                                        },

                                        // Primary: inbox ID
                                        div { class: "identity-row",
                                            span { class: "identity-label", "Inbox" }
                                            if is_active {
                                                span { class: "identity-active-badge", "Active" }
                                            }
                                        }
                                        span { class: "identity-address", "{inbox_short}" }

                                        // Secondary: linked addresses
                                        if !addresses.is_empty() {
                                            div { class: "identity-addr-section",
                                                span { class: "identity-addr-label",
                                                    if addresses.len() == 1 { "Linked address" } else { "Linked addresses" }
                                                }
                                                for addr in &addresses {
                                                    span { class: "identity-inbox",
                                                        "{short(addr, 6)}"
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Right side: checkmark (if active) + add-address button
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
                                            title: "Link another address to this inbox",
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
