use dioxus::prelude::*;
use js_sys::Date;
use xmes_xmtp_wasm::{ConversationSummary, IdentityInfo, MemberInfo, MessageInfo, XmtpHandle};
use crate::{components::qr::QrScannerSheet, View};

// ── Link detection & preview ──────────────────────────────────────────────────

fn is_known_tld(tld: &str) -> bool {
    matches!(tld,
        "com" | "net" | "org" | "edu" | "gov" | "mil" | "int" | "biz" | "info" | "name" |
        "io" | "co" | "app" | "dev" | "ai" | "me" | "tv" | "cc" | "so" | "sh" | "gg" | "fm" |
        "gl" | "ly" | "is" | "to" | "li" | "ac" | "im" | "vc" | "nu" |
        "online" | "store" | "shop" | "blog" | "news" | "media" | "tech" | "cloud" | "site" |
        "link" | "page" | "live" | "digital" | "studio" | "design" | "agency" | "space" |
        "de" | "at" | "ch" | "uk" | "us" | "ca" | "fr" | "es" | "it" | "nl" | "be" | "pt" |
        "ie" | "se" | "no" | "dk" | "fi" | "ru" | "pl" | "cz" | "hu" | "ro" | "gr" | "hr" |
        "lt" | "lv" | "ee" | "ua" | "tr" | "il" | "ae" | "za" | "in" | "cn" | "jp" | "kr" |
        "tw" | "hk" | "sg" | "my" | "id" | "th" | "vn" | "au" | "nz" | "br" | "mx" | "ar"
    )
}

fn looks_like_bare_url(s: &str) -> bool {
    let low = s.to_lowercase();
    if low.starts_with("http://") || low.starts_with("https://") { return false; }
    let host = s.split(['/', '?', '#', ':']).next().unwrap_or(s);
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() < 2 { return false; }
    let tld = parts.last().unwrap().to_lowercase();
    for part in &parts[..parts.len() - 1] {
        if part.is_empty() { return false; }
        if !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { return false; }
        if part.starts_with('-') || part.ends_with('-') { return false; }
    }
    if parts[0].chars().all(|c| c.is_ascii_digit()) { return false; }
    is_known_tld(&tld)
}

fn find_bare_url_in(text: &str) -> Option<usize> {
    let mut pos = 0;
    while pos < text.len() {
        let rest = &text[pos..];
        let ws = rest.find(|c: char| !c.is_ascii_whitespace()).unwrap_or(rest.len());
        if ws == rest.len() { break; }
        pos += ws;
        let rest = &text[pos..];
        let token_len = rest.find(|c: char| c.is_ascii_whitespace()).unwrap_or(rest.len());
        let token = &rest[..token_len];
        let trimmed = token.trim_end_matches(|c| matches!(c, '.' | ',' | '!' | '?' | ';' | ':'));
        if looks_like_bare_url(trimmed) { return Some(pos); }
        pos += token_len;
    }
    None
}

fn split_urls(text: &str) -> Vec<(String, Option<String>)> {
    let mut result: Vec<(String, Option<String>)> = Vec::new();
    let mut remaining = text;
    loop {
        let low = remaining.to_lowercase();
        let proto = {
            let a = low.find("https://");
            let b = low.find("http://");
            match (a, b) {
                (Some(x), Some(y)) => Some(x.min(y)),
                (Some(x), None) | (None, Some(x)) => Some(x),
                (None, None) => None,
            }
        };
        let bare = find_bare_url_in(remaining);
        let next = match (proto, bare) {
            (Some(p), Some(b)) if p <= b => Some((p, true)),
            (Some(_), Some(b))           => Some((b, false)),
            (Some(p), None)              => Some((p, true)),
            (None,    Some(b))           => Some((b, false)),
            (None,    None)              => None,
        };
        let Some((i, is_proto)) = next else {
            if !remaining.is_empty() { result.push((remaining.to_string(), None)); }
            break;
        };
        if i > 0 { result.push((remaining[..i].to_string(), None)); }
        let rest = &remaining[i..];
        let raw_end = rest
            .find(|c: char| c.is_ascii_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | ')' | ']' | '}'))
            .unwrap_or(rest.len());
        let raw = &rest[..raw_end];
        let display = raw.trim_end_matches(|c| matches!(c, '.' | ',' | '!' | '?' | ';' | ':'));
        let trailing = &raw[display.len()..];
        let href = if is_proto {
            if display.len() > 7 { Some(display.to_string()) } else { None }
        } else {
            Some(format!("https://{}", display))
        };
        if href.is_some() {
            result.push((display.to_string(), href));
            if !trailing.is_empty() { result.push((trailing.to_string(), None)); }
        } else {
            result.push((raw.to_string(), None));
        }
        remaining = &remaining[i + raw_end..];
    }
    result
}

fn extract_first_url(text: &str) -> Option<String> {
    split_urls(text).into_iter().find_map(|(_, href)| href)
}

#[derive(Clone, PartialEq)]
struct LinkPreview {
    url:         String,
    title:       Option<String>,
    description: Option<String>,
    image:       Option<String>,
    site_name:   Option<String>,
}

async fn fetch_preview(url: String) -> Option<LinkPreview> {
    let escaped = url.replace('\\', "\\\\").replace('"', "\\\"");
    let js = format!(
        r#"(async()=>{{try{{
            const r=await fetch("{}",{{headers:{{"Accept":"text/html,application/xhtml+xml"}}}});
            if(!r.ok)return null;
            const ct=r.headers.get("content-type")||"";
            if(!ct.includes("text/html"))return null;
            const html=await r.text();
            const g=(a,v)=>{{
                const p1=new RegExp('<meta[^>]+'+a+'=["\']'+v+'["\'][^>]+content=["\']([^"\'<>]{{1,400}})["\']','i');
                const p2=new RegExp('<meta[^>]+content=["\']([^"\'<>]{{1,400}})["\'][^>]+'+a+'=["\']'+v+'["\']','i');
                const m=html.match(p1)||html.match(p2);
                return m?m[1].trim():null;
            }};
            const title=g('property','og:title')||g('name','twitter:title')||(html.match(/<title[^>]*>([^<]{{1,200}})<\/title>/i)||[])[1]||null;
            const description=g('property','og:description')||g('name','description')||null;
            const image=g('property','og:image')||g('name','twitter:image')||null;
            const site_name=g('property','og:site_name')||null;
            if(!title&&!description&&!image)return null;
            return JSON.stringify({{title:title&&title.trim()||null,description:description&&description.trim()||null,image,site_name}});
        }}catch(e){{return null;}}}})()"#,
        escaped
    );
    let val = js_sys::eval(&js).ok()?;
    let result = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(val)).await.ok()?;
    let json = result.as_string()?;
    let obj = js_sys::JSON::parse(&json).ok()?;
    let get = |k: &str| -> Option<String> {
        js_sys::Reflect::get(&obj, &wasm_bindgen::JsValue::from_str(k)).ok()
            .filter(|v| !v.is_null() && !v.is_undefined())
            .and_then(|v| v.as_string())
            .filter(|s| !s.is_empty())
    };
    let title = get("title");
    let description = get("description");
    let image = get("image");
    let site_name = get("site_name");
    if title.is_none() && description.is_none() && image.is_none() { return None; }
    Some(LinkPreview { url, title, description, image, site_name })
}

#[component]
fn MessageText(text: String, query: String) -> Element {
    let segments = split_urls(&text);
    rsx! {
        for (display, href_opt) in segments {
            if let Some(href) = href_opt {
                a {
                    class: "msg-link",
                    href: "{href}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    onclick: move |e| e.stop_propagation(),
                    HighlightedText { text: display.clone(), query: query.clone() }
                }
            } else {
                HighlightedText { text: display, query: query.clone() }
            }
        }
    }
}

fn av_class(name: &str) -> &'static str {
    let idx = name.bytes().fold(0usize, |a, b| a.wrapping_add(b as usize)) % 8;
    match idx {
        0 => "av-0", 1 => "av-1", 2 => "av-2", 3 => "av-3",
        4 => "av-4", 5 => "av-5", 6 => "av-6", _ => "av-7",
    }
}

fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().filter(|w| !w.is_empty()).collect();
    match words.as_slice() {
        [] => "?".into(),
        [w] => w.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or("?".into()),
        [first, .., last] => format!(
            "{}{}",
            first.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
            last.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
        ),
    }
}

fn format_time_ns(ns: i64) -> String {
    let ms = (ns / 1_000_000) as f64;
    let d = Date::new(&wasm_bindgen::JsValue::from_f64(ms));
    format!("{:02}:{:02}", d.get_hours(), d.get_minutes())
}

fn short_addr(s: &str) -> String {
    if s.len() <= 13 { s.to_string() }
    else { format!("{}…{}", &s[..6], &s[s.len()-4..]) }
}

fn pending_load(conv_id: &str) -> Vec<String> {
    let key = conv_id.replace('\'', "");
    let now_days = js_sys::Date::now() as u64 / 86_400_000;
    let raw = js_sys::eval(&format!("localStorage.getItem('pending_push_{key}')||''"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    raw.split(',')
        .filter_map(|entry| {
            let mut parts = entry.splitn(2, ':');
            let id  = parts.next()?.trim().to_string();
            let day: u64 = parts.next().and_then(|d| d.trim().parse().ok()).unwrap_or(0);
            if id.is_empty() || now_days.saturating_sub(day) > 7 { None } else { Some(id) }
        })
        .collect()
}

fn pending_save(conv_id: &str, members: &[String]) {
    let key  = conv_id.replace('\'', "");
    let day  = (js_sys::Date::now() as u64 / 86_400_000).to_string();
    let data = members.iter()
        .map(|m| format!("{}:{day}", m.replace(['\'', ',', ':'], "")))
        .collect::<Vec<_>>()
        .join(",");
    let _ = js_sys::eval(&format!("localStorage.setItem('pending_push_{key}','{data}')"));
}

fn notify_push_invite(new_member_inbox_id: &str, group_name: &str) {
    let id   = new_member_inbox_id.replace('"', "");
    let name = group_name.replace('"', "").replace('\\', "");
    let _ = js_sys::eval(&format!(
        r#"(function(){{var u=window.XMES_PUSH_WORKER_URL;if(!u)return;fetch(u+"/notify",{{method:"POST",headers:{{"content-type":"application/json"}},body:JSON.stringify({{member_inbox_ids:["{id}"],sender_inbox_id:"",group_name:"{name}",title:"Group welcome",body:"You have been added to group {name}"}})}}).catch(()=>{{}})}})()"#,
        id=id, name=name
    ));
}

fn copy_to_clipboard(text: String, mut copied: Signal<bool>) {
    let _ = js_sys::eval(&format!(
        "navigator.clipboard.writeText('{}')",
        text.replace('\'', "\\'")
    ));
    copied.set(true);
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(1500).await;
        copied.set(false);
    });
}

#[component]
fn CopyBtn(text: String) -> Element {
    let copied = use_signal(|| false);
    rsx! {
        button {
            class: "copy-btn",
            title: if copied() { "Copied!" } else { "Copy" },
            onclick: move |e| { e.stop_propagation(); copy_to_clipboard(text.clone(), copied); },
            if copied() {
                svg {
                    xmlns: "http://www.w3.org/2000/svg", width: "13", height: "13",
                    view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                    stroke_width: "2.8", stroke_linecap: "round", stroke_linejoin: "round",
                    polyline { points: "20 6 9 17 4 12" }
                }
            } else {
                svg {
                    xmlns: "http://www.w3.org/2000/svg", width: "13", height: "13",
                    view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                    stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                    rect { x: "9", y: "9", width: "13", height: "13", rx: "2", ry: "2" }
                    path { d: "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" }
                }
            }
        }
    }
}

fn role_label(role: u8) -> &'static str {
    match role {
        2 => "Super Admin",
        1 => "Admin",
        _ => "Member",
    }
}

fn role_class(role: u8) -> &'static str {
    match role {
        2 => "role-badge role-superadmin",
        1 => "role-badge role-admin",
        _ => "role-badge role-member",
    }
}

/// Slide-in panel from the right showing group members + add member.
#[component]
fn ChatGroupSettingsPanel(
    conversation_id: String,
    conv_name: Signal<String>,
    members: Vec<MemberInfo>,
    own_inbox_id: String,
    xmtp: Signal<Option<XmtpHandle>>,
    pending_members: Signal<Vec<String>>,
    on_close: EventHandler<()>,
) -> Element {
    let mut add_input:    Signal<String>         = use_signal(|| String::new());
    let mut menu_open:    Signal<Option<String>>  = use_signal(|| None);
    let mut show_rename:  Signal<bool>            = use_signal(|| false);
    let mut rename_input: Signal<String>          = use_signal(move || conv_name.peek().clone());
    let mut show_scanner: Signal<bool>            = use_signal(|| false);

    let own_role = members.iter()
        .find(|m| m.inbox_id == own_inbox_id)
        .map(|m| m.role)
        .unwrap_or(0);
    let can_rename = own_role >= 1;

    rsx! {
        div {
            class: "members-panel-backdrop",
            onclick: move |_| on_close.call(()),
        }
        div { class: "members-panel",

            // ── Header ────────────────────────────────────────────
            div { class: "members-panel-header",
                if show_rename() {
                    input {
                        class: "members-rename-input",
                        r#type: "text",
                        value: "{rename_input}",
                        oninput: move |e| rename_input.set(e.value()),
                        onkeydown: {
                            let conv_id = conversation_id.clone();
                            move |e: Event<KeyboardData>| {
                                if e.data().code().to_string() == "Enter" {
                                    let name = rename_input.read().trim().to_string();
                                    if !name.is_empty() {
                                        conv_name.set(name.clone());
                                        if let Some(h) = xmtp.peek().as_ref() {
                                            h.request_update_group_name(&conv_id, &name);
                                        }
                                    }
                                    show_rename.set(false);
                                }
                                if e.data().code().to_string() == "Escape" {
                                    rename_input.set(conv_name.peek().clone());
                                    show_rename.set(false);
                                }
                            }
                        },
                    }
                    button {
                        class: "members-rename-confirm",
                        disabled: rename_input.read().trim().is_empty(),
                        onclick: {
                            let conv_id = conversation_id.clone();
                            move |_| {
                                let name = rename_input.read().trim().to_string();
                                if !name.is_empty() {
                                    conv_name.set(name.clone());
                                    if let Some(h) = xmtp.peek().as_ref() {
                                        h.request_update_group_name(&conv_id, &name);
                                    }
                                }
                                show_rename.set(false);
                            }
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2.8", stroke_linecap: "round", stroke_linejoin: "round",
                            polyline { points: "20 6 9 17 4 12" }
                        }
                    }
                } else {
                    if can_rename {
                        button {
                            class: "member-menu-btn",
                            title: "Rename group",
                            onclick: move |_| show_rename.set(true),
                            svg {
                                xmlns: "http://www.w3.org/2000/svg", width: "15", height: "15",
                                view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                                path { d: "M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" }
                                path { d: "M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" }
                            }
                        }
                    }
                    span { class: "members-panel-title", "{conv_name()}" }
                }
                button {
                    class: "panel-close-btn",
                    onclick: move |_| on_close.call(()),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        line { x1: "18", y1: "6", x2: "6", y2: "18" }
                        line { x1: "6", y1: "6", x2: "18", y2: "18" }
                    }
                }
            }

            // ── Member list ───────────────────────────────────────
            div { class: "members-list",
                for m in members.iter() {
                    {
                        let m = m.clone();
                        let is_menu_open = menu_open.read().as_deref() == Some(&m.inbox_id);
                        let show_menu_btn = own_role >= 1 && m.role < 2;
                        let conv_id = conversation_id.clone();
                        let iid = m.inbox_id.clone();
                        rsx! {
                            div { class: "member-row",
                                div { class: "addr-primary-pill",
                                    span { class: "addr-primary-text", "{short_addr(&m.address)}" }
                                    CopyBtn { text: m.address.clone() }
                                }
                                span { class: "{role_class(m.role)}", "{role_label(m.role)}" }
                                if show_menu_btn {
                                    div { class: "member-menu-wrap",
                                        button {
                                            class: "member-menu-btn",
                                            title: "Manage member",
                                            onclick: move |e| {
                                                e.stop_propagation();
                                                if is_menu_open {
                                                    menu_open.set(None);
                                                } else {
                                                    menu_open.set(Some(iid.clone()));
                                                }
                                            },
                                            svg {
                                                xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                                                view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                                                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                                                circle { cx: "12", cy: "5",  r: "1" }
                                                circle { cx: "12", cy: "12", r: "1" }
                                                circle { cx: "12", cy: "19", r: "1" }
                                            }
                                        }
                                        if is_menu_open {
                                            div { class: "member-dropdown-overlay", onclick: move |_| menu_open.set(None) }
                                            div { class: "member-dropdown",
                                                onclick: move |e| e.stop_propagation(),
                                                if own_role == 2 {
                                                    if m.role == 0 {
                                                        button {
                                                            class: "member-dropdown-item",
                                                            onclick: {
                                                                let cid = conv_id.clone();
                                                                let mid = m.inbox_id.clone();
                                                                move |_| {
                                                                    menu_open.set(None);
                                                                    if let Some(h) = xmtp.peek().as_ref() {
                                                                        h.request_set_admin(&cid, &mid, true);
                                                                    }
                                                                }
                                                            },
                                                            "Make Admin"
                                                        }
                                                        button {
                                                            class: "member-dropdown-item",
                                                            onclick: {
                                                                let cid = conv_id.clone();
                                                                let mid = m.inbox_id.clone();
                                                                move |_| {
                                                                    menu_open.set(None);
                                                                    if let Some(h) = xmtp.peek().as_ref() {
                                                                        h.request_set_super_admin(&cid, &mid, true);
                                                                    }
                                                                }
                                                            },
                                                            "Make Super Admin"
                                                        }
                                                    }
                                                    if m.role == 1 {
                                                        button {
                                                            class: "member-dropdown-item",
                                                            onclick: {
                                                                let cid = conv_id.clone();
                                                                let mid = m.inbox_id.clone();
                                                                move |_| {
                                                                    menu_open.set(None);
                                                                    if let Some(h) = xmtp.peek().as_ref() {
                                                                        h.request_set_admin(&cid, &mid, false);
                                                                    }
                                                                }
                                                            },
                                                            "Remove Admin"
                                                        }
                                                        button {
                                                            class: "member-dropdown-item",
                                                            onclick: {
                                                                let cid = conv_id.clone();
                                                                let mid = m.inbox_id.clone();
                                                                move |_| {
                                                                    menu_open.set(None);
                                                                    if let Some(h) = xmtp.peek().as_ref() {
                                                                        h.request_set_super_admin(&cid, &mid, true);
                                                                    }
                                                                }
                                                            },
                                                            "Make Super Admin"
                                                        }
                                                    }
                                                }
                                                button {
                                                    class: "member-dropdown-item member-dropdown-danger",
                                                    onclick: {
                                                        let cid = conv_id.clone();
                                                        let mid = m.inbox_id.clone();
                                                        move |_| {
                                                            menu_open.set(None);
                                                            if let Some(h) = xmtp.peek().as_ref() {
                                                                h.request_remove_member(&cid, &mid);
                                                            }
                                                        }
                                                    },
                                                    "Remove from group"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Footer: add member ────────────────────────────────
            div { class: "members-panel-footer",
                div { class: "add-member-input-row",
                    input {
                        class: "add-member-input",
                        r#type: "text",
                        placeholder: "Address / Inbox ID…",
                        value: "{add_input}",
                        oninput: move |e| add_input.set(e.value()),
                        onkeydown: {
                            let conv_id   = conversation_id.clone();
                            let conv_name = conv_name.clone();
                            move |e: Event<KeyboardData>| {
                                if e.data().code().to_string() == "Enter" {
                                    let id = add_input.read().trim().to_string();
                                    if id.is_empty() { return; }
                                    add_input.set(String::new());
                                    notify_push_invite(&id, &conv_name.peek());
                                    pending_members.write().push(id.clone());
                                    pending_save(&conv_id, &pending_members.read());
                                    if let Some(h) = xmtp.peek().as_ref() {
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
                    button {
                        class: "add-member-btn",
                        disabled: add_input.read().trim().is_empty(),
                        onclick: {
                            let conv_id   = conversation_id.clone();
                            let conv_name = conv_name.clone();
                            move |_| {
                                let id = add_input.read().trim().to_string();
                                if id.is_empty() { return; }
                                add_input.set(String::new());
                                notify_push_invite(&id, &conv_name.peek());
                                pending_members.write().push(id.clone());
                                pending_save(&conv_id, &pending_members.read());
                                if let Some(h) = xmtp.peek().as_ref() {
                                    h.request_add_members(&conv_id, &[id]);
                                }
                                on_close.call(());
                            }
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "15", height: "15",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
                            circle { cx: "9", cy: "7", r: "4" }
                            line { x1: "19", y1: "8", x2: "19", y2: "14" }
                            line { x1: "22", y1: "11", x2: "16", y2: "11" }
                        }
                        "Add"
                    }
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

#[component]
fn HighlightedText(text: String, query: String) -> Element {
    if query.is_empty() {
        return rsx! { "{text}" };
    }
    let t_lower = text.to_lowercase();
    let q_lower = query.to_lowercase();
    let mut segments: Vec<(String, bool)> = Vec::new();
    let mut last = 0usize;
    loop {
        match t_lower[last..].find(&*q_lower) {
            None => {
                if last < text.len() { segments.push((text[last..].to_string(), false)); }
                break;
            }
            Some(rel) => {
                let abs = last + rel;
                if abs > last { segments.push((text[last..abs].to_string(), false)); }
                segments.push((text[abs..abs + q_lower.len()].to_string(), true));
                last = abs + q_lower.len();
            }
        }
    }
    rsx! {
        for (seg, highlighted) in segments {
            if highlighted {
                span { class: "search-highlight", "{seg}" }
            } else {
                span { "{seg}" }
            }
        }
    }
}

#[component]
pub fn Chat(conversation: ConversationSummary) -> Element {
    let mut text_input          = use_signal(|| String::new());
    let view                    = use_context::<Signal<View>>();
    let xmtp                    = use_context::<Signal<Option<XmtpHandle>>>();
    let mut messages            = use_context::<Signal<Vec<MessageInfo>>>();
    let group_members           = use_context::<Signal<Vec<MemberInfo>>>();
    let identity_info           = use_context::<Signal<Option<IdentityInfo>>>();
    let mut unread_ids          = use_context::<Signal<std::collections::HashSet<String>>>();
    let mut initial_scroll_done = use_signal(|| false);
    let mut user_scrolled_up    = use_signal(|| false);
    let mut loading             = use_signal(|| true);
    let mut show_search         = use_signal(|| false);
    let mut search_query        = use_signal(String::new);

    let conv_id_for_pending = conversation.id.clone();
    let mut pending_members: Signal<Vec<String>> = use_signal(move || {
        pending_load(&conv_id_for_pending)
    });
    let mut show_members = use_signal(|| false);
    let mut conv_name    = use_signal(|| conversation.name.clone());
    let conv_id          = conversation.id.clone();
    let own_inbox        = identity_info.read().as_ref().map(|i| i.inbox_id.clone()).unwrap_or_default();

    let link_previews: Signal<std::collections::HashMap<String, Option<LinkPreview>>> =
        use_signal(|| std::collections::HashMap::new());

    use_effect(move || {
        for msg in messages.read().iter() {
            let Some(url) = extract_first_url(&msg.text) else { continue };
            if link_previews.peek().contains_key(&url) { continue; }
            link_previews.write().insert(url.clone(), None);
            let mut lp = link_previews;
            spawn(async move {
                let preview = fetch_preview(url.clone()).await;
                lp.write().insert(url, preview);
            });
        }
    });

    let member_count  = group_members.read().len();
    let member_label  = if member_count == 1 { "1 Member".to_string() }
                        else { format!("{} Members", member_count) };

    let conv_id_pending = conversation.id.clone();
    use_effect(move || {
        let msgs = messages.read();
        let mut pending = pending_members.write();
        let before = pending.len();
        pending.retain(|id| !msgs.iter().any(|m| &m.sender_inbox_id == id));
        if pending.len() != before {
            pending_save(&conv_id_pending, &pending);
        }
    });

    let conv_id_unread = conversation.id.clone();
    use_effect(move || {
        messages.set(vec![]);
        unread_ids.write().remove(&conv_id_unread);
        if let Some(h) = xmtp.read().as_ref() {
            h.request_list_messages(&conv_id);
            h.request_list_members(&conv_id);
        }
    });

    let conv_id_sync = conversation.id.clone();
    use_effect(move || {
        let id = conv_id_sync.clone();
        let interval = gloo_timers::callback::Interval::new(8_000, move || {
            let still_open = matches!(view.peek().clone(), View::Chat(c) if c.id == id);
            if still_open {
                if let Some(h) = xmtp.peek().as_ref() {
                    h.request_list_messages(&id);
                }
            }
        });
        interval.forget();
    });

    use_effect(move || {
        let _ = messages.read();
        loading.set(false);
        let is_initial = !*initial_scroll_done.peek();
        let scrolled_up = *user_scrolled_up.peek();
        if is_initial || !scrolled_up {
            if let Some(window) = web_sys::window() {
                if let Some(doc) = window.document() {
                    if let Some(el) = doc.query_selector(".chat-messages").ok().flatten() {
                        el.set_scroll_top(el.scroll_height());
                        initial_scroll_done.set(true);
                    }
                }
            }
        }
    });

    let search_q = search_query.read().trim().to_string();
    let visible_msgs: Vec<MessageInfo> = {
        let q = search_q.to_lowercase();
        messages.read().iter()
            .filter(|m| {
                q.is_empty()
                    || m.text.to_lowercase().contains(&q)
                    || m.system_text.as_deref()
                        .map(|t| t.to_lowercase().contains(&q))
                        .unwrap_or(false)
                    || m.sender_inbox_id.to_lowercase().contains(&q)
                    || m.sender_address.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    };

    rsx! {
        div { class: "chat-shell",

            // ── Header ───────────────────────────────────────────
            header { class: "chat-header",
                div { class: "chat-header-avatar {av_class(&conv_name())}", "{initials(&conv_name())}" }
                div { class: "chat-header-center",
                    div { class: "chat-header-info",
                        span { class: "chat-header-name", "{conv_name()}" }
                        span { class: "chat-header-sub", "{member_label}" }
                    }
                }
                button {
                    class: if show_search() { "chat-search-btn active" } else { "chat-search-btn" },
                    title: "Search in conversation",
                    onclick: move |_| {
                        let next = !show_search();
                        show_search.set(next);
                        if !next { search_query.set(String::new()); }
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "20", height: "20",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                        circle { cx: "11", cy: "11", r: "8" }
                        path { d: "m21 21-4.35-4.35" }
                    }
                }
                button {
                    class: "chat-menu-btn",
                    title: "Group members",
                    onclick: move |_| show_members.set(true),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "20", height: "20",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                        circle { cx: "12", cy: "5",  r: "1" }
                        circle { cx: "12", cy: "12", r: "1" }
                        circle { cx: "12", cy: "19", r: "1" }
                    }
                }
            }

            // ── Members panel ─────────────────────────────────────
            if show_members() {
                ChatGroupSettingsPanel {
                    conversation_id: conversation.id.clone(),
                    conv_name,
                    members: group_members.read().clone(),
                    own_inbox_id: own_inbox.clone(),
                    xmtp,
                    pending_members,
                    on_close: move |_| show_members.set(false),
                }
            }

            // ── Search bar ───────────────────────────────────────
            if show_search() {
                div { class: "chat-search-bar",
                    input {
                        class: "chat-search-input",
                        r#type: "text",
                        placeholder: "Search conversation…",
                        value: "{search_query}",
                        autofocus: true,
                        oninput: move |e| search_query.set(e.value()),
                    }
                    button {
                        class: "chat-search-clear",
                        title: "Close search",
                        onclick: move |_| {
                            show_search.set(false);
                            search_query.set(String::new());
                        },
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "16", height: "16",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                            line { x1: "18", y1: "6", x2: "6", y2: "18" }
                            line { x1: "6", y1: "6", x2: "18", y2: "18" }
                        }
                    }
                }
            }

            // ── Messages ─────────────────────────────────────────
            div {
                class: "chat-messages",
                onscroll: move |_| {
                    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                        if let Some(el) = doc.query_selector(".chat-messages").ok().flatten() {
                            let distance = el.scroll_height() - el.scroll_top() - el.client_height();
                            user_scrolled_up.set(distance > 80);
                        }
                    }
                },
                if loading() {
                    div { class: "chat-loading",
                        div { class: "chat-spinner" }
                    }
                } else if messages.read().is_empty() {
                    div { class: "chat-empty",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "32", height: "32",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "1.5", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                        }
                        span { "No messages yet. Say hello!" }
                    }
                } else if visible_msgs.is_empty() {
                    div { class: "chat-empty",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg", width: "32", height: "32",
                            view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                            stroke_width: "1.5", stroke_linecap: "round", stroke_linejoin: "round",
                            circle { cx: "11", cy: "11", r: "8" }
                            path { d: "m21 21-4.35-4.35" }
                        }
                        span { "No results" }
                    }
                }
                for msg in visible_msgs.iter() {
                    {
                        let system_text = msg.system_text.clone();
                        let is_own  = msg.sender_inbox_id == own_inbox;
                        let time    = format_time_ns(msg.sent_at_ns);
                        let text    = msg.text.clone();
                        let deliv   = msg.delivered;
                        let sender_addr = if !is_own {
                            group_members.read().iter()
                                .find(|m| m.inbox_id == msg.sender_inbox_id)
                                .map(|m| short_addr(&m.address))
                        } else { None };
                        let preview_data: Option<(String, Option<String>, Option<String>, Option<String>, Option<String>)> = {
                            let previews = link_previews.read();
                            extract_first_url(&msg.text).and_then(|u| {
                                previews.get(&u).and_then(|p| p.as_ref().map(|p| (
                                    p.url.clone(), p.image.clone(), p.title.clone(),
                                    p.description.clone(), p.site_name.clone()
                                )))
                            })
                        };
                        rsx! {
                            if let Some(ref st) = system_text {
                                div { class: "system-event", "{st}" }
                            } else {
                            div { class: if is_own { "bubble-row own" } else { "bubble-row other" },
                                if !is_own {
                                    div { class: "bubble-avatar {av_class(&conv_name())}", "{initials(&conv_name())}" }
                                }
                                div { class: "bubble-col",
                                    if let Some(ref addr) = sender_addr {
                                        span { class: "bubble-sender",
                                            HighlightedText { text: addr.clone(), query: search_q.clone() }
                                        }
                                    }
                                    if let Some((purl, pimg, ptitle, pdesc, psite)) = preview_data {
                                        a {
                                            class: if is_own { "link-preview own" } else { "link-preview other" },
                                            href: "{purl}",
                                            target: "_blank",
                                            rel: "noopener noreferrer",
                                            onclick: move |e| e.stop_propagation(),
                                            if let Some(ref src) = pimg {
                                                img { class: "link-preview-img", src: "{src}", alt: "" }
                                            }
                                            div { class: "link-preview-body",
                                                if let Some(ref site) = psite {
                                                    span { class: "link-preview-site", "{site}" }
                                                }
                                                if let Some(ref t) = ptitle {
                                                    p { class: "link-preview-title", "{t}" }
                                                }
                                                if let Some(ref d) = pdesc {
                                                    p { class: "link-preview-desc", "{d}" }
                                                }
                                            }
                                            div { class: if is_own { "link-preview-message own" } else { "link-preview-message other" },
                                                MessageText { text, query: search_q.clone() }
                                            }
                                        }
                                    } else {
                                        div { class: if is_own { "bubble own" } else { "bubble other" },
                                            MessageText { text, query: search_q.clone() }
                                        }
                                    }
                                    div { class: "bubble-meta",
                                        span { class: "bubble-time", "{time}" }
                                        if is_own {
                                            if deliv {
                                                span { class: "bubble-sent",
                                                    svg {
                                                        xmlns: "http://www.w3.org/2000/svg",
                                                        width: "12", height: "12",
                                                        view_box: "0 0 24 24", fill: "none",
                                                        stroke: "currentColor", stroke_width: "2.8",
                                                        stroke_linecap: "round", stroke_linejoin: "round",
                                                        polyline { points: "20 6 9 17 4 12" }
                                                    }
                                                }
                                            } else {
                                                span { class: "bubble-sending", "•" }
                                            }
                                        }
                                    }
                                }
                            }
                            }
                        }
                    }
                }
            }

            // ── Input bar ────────────────────────────────────────
            div { class: "chat-input-bar",
                textarea {
                    class: "chat-input",
                    rows: "1",
                    placeholder: "Message…",
                    value: "{text_input}",
                    oninput: move |e| {
                        text_input.set(e.value());
                        let _ = js_sys::eval(
                            "var el=document.querySelector('.chat-input');\
                             if(el){el.style.height='auto';el.style.height=el.scrollHeight+'px';}"
                        );
                    },
                    onkeydown: {
                        let conv_id2   = conversation.id.clone();
                        let own_inbox2 = own_inbox.clone();
                        move |e: Event<KeyboardData>| {
                            let is_enter = e.data().code().to_string() == "Enter";
                            let shift    = e.data().modifiers().shift();
                            if is_enter && !shift {
                                e.prevent_default();
                                let text = text_input.read().trim().to_string();
                                if text.is_empty() { return; }
                                text_input.set(String::new());
                                let _ = js_sys::eval(
                                    "var el=document.querySelector('.chat-input');\
                                     if(el){el.style.height='';}"
                                );
                                let mut m = messages;
                                let mut list = m.read().clone();
                                list.push(MessageInfo {
                                    id:              format!("pending-{}", Date::now() as i64),
                                    text:            text.clone(),
                                    system_text:     None,
                                    sender_inbox_id: own_inbox2.clone(),
                                    sender_address:  String::new(),
                                    sent_at_ns:      (Date::now() * 1_000_000.0) as i64,
                                    delivered:       false,
                                });
                                m.set(list);
                                if let Some(h) = xmtp.read().as_ref() {
                                    h.request_send_message(&conv_id2, &text);
                                    let _ = js_sys::eval("window.__xmes_push_pending=(window.__xmes_push_pending||0)+1");
                                }
                            }
                        }
                    },
                }
                button {
                    class: "chat-send-btn",
                    disabled: text_input.read().trim().is_empty(),
                    title: "Send",
                    onclick: {
                        let conv_id3   = conversation.id.clone();
                        let own_inbox3 = own_inbox.clone();
                        move |_| {
                            let text = text_input.read().trim().to_string();
                            if text.is_empty() { return; }
                            text_input.set(String::new());
                            let _ = js_sys::eval(
                                "var el=document.querySelector('.chat-input');\
                                 if(el){el.style.height='';}"
                            );
                            let mut m = messages;
                            let mut list = m.read().clone();
                            list.push(MessageInfo {
                                id:              format!("pending-{}", Date::now() as i64),
                                text:            text.clone(),
                                sender_inbox_id: own_inbox3.clone(),
                                sender_address:  String::new(),
                                sent_at_ns:      (Date::now() * 1_000_000.0) as i64,
                                delivered:       false,
                                system_text:     None,
                            });
                            m.set(list);
                            if let Some(h) = xmtp.read().as_ref() {
                                h.request_send_message(&conv_id3, &text);
                                let _ = js_sys::eval("window.__xmes_push_pending=(window.__xmes_push_pending||0)+1");
                            }
                        }
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "18", height: "18",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M22 2 11 13" }
                        path { d: "M22 2 15 22 11 13 2 9l20-7z" }
                    }
                }
            }
        }
    }
}
