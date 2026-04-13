use dioxus::prelude::*;
use xmes_xmtp::Identity;

#[component]
pub fn Conversation(identity: Signal<Option<Identity>>) -> Element {
    rsx! {
        "Conversation"
    }
}