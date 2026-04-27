use dioxus::prelude::*;
use xmes_xmtp_wasm::XmtpHandle;
use crate::components::qr::QrScannerSheet;

fn notify_push_invite(new_member_inbox_id: &str, group_name: &str) {
    let id   = new_member_inbox_id.replace('"', "");
    let name = group_name.replace('"', "").replace('\\', "");
    let _ = js_sys::eval(&format!(
        r#"(function(){{var u=window.XMES_PUSH_WORKER_URL;if(!u)return;fetch(u+"/notify",{{method:"POST",headers:{{"content-type":"application/json"}},body:JSON.stringify({{member_inbox_ids:["{id}"],sender_inbox_id:"",group_name:"{name}",title:"Group welcome",body:"You have been added to group {name}"}})}}).catch(()=>{{}})}})()"#,
        id=id, name=name
    ));
}

#[component]
pub fn AddMembersSheet(
    conversation_id: String,
    conversation_name: String,
    xmtp: Signal<Option<XmtpHandle>>,
    on_close: EventHandler<()>,
) -> Element {
    let mut inbox_input  = use_signal(|| String::new());
    let mut show_scanner = use_signal(|| false);

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
                    "Enter the address or inbox ID of the person you want to add."
                }
                div { class: "add-member-input-row",
                    input {
                        class: "add-member-input",
                        r#type: "text",
                        placeholder: "Address / Inbox ID…",
                        autofocus: true,
                        value: "{inbox_input}",
                        oninput: move |e| inbox_input.set(e.value()),
                        onkeydown: {
                            let conv_id   = conversation_id.clone();
                            let conv_name = conversation_name.clone();
                            move |e: Event<KeyboardData>| {
                                if e.data().code().to_string() == "Enter" && can_add {
                                    let id = inbox_input.read().trim().to_string();
                                    inbox_input.set(String::new());
                                    notify_push_invite(&id, &conv_name);
                                    if let Some(h) = xmtp.read().as_ref() {
                                        h.request_add_members(&conv_id, &[id]);
                                    }
                                    on_close.call(());
                                }
                            }
                        },
                    }
                    button {
                        class: "qr-scan-btn",
                        title: "Scan QR code",
                        onclick: move |_| show_scanner.set(true),
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "18", height: "18",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M11 3H5a2 2 0 0 0-2 2v6" }
                            path { d: "M13 21h6a2 2 0 0 0 2-2v-6" }
                            path { d: "M3 13v6a2 2 0 0 0 2 2h6" }
                            path { d: "M21 11V5a2 2 0 0 0-2-2h-6" }
                            rect { x: "7", y: "7", width: "4", height: "4" }
                            rect { x: "13", y: "7", width: "4", height: "4" }
                            rect { x: "7", y: "13", width: "4", height: "4" }
                        }
                    }
                }
            }

            div { class: "sheet-footer",
                button {
                    class: "add-member-btn",
                    disabled: !can_add,
                    onclick: {
                        let conv_id   = conversation_id.clone();
                        let conv_name = conversation_name.clone();
                        move |_| {
                            let id = inbox_input.read().trim().to_string();
                            if id.is_empty() { return; }
                            inbox_input.set(String::new());
                            notify_push_invite(&id, &conv_name);
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

        if show_scanner() {
            QrScannerSheet {
                conversation_id: conversation_id.clone(),
                xmtp,
                on_close: move |_| {
                    show_scanner.set(false);
                    on_close.call(());
                },
            }
        }
    }
}
