use dioxus::prelude::*;
use xmes_xmtp_wasm::XmtpHandle;

#[component]
pub fn AddMembersSheet(
    conversation_id: String,
    xmtp: Signal<Option<XmtpHandle>>,
    on_close: EventHandler<()>,
) -> Element {
    let mut inbox_input = use_signal(|| String::new());

    let can_add = !inbox_input.read().trim().is_empty();

    rsx! {
        div { class: "sheet-backdrop", onclick: move |_| on_close.call(()), }
        div { class: "identity-sheet",
            div { class: "sheet-handle" }
            div { class: "sheet-header",
                span { class: "sheet-title", "Add member" }
                button {
                    class: "sheet-close-btn",
                    onclick: move |_| on_close.call(()),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "14", height: "14",
                        view_box: "0 0 24 24", fill: "none",
                        stroke: "currentColor", stroke_width: "2.5",
                        stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M18 6L6 18" }
                        path { d: "M6 6l12 12" }
                    }
                }
            }

            div { class: "add-member-body",
                p { class: "add-member-hint",
                    "Enter the XMTP inbox ID of the person you want to add to this conversation."
                }
                input {
                    class: "add-member-input",
                    r#type: "text",
                    placeholder: "Inbox ID…",
                    autofocus: true,
                    value: "{inbox_input}",
                    oninput: move |e| inbox_input.set(e.value()),
                    onkeydown: {
                        let conv_id = conversation_id.clone();
                        move |e: Event<KeyboardData>| {
                            if e.data().code().to_string() == "Enter" && can_add {
                                let id = inbox_input.read().trim().to_string();
                                inbox_input.set(String::new());
                                if let Some(h) = xmtp.read().as_ref() {
                                    h.request_add_members(&conv_id, &[id]);
                                }
                                on_close.call(());
                            }
                        }
                    },
                }
            }

            div { class: "sheet-footer",
                button {
                    class: "add-member-btn",
                    disabled: !can_add,
                    onclick: {
                        let conv_id = conversation_id.clone();
                        move |_| {
                            let id = inbox_input.read().trim().to_string();
                            if id.is_empty() { return; }
                            inbox_input.set(String::new());
                            if let Some(h) = xmtp.read().as_ref() {
                                h.request_add_members(&conv_id, &[id]);
                            }
                            on_close.call(());
                        }
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "16", height: "16",
                        view_box: "0 0 24 24", fill: "none",
                        stroke: "currentColor", stroke_width: "2.2",
                        stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
                        circle { cx: "9", cy: "7", r: "4" }
                        path { d: "M22 21v-2a4 4 0 0 0-3-3.87" }
                        path { d: "M16 3.13a4 4 0 0 1 0 7.75" }
                    }
                    span { "Add" }
                }
            }
        }
    }
}
