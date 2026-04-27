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

/// Accept Ethereum addresses (0x + 40 hex) and XMTP inbox IDs (64 hex).
fn is_valid_qr_result(s: &str) -> bool {
    let s = s.trim();
    (s.len() == 42
        && (s.starts_with("0x") || s.starts_with("0X"))
        && s[2..].chars().all(|c| c.is_ascii_hexdigit()))
    || (s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Start camera; Rust side will read frames via the canvas element.
fn start_camera_js() {
    let _ = js_sys::eval(r#"(async () => {
        window.__xmes_cam_error = null;
        window.__xmes_cam_ready = false;
        try {
            const stream = await navigator.mediaDevices.getUserMedia({
                video: { facingMode: 'environment', width: { ideal: 1280 }, height: { ideal: 720 } }
            });
            const video = document.getElementById('xmes-qr-video');
            if (!video) { stream.getTracks().forEach(t => t.stop()); return; }
            video.srcObject = stream;
            await video.play();
            window.__xmes_qr_stream = stream;

            // Off-screen canvas that Rust reads pixel data from
            let canvas = document.getElementById('__xmes_qr_canvas');
            if (!canvas) {
                canvas = document.createElement('canvas');
                canvas.id = '__xmes_qr_canvas';
                canvas.style.cssText = 'position:fixed;top:-9999px;left:-9999px;';
                document.body.appendChild(canvas);
            }
            window.__xmes_cam_ready = true;
        } catch(e) {
            window.__xmes_cam_error = e.message || 'Camera access denied';
        }
    })()"#);
}

fn stop_camera_js() {
    let _ = js_sys::eval(r#"(function() {
        window.__xmes_cam_ready = false;
        if (window.__xmes_qr_stream) {
            window.__xmes_qr_stream.getTracks().forEach(t => t.stop());
            window.__xmes_qr_stream = null;
        }
        const c = document.getElementById('__xmes_qr_canvas');
        if (c) c.remove();
    })()"#);
}

/// Read one video frame into the canvas and return RGBA pixels + dimensions.
fn capture_frame() -> Option<(Vec<u8>, u32, u32)> {
    use wasm_bindgen::JsCast;
    let doc = web_sys::window()?.document()?;

    let video = doc.get_element_by_id("xmes-qr-video")
        .and_then(|e| e.dyn_into::<web_sys::HtmlVideoElement>().ok())?;
    let w = video.video_width();
    let h = video.video_height();
    if w == 0 || h == 0 { return None; }

    let canvas = doc.get_element_by_id("__xmes_qr_canvas")
        .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())?;
    canvas.set_width(w);
    canvas.set_height(h);

    let ctx = canvas.get_context("2d").ok()??
        .dyn_into::<web_sys::CanvasRenderingContext2d>().ok()?;
    ctx.draw_image_with_html_video_element(&video, 0.0, 0.0).ok()?;

    let img_data = ctx.get_image_data(0.0, 0.0, w as f64, h as f64).ok()?;
    Some((img_data.data().to_vec(), w, h))
}

/// Decode a QR code from raw RGBA pixels using rqrr.
fn decode_qr(rgba: &[u8], width: u32, height: u32) -> Option<String> {
    let luma = image::GrayImage::from_fn(width, height, |x, y| {
        let i = (y * width + x) as usize * 4;
        let l = (rgba[i] as u32 * 299 + rgba[i+1] as u32 * 587 + rgba[i+2] as u32 * 114) / 1000;
        image::Luma([l as u8])
    });
    let mut prepared = rqrr::PreparedImage::prepare(luma);
    for grid in prepared.detect_grids() {
        if let Ok((_, content)) = grid.decode() {
            return Some(content);
        }
    }
    None
}

#[component]
pub fn QrScannerSheet(
    conversation_id: String,
    xmtp: Signal<Option<XmtpHandle>>,
    on_close: EventHandler<()>,
) -> Element {
    let mut error_msg:   Signal<Option<String>> = use_signal(|| None);
    let mut scanned:     Signal<Option<String>> = use_signal(|| None);
    let mut active:      Signal<bool>           = use_signal(|| true);
    let mut status_text: Signal<String>         = use_signal(|| "Starting camera…".into());

    use_effect(move || {
        start_camera_js();
        spawn(async move {
            // Wait for camera to be ready
            loop {
                gloo_timers::future::TimeoutFuture::new(200).await;
                if !*active.peek() { return; }

                if let Ok(v) = js_sys::eval("window.__xmes_cam_error||''") {
                    let s = v.as_string().unwrap_or_default();
                    if !s.is_empty() { error_msg.set(Some(s)); return; }
                }
                let ready = js_sys::eval("!!window.__xmes_cam_ready")
                    .ok().and_then(|v| v.as_bool()).unwrap_or(false);
                if ready { break; }
            }

            status_text.set("Scanning…".into());

            // Scan loop: capture frames and decode with rqrr
            loop {
                gloo_timers::future::TimeoutFuture::new(250).await;
                if !*active.peek() { break; }

                if let Some((rgba, w, h)) = capture_frame() {
                    if let Some(result) = decode_qr(&rgba, w, h) {
                        if is_valid_qr_result(&result) {
                            scanned.set(Some(result));
                            break;
                        } else {
                            status_text.set(format!("Found (invalid): {:.20}", result));
                        }
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
                p { class: "qr-scanner-status", "{status_text}" }
            }
        }
    }
}
