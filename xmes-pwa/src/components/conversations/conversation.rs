use dioxus::prelude::*;
use xmes_xmtp_wasm::Identity;

#[component]
pub fn Conversation(active_identity: Signal<Option<Identity>>) -> Element {
    rsx! {
        "Conversation"
    }
}