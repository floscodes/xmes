use dioxus::prelude::*;
use xmes_xmtp_wasm::{IdentityInfo, XmtpHandle};
use crate::View;

fn truncate(s: &str) -> String {
    if s.len() <= 14 { s.to_string() }
    else { format!("{}…{}", &s[..6], &s[s.len()-4..]) }
}

fn addr_avatar(address: &str) -> &'static str {
    let idx = address.bytes().fold(0usize, |a, b| a.wrapping_add(b as usize)) % 8;
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
                            let av    = addr_avatar(&info.address);
                            let addr  = truncate(&info.address);
                            let inbox = truncate(&info.inbox_id);
                            let is_active = identity_info.read()
                                .as_ref()
                                .map(|a| a.address == info.address)
                                .unwrap_or(false);
                            let info_clone = info.clone();

                            rsx! {
                                div {
                                    class: if is_active { "identity-card active" } else { "identity-card" },
                                    onclick: move |_| {
                                        if let Some(h) = xmtp.read().as_ref() {
                                            h.request_switch_identity(idx);
                                        }
                                        let mut a = anim; a.set("slide-in-right");
                                        let mut v = view; v.set(View::Conversations);
                                    },

                                    div { class: "identity-avatar {av}",
                                        svg {
                                            xmlns: "http://www.w3.org/2000/svg",
                                            width: "22", height: "22",
                                            view_box: "0 0 24 24", fill: "none",
                                            stroke: "white", stroke_width: "2",
                                            stroke_linecap: "round", stroke_linejoin: "round",
                                            path { d: "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" }
                                            circle { cx: "12", cy: "7", r: "4" }
                                        }
                                    }

                                    div { class: "identity-info",
                                        div { class: "identity-row",
                                            span { class: "identity-label",
                                                "Identity {idx + 1}"
                                            }
                                            if is_active {
                                                span { class: "identity-active-badge", "Active" }
                                            }
                                        }
                                        span { class: "identity-address", "{addr}" }
                                        span { class: "identity-inbox",   "Inbox  {inbox}" }
                                    }

                                    if is_active {
                                        div { class: "identity-check",
                                            svg {
                                                xmlns: "http://www.w3.org/2000/svg",
                                                width: "18", height: "18",
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
            }
        }

        // ── FAB — add new identity ───────────────────────────────
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
