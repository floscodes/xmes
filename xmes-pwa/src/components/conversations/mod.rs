use dioxus::prelude::*;

mod conversation;

#[component]
pub fn Conversations() -> Element {
    rsx! {
        div {
            class: "flex justify-center",
            input {
                class: "border border-solid rounded-md p-3 w-10/11",
                placeholder: "Search..."
            }
        }
        div {
            class: "conversations-div",
        }
    }
}