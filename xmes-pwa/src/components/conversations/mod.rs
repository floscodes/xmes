use dioxus::prelude::*;
use xmes_xmtp_wasm::{ConversationSummary, XmtpHandle};

mod conversation;

#[component]
pub fn Conversations() -> Element {
    let xmtp = use_context::<Signal<Option<XmtpHandle>>>();
    let conversations = use_context::<Signal<Option<Vec<ConversationSummary>>>>();
    let identity_ready = use_context::<Signal<bool>>();

    rsx! {
        div {
            class: "flex flex-col items-center",
            div {
                class: "w-[85%]",
                input {
                    class: "border border-solid rounded-md p-3 w-full",
                    placeholder: "Search..."
                }
            }
            div {
                class: "w-[85%] mt-6",
                match conversations.read().as_ref() {
                    None => rsx! {
                        div {
                            class: "animate-spin rounded-full h-8 w-8 border-4 border-gray-200 border-t-gray-600 mt-6 mx-auto"
                        }
                    },
                    Some(convos) if convos.is_empty() => rsx! {
                        div {
                            class: "text-gray-500 text-sm mt-4",
                            "No conversations found for this identity."
                        }
                    },
                    Some(convos) => rsx! {
                        for summary in convos.clone() {
                            conversation::Convo {
                                summary,
                                on_delete: move |id: String| {
                                    if let Some(h) = xmtp.read().as_ref() {
                                        h.request_leave(id);
                                    }
                                }
                            }
                        }
                    },
                }
            }
        }

        // Floating action button — create a new group conversation
        button {
            class: "fixed bottom-6 right-6 w-14 h-14 rounded-full bg-gray-900 text-white shadow-lg flex items-center justify-center hover:bg-gray-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed",
            title: "Create Conversation",
            disabled: !identity_ready(),
            onclick: move |_| {
                if let Some(h) = xmtp.read().as_ref() {
                    h.request_create_group();
                }
            },
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                width: "22",
                height: "22",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                line { x1: "12", y1: "8", x2: "12", y2: "14" }
                line { x1: "9", y1: "11", x2: "15", y2: "11" }
            }
        }
    }
}
