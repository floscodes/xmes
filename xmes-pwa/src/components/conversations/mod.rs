use dioxus::prelude::*;
use xmes_xmtp::Identity;

mod conversation;

#[component]
pub fn Conversations(identity: Signal<Option<Identity>>) -> Element {
    rsx! {
        div {
            class: "flex justify-center",
            input {
                class: "border border-solid rounded-md p-3 w-10/11",
                placeholder: "Search..."
            }
        }
        div {
            {
                if identity.read().is_some() {
                    rsx! {
                        conversation::Conversation { identity }
                    }
                } else {
                    rsx! {
                        "No Conversations found on this identity. Please select an identity or create a new one."
                    }
                }
            }
        }
    }
}