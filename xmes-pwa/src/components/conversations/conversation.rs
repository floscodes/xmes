use std::sync::Arc;
use dioxus::prelude::*;
use xmes_xmtp_wasm::{ConversationSummary, XmtpHandle};
use crate::ConfirmAction;
use crate::components::add_members::AddMembersSheet;

const DELETE_WIDTH: f64 = 80.0;
const SWIPE_THRESHOLD: f64 = 40.0;

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
        [w] => w.chars().next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or("?".into()),
        [first, .., last] => format!(
            "{}{}",
            first.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
            last.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
        ),
    }
}

#[component]
pub fn Convo(
    summary: ConversationSummary,
    on_open: EventHandler<ConversationSummary>,
    #[props(default = false)]
    has_unread: bool,
) -> Element {
    let mut show_add  = use_signal(|| false);
    let mut offset   = use_signal(|| 0.0f64);
    let mut start_x  = use_signal(|| 0.0f64);
    let mut dragging = use_signal(|| false);

    let delete_id    = summary.id.clone();
    let open_summary = summary.clone();
    let confirm       = use_context::<Signal<Option<ConfirmAction>>>();
    let xmtp          = use_context::<Signal<Option<XmtpHandle>>>();
    let conversations = use_context::<Signal<Option<Vec<ConversationSummary>>>>();

    let av_class = avatar_class(&summary.name);
    let av_text  = initials(&summary.name);

    let row_style = format!(
        "transform: translateX({}px); transition: {};",
        -offset(),
        if *dragging.read() { "none" } else { "transform 0.22s cubic-bezier(0.4,0,0.2,1)" }
    );

    if summary.is_pending {
        // ── Pending invitation row ────────────────────────────────────
        let inv_id_accept  = summary.id.clone();
        let inv_id_decline = summary.id.clone();
        rsx! {
            div { class: "convo-item",
                div { class: "convo-row invite-row",
                    div { class: "convo-avatar {av_class}", "{av_text}" }
                    div { class: "convo-info",
                        span { class: "convo-name", "{summary.name}" }
                        div {
                            span { class: "convo-sub invite-label", "Group invitation" }
                        }
                    }
                    div { class: "invite-actions",
                        button {
                            class: "invite-btn invite-accept",
                            title: "Accept",
                            onclick: move |_| {
                                let mut convos = conversations;
                                let id_ref = inv_id_accept.clone();
                                let updated = convos.peek().as_ref().map(|list| {
                                    list.iter().map(|c| {
                                        if c.id == id_ref {
                                            let mut c2 = c.clone();
                                            c2.is_pending = false;
                                            c2
                                        } else { c.clone() }
                                    }).collect::<Vec<_>>()
                                });
                                if let Some(u) = updated { convos.set(Some(u)); }
                                if let Some(h) = xmtp.peek().as_ref() {
                                    h.request_accept_invitation(&inv_id_accept);
                                }
                            },
                            svg {
                                xmlns: "http://www.w3.org/2000/svg",
                                width: "18", height: "18",
                                view_box: "0 0 24 24", fill: "none",
                                stroke: "currentColor", stroke_width: "2.5",
                                stroke_linecap: "round", stroke_linejoin: "round",
                                polyline { points: "20 6 9 17 4 12" }
                            }
                        }
                        button {
                            class: "invite-btn invite-decline",
                            title: "Decline",
                            onclick: move |_| {
                                let mut convos = conversations;
                                let id_ref = inv_id_decline.clone();
                                let filtered = convos.peek().as_ref().map(|list| {
                                    list.iter().filter(|c| c.id != id_ref).cloned().collect::<Vec<_>>()
                                });
                                if let Some(f) = filtered { convos.set(Some(f)); }
                                if let Some(h) = xmtp.peek().as_ref() {
                                    h.request_decline_invitation(&inv_id_decline);
                                }
                            },
                            svg {
                                xmlns: "http://www.w3.org/2000/svg",
                                width: "18", height: "18",
                                view_box: "0 0 24 24", fill: "none",
                                stroke: "currentColor", stroke_width: "2.5",
                                stroke_linecap: "round", stroke_linejoin: "round",
                                line { x1: "18", y1: "6", x2: "6", y2: "18" }
                                line { x1: "6", y1: "6", x2: "18", y2: "18" }
                            }
                        }
                    }
                }
            }
        }
    } else {
        rsx! {
            div { class: "convo-item",

                // Delete action revealed on swipe
                div { class: "delete-reveal",
                    button {
                        class: "delete-btn",
                        onclick: move |_| {
                            let id = delete_id.clone();
                            let mut c = confirm;
                            c.set(Some(ConfirmAction {
                                title:         "Leave conversation?".into(),
                                message:       "You will leave this group permanently.".into(),
                                confirm_label: "Leave".into(),
                                on_confirm: Arc::new(move || {
                                    let mut convos = conversations;
                                    let filtered = convos.peek().as_ref().map(|list| {
                                        let id_ref = id.clone();
                                        list.iter().filter(|c| c.id != id_ref).cloned().collect::<Vec<_>>()
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
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "18", height: "18",
                            view_box: "0 0 24 24", fill: "none",
                            stroke: "currentColor", stroke_width: "2",
                            stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4" }
                            polyline { points: "10 17 15 12 10 7" }
                            line { x1: "15", y1: "12", x2: "3", y2: "12" }
                        }
                        span { "Leave" }
                    }
                }

                // Conversation row (slides left on swipe)
                div {
                    class: "convo-row",
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
                        let current = *offset.read();
                        if current < SWIPE_THRESHOLD {
                            offset.set(0.0);
                            on_open.call(open_summary.clone());
                        } else {
                            offset.set(DELETE_WIDTH);
                        }
                    },
                    onpointercancel: move |_| {
                        dragging.set(false);
                        offset.set(0.0);
                    },

                    div { class: "convo-avatar-wrap",
                        div { class: "convo-avatar {av_class}", "{av_text}" }
                        if has_unread {
                            div { class: "unread-badge" }
                        }
                    }
                    div {
                        class: "convo-info",
                        span { class: if has_unread { "convo-name convo-name-unread" } else { "convo-name" }, "{summary.name}" }
                        if let Some(sender) = &summary.last_sender {
                            div {
                                span { class: "convo-sub", "{sender}" }
                            }
                        }
                    }
                    button {
                        class: "convo-add-btn",
                        title: "Add member",
                        onpointerdown: move |e| { e.stop_propagation(); },
                        onpointerup:   move |e| { e.stop_propagation(); },
                        onclick: move |e| {
                            e.stop_propagation();
                            show_add.set(true);
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "17", height: "17",
                            view_box: "0 0 24 24", fill: "none",
                            stroke: "currentColor", stroke_width: "2",
                            stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
                            circle { cx: "9", cy: "7", r: "4" }
                            line { x1: "19", y1: "8", x2: "19", y2: "14" }
                            line { x1: "22", y1: "11", x2: "16", y2: "11" }
                        }
                    }
                }
            }

            if show_add() {
                AddMembersSheet {
                    conversation_id: summary.id.clone(),
                    xmtp,
                    on_close: move |_| show_add.set(false),
                }
            }
        }
    }
}
