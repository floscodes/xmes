use dioxus::prelude::*;
use xmes_xmtp_wasm::Identity;

mod conversation;

#[component]
pub fn Conversations(active_identity: Signal<Option<Identity>>) -> Element {
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
            if active_identity.read().is_some() {
                conversation::Conversation { active_identity }
            } else {
                div {
                    class: "animate-spin rounded-full h-8 w-8 border-4 border-gray-200 border-t-gray-600 mt-6 mx-auto"
                }
            }
        }
    }
    }
}