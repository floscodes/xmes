use dioxus::prelude::*;
use xmes_xmtp_wasm::XmtpHandle;

// ── QR code generation ────────────────────────────────────────────────────────

fn address_to_qr_svg(address: &str) -> String {
    use qrcode::{Color, QrCode};
    let code = match QrCode::new(address.as_bytes()) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let width = code.width();
    let colors = code.into_colors();
    let module_px = 8u32;
    let quiet = 4u32;
    let total = (width as u32 + quiet * 2) * module_px;

    let mut path_d = String::new();
    for row in 0..width {
        for col in 0..width {
            if colors[row * width + col] == Color::Dark {
                let x = (col as u32 + quiet) * module_px;
                let y = (row as u32 + quiet) * module_px;
                path_d.push_str(&format!(
                    "M{x},{y}h{module_px}v{module_px}h-{module_px}z"
                ));
            }
        }
    }

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total} {total}" style="width:260px;height:260px"><rect width="{total}" height="{total}" fill="white"/><path fill="black" d="{path_d}"/></svg>"#
    )
}

// ── ShowAddressQrSheet ────────────────────────────────────────────────────────

#[component]
pub fn ShowAddressQrSheet(address: String, on_close: EventHandler<()>) -> Element {
    let svg = address_to_qr_svg(&address);
    rsx! {
        div { class: "sheet-backdrop", onclick: move |_| on_close.call(()), }
        div { class: "identity-sheet qr-sheet",
            div { class: "sheet-handle" }
            div { class: "sheet-header",
                span { class: "sheet-title", "Address QR Code" }
                button {
                    class: "sheet-close-btn",
                    onclick: move |_| on_close.call(()),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "14", height: "14",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.5", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M18 6L6 18" }
                        path { d: "M6 6l12 12" }
                    }
                }
            }
            div { class: "qr-svg-wrap",
                dangerous_inner_html: "{svg}",
            }
            p { class: "qr-address-label", "{address}" }
        }
    }
}

// ── QrScannerSheet ────────────────────────────────────────────────────────────

fn is_valid_eth_address(s: &str) -> bool {
    let s = s.trim();
    s.len() == 42
        && (s.starts_with("0x") || s.starts_with("0X"))
        && s[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn start_camera_js() {
    let _ = js_sys::eval(
        r#"(async () => {
            window.__xmes_qr_result = null;
            window.__xmes_qr_error  = null;
            try {
                const stream = await navigator.mediaDevices.getUserMedia({
                    video: { facingMode: 'environment' }
                });
                const video = document.getElementById('xmes-qr-video');
                if (!video) { stream.getTracks().forEach(t => t.stop()); return; }
                video.srcObject = stream;
                video.play();
                window.__xmes_qr_stream = stream;
                if (!('BarcodeDetector' in window)) {
                    window.__xmes_qr_error = 'QR scanner not supported in this browser';
                    return;
                }
                const det = new BarcodeDetector({ formats: ['qr_code'] });
                window.__xmes_qr_timer = setInterval(async () => {
                    try {
                        const r = await det.detect(video);
                        if (r.length > 0) window.__xmes_qr_result = r[0].rawValue;
                    } catch(_) {}
                }, 400);
            } catch(e) {
                window.__xmes_qr_error = e.message || 'Camera access denied';
            }
        })()"#,
    );
}

fn stop_camera_js() {
    let _ = js_sys::eval(
        r#"(function() {
            if (window.__xmes_qr_timer) { clearInterval(window.__xmes_qr_timer); window.__xmes_qr_timer = null; }
            if (window.__xmes_qr_stream) { window.__xmes_qr_stream.getTracks().forEach(t => t.stop()); window.__xmes_qr_stream = null; }
            window.__xmes_qr_result = null;
            window.__xmes_qr_error  = null;
        })()"#,
    );
}

#[component]
pub fn QrScannerSheet(
    conversation_id: String,
    xmtp: Signal<Option<XmtpHandle>>,
    on_close: EventHandler<()>,
) -> Element {
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);
    let mut scanned:   Signal<Option<String>> = use_signal(|| None);
    let mut active:    Signal<bool>           = use_signal(|| true);

    use_effect(move || {
        start_camera_js();
        spawn(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(500).await;
                if !*active.peek() { break; }

                if let Ok(v) = js_sys::eval("window.__xmes_qr_error||''") {
                    let s = v.as_string().unwrap_or_default();
                    if !s.is_empty() {
                        error_msg.set(Some(s));
                        let _ = js_sys::eval("window.__xmes_qr_error=null");
                    }
                }
                if let Ok(v) = js_sys::eval("window.__xmes_qr_result||''") {
                    let s = v.as_string().unwrap_or_default();
                    if !s.is_empty() && is_valid_eth_address(&s) {
                        let _ = js_sys::eval("window.__xmes_qr_result=null");
                        scanned.set(Some(s));
                        break;
                    }
                }
            }
        });
    });

    use_effect(move || {
        if let Some(addr) = scanned.read().clone() {
            stop_camera_js();
            active.set(false);
            if let Some(h) = xmtp.read().as_ref() {
                h.request_add_members(&conversation_id, &[addr]);
            }
            on_close.call(());
        }
    });

    let close = move |_: Event<MouseData>| {
        stop_camera_js();
        active.set(false);
        on_close.call(());
    };

    rsx! {
        div { class: "sheet-backdrop", onclick: close, }
        div { class: "identity-sheet qr-sheet",
            div { class: "sheet-handle" }
            div { class: "sheet-header",
                span { class: "sheet-title", "Scan QR Code" }
                button {
                    class: "sheet-close-btn",
                    onclick: close,
                    svg {
                        xmlns: "http://www.w3.org/2000/svg", width: "14", height: "14",
                        view_box: "0 0 24 24", fill: "none", stroke: "currentColor",
                        stroke_width: "2.5", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M18 6L6 18" }
                        path { d: "M6 6l12 12" }
                    }
                }
            }

            if let Some(err) = error_msg.read().clone() {
                div { class: "qr-scanner-error", "{err}" }
            } else {
                video {
                    id: "xmes-qr-video",
                    class: "qr-scanner-video",
                    autoplay: true,
                    muted: true,
                    playsinline: true,
                }
                p { class: "qr-scanner-hint", "Point the camera at an Ethereum address QR code." }
            }
        }
    }
}
