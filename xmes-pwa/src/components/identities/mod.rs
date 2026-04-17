use dioxus::prelude::*;
use xmes_xmtp_wasm::{IdentityInfo, XmtpHandle};

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…{}", &s[..6], &s[s.len() - 4..])
    }
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
    let identity_info = use_context::<Signal<Option<IdentityInfo>>>();
    let identity_ready = use_context::<Signal<bool>>();
    let xmtp = use_context::<Signal<Option<XmtpHandle>>>();

    rsx! {
        div { class: "app-shell",

            // ── Header ──────────────────────────────────────────
            header { class: "app-header",
                div { class: "app-logo",
                    div { class: "app-logo-mark",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "100%", height: "100%",
                            view_box: "0 0 176 170",
                            fill: "none",
                            path {
                                d: "M175,23L0,24L1,170L175,168L175,23L86,107",
                                stroke: "rgba(255,255,255,0.55)",
                                stroke_width: "18",
                                stroke_linejoin: "round",
                                stroke_linecap: "round",
                            }
                            path {
                                d: "M2,170L1,26L74,96L86,106L175,20L176,168L4,170L176,0",
                                stroke: "white",
                                stroke_width: "18",
                                stroke_linejoin: "round",
                                stroke_linecap: "round",
                            }
                        }
                    }
                    span { class: "app-logo-name", "xmes" }
                }
            }

            // ── Section label ────────────────────────────────────
            div { class: "section-label", "Identities" }

            // ── Identity list ────────────────────────────────────
            div { class: "identity-list",
                match identity_info.read().as_ref() {
                    None => rsx! {
                        div { class: "spinner-wrap",
                            div { class: "spinner" }
                            span { class: "spinner-label", "Loading identity…" }
                        }
                    },
                    Some(info) => {
                        let av  = addr_avatar(&info.address);
                        let addr_short    = truncate(&info.address, 12);
                        let inbox_short   = truncate(&info.inbox_id, 12);
                        rsx! {
                            div { class: "identity-card active",
                                div { class: "identity-avatar {av}",
                                    svg {
                                        xmlns: "http://www.w3.org/2000/svg",
                                        width: "22", height: "22",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "white",
                                        stroke_width: "2",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        path { d: "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" }
                                        circle { cx: "12", cy: "7", r: "4" }
                                    }
                                }
                                div { class: "identity-info",
                                    div { class: "identity-row",
                                        span { class: "identity-label", "Address" }
                                        span { class: "identity-active-badge", "Active" }
                                    }
                                    span { class: "identity-address", "{addr_short}" }
                                    span { class: "identity-inbox", "Inbox  {inbox_short}" }
                                }
                                div { class: "identity-check",
                                    svg {
                                        xmlns: "http://www.w3.org/2000/svg",
                                        width: "18", height: "18",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2.5",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        polyline { points: "20 6 9 17 4 12" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── FAB — add identity (coming soon) ────────────────────
        button {
            class: "fab",
            title: "Add identity — coming soon",
            disabled: true,
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                width: "22", height: "22",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2.2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                path { d: "M12 5v14" }
                path { d: "M5 12h14" }
            }
        }
    }
}
