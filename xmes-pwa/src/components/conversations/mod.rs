use std::rc::Rc;

use dioxus::prelude::*;
use xmes_xmtp_wasm::{ConversationSummary, Identity};

mod conversation;

#[component]
pub fn Conversations(active_identity: Signal<Option<Rc<Identity>>>) -> Element {
    let conversations = use_resource(move || async move {
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
    }
}
