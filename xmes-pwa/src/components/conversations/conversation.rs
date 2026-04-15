use dioxus::prelude::*;
use xmes_xmtp_wasm::ConversationSummary;

#[component]
pub fn Convo(summary: ConversationSummary) -> Element {
    rsx! {
        div {
            class: "flex flex-col border-b border-gray-100 py-3 px-2 cursor-pointer hover:bg-gray-50",
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