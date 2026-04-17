use dioxus::prelude::*;
use xmes_xmtp_wasm::ConversationSummary;

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
    on_delete: EventHandler<String>,
    on_open: EventHandler<ConversationSummary>,
) -> Element {
    let mut offset = use_signal(|| 0.0f64);
    let mut start_x = use_signal(|| 0.0f64);
    let mut dragging = use_signal(|| false);

    let delete_id = summary.id.clone();
    let open_summary = summary.clone();
    let av_class = avatar_class(&summary.name);
    let av_text = initials(&summary.name);

    let row_style = format!(
        "transform: translateX({}px); transition: {};",
        -offset(),
        if *dragging.read() { "none" } else { "transform 0.22s cubic-bezier(0.4,0,0.2,1)" }
    );

    rsx! {
        div {
            class: "convo-item",

            // Delete action revealed on swipe
            div {
                class: "delete-reveal",
                button {
                    class: "delete-btn",
                    onclick: move |_| on_delete.call(delete_id.clone()),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "18", height: "18",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        polyline { points: "3 6 5 6 21 6" }
                        path { d: "M19 6l-1 14H6L5 6" }
                        path { d: "M10 11v6" }
                        path { d: "M14 11v6" }
                        path { d: "M9 6V4h6v2" }
                    }
                    span { "Delete" }
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
                        .max(0.0)
                        .min(DELETE_WIDTH);
                    offset.set(dx);
                },
                onpointerup: move |_| {
                    dragging.set(false);
                    let current = *offset.read();
                    if current < SWIPE_THRESHOLD {
                        // Snap closed — treat as a tap → open chat
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

                div { class: "convo-avatar {av_class}", "{av_text}" }

                div {
                    class: "convo-info",
                    span { class: "convo-name", "{summary.name}" }
                    if let Some(sender) = &summary.last_sender {
                        span { class: "convo-sub", "{sender}" }
                    }
                }
            }
        }
    }
}
