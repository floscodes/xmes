use dioxus::prelude::*;
use xmes_xmtp_wasm::ConversationSummary;

const DELETE_WIDTH: f64 = 80.0;
const SWIPE_THRESHOLD: f64 = 40.0;

#[component]
pub fn Convo(summary: ConversationSummary, on_delete: EventHandler<String>) -> Element {
    let mut offset = use_signal(|| 0.0f64);
    let mut start_x = use_signal(|| 0.0f64);
    let mut dragging = use_signal(|| false);

    let delete_id = summary.id.clone();

    let content_style = format!(
        "transform: translateX({}px); transition: {}; touch-action: pan-y;",
        -offset(),
        if *dragging.read() { "none" } else { "transform 0.25s cubic-bezier(0.4,0,0.2,1)" }
    );

    rsx! {
        div {
            class: "relative overflow-hidden select-none",

            // Red delete action revealed by swiping
            div {
                class: "absolute inset-y-0 right-0 flex items-stretch",
                style: "width: {DELETE_WIDTH}px",
                button {
                    class: "flex flex-col items-center justify-center w-full bg-red-500 text-white gap-1",
                    onclick: move |_| on_delete.call(delete_id.clone()),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "20",
                        height: "20",
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
                    span { class: "text-xs font-semibold", "Delete" }
                }
            }

            // Conversation row — slides left on swipe
            div {
                class: "relative bg-white flex flex-col border-b border-gray-100 py-3 px-2",
                style: "{content_style}",
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
                    let snap = if *offset.read() >= SWIPE_THRESHOLD { DELETE_WIDTH } else { 0.0 };
                    offset.set(snap);
                },
                onpointercancel: move |_| {
                    dragging.set(false);
                    offset.set(0.0);
                },
                span {
                    class: "font-medium text-gray-900 text-sm truncate",
                    "{summary.name}"
                }
                if let Some(sender) = &summary.last_sender {
                    span {
                        class: "text-xs text-gray-400 truncate mt-0.5",
                        "{sender}"
                    }
                }
            }
        }
    }
}
