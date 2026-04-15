use std::rc::Rc;

use dioxus::prelude::*;
use xmes_xmtp_wasm::{ConversationSummary, Identity};

mod conversation;

#[component]
pub fn Conversations(active_identity: Signal<Option<Rc<Identity>>>) -> Element {
    let mut conversations = use_resource(move || async move {
        // Rc::clone while holding the guard, then release the guard before .await
        let rc_id = active_identity.read().as_ref().map(Rc::clone);
        match rc_id {
            None => None,
            Some(id) => Some(id.list_conversations().await.unwrap_or_default()),
        }
    });

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
                if let Some(Some(convos)) = (*conversations.read()).clone() {
                    if convos.is_empty() {
                        div {
                            class: "text-gray-500 text-sm mt-4",
                            "No conversations found for this identity."
                        }
                    } else {
                        for summary in convos {
                            conversation::Convo { summary }
                        }
                    }
                } else {
                    div {
                        class: "animate-spin rounded-full h-8 w-8 border-4 border-gray-200 border-t-gray-600 mt-6 mx-auto"
                    }
                }
            }
        }

        // Floating action button
        button {
            class: "fixed bottom-6 right-6 w-14 h-14 rounded-full bg-gray-900 text-white shadow-lg flex items-center justify-center hover:bg-gray-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed",
            title: "Create Conversation",
            disabled: active_identity.read().is_none(),
            onclick: move |_| {
                let rc_id = active_identity.read().as_ref().map(Rc::clone);
                spawn(async move {
                    if let Some(id) = rc_id {
                        let _ = id.create_group().await;
                        conversations.restart();
                    }
                });
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
